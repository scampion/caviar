use anyhow::Result;
use candle_core::{DType, Device, Result as CandleResult, Tensor as CandleTensor};
use candle_nn::{VarBuilder, ops::softmax};
use candle_transformers::models::modernbert::{ModernBertForTokenClassification, Config, NERItem};
use hf_hub::{Repo, RepoType, api::sync::Api};
use log::info;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokenizers::Tokenizer;

#[derive(Deserialize)]
pub struct InputText {
    pub text: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Entity {
    pub word: String,
    pub entity: String,
    pub score: f32,
    pub start: usize,
    pub end: usize,
    pub index: usize,
}

#[derive(Serialize)]
pub struct PIIResponse {
    pub entities: Vec<Entity>,
}

pub struct PiiDetector {
    model: Mutex<ModernBertForTokenClassification>,
    tokenizer: Mutex<Tokenizer>,
    labels: Vec<String>,
    device: Device,
}

impl PiiDetector {
    pub fn new() -> Result<Self> {

        let device = Device::cuda_if_available(0)?;
        // Due to a bug in the tokeniser we load that one first
        let api = Api::new()?;
        let repo = api.repo(Repo::with_revision(
            "answerdotai/ModernBERT-base".to_string(),
            RepoType::Model,
            "main".to_string(),
        ));
        let tokenizer_filename = repo.get("tokenizer.json")?;
        info!("Loading tokenizer from {:?}", tokenizer_filename);
        let tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(|e| {
            log::error!("Error loading tokenizer: {}", e);
        }).unwrap();

        let api = Api::new()?;
        let repo = api.repo(Repo::with_revision(
            "scampion/piiranha".to_string(),
            RepoType::Model,
            "main".to_string(),
        ));

        // let tokenizer_filename = repo.get("tokenizer.json").unwrap();
        // info!("Loading tokenizer from {:?}", tokenizer_filename);
        // let mut tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(|e| {
        //     error!("Error loading tokenizer: {}", e);
        // }).expect("Error loading tokenizer //// ");

        let weights_filename = repo.get("model.safetensors")?;
        let vb = if weights_filename.ends_with("model.safetensors") {
            unsafe {
                VarBuilder::from_mmaped_safetensors(&[weights_filename], DType::F32, &device)?
            }
        } else {
            info!("Loading weights from pytorch_model.bin");
            VarBuilder::from_pth(&weights_filename, DType::F32, &device)?
        };

        let config_filename = repo.get("config.json")?;
        let config = std::fs::read_to_string(&config_filename)?;
        let config: Config = serde_json::from_str(&config)?;
        info!("Config loaded : content {:?}", config);

        let labels = {
            let config_json = std::fs::read_to_string(&config_filename)?;
            let config_json: serde_json::Value = serde_json::from_str(&config_json)?;
            let id2label = config_json["id2label"].as_object().ok_or(anyhow::anyhow!("id2label not found in config.json"))?;
            let mut labels = vec!["O".to_string(); id2label.len()];
            for (id, label) in id2label {
                labels[id.parse::<usize>()?] = label.as_str().ok_or(anyhow::anyhow!("label is not a string"))?.to_string();
            }
            labels
        };
        info!("Model loaded");
        info!("Labels: {:?}", labels);

        let model = ModernBertForTokenClassification::load(vb, &config)?;

        Ok(Self {
            model: Mutex::new(model),
            tokenizer: Mutex::new(tokenizer),
            labels,
            device,
        })
    }

    pub async fn detect(&self, input: &InputText) -> Result<PIIResponse> {
        let (max_indices_vec, input_ids, tokenizer_encodings, max_scores_vec) =
            self.compute_logits(input).await;

        let entities = self
            .process_and_merge_entities(input_ids, max_indices_vec, tokenizer_encodings, max_scores_vec)
            .await?;

        Ok(PIIResponse { entities })
    }

    async fn compute_logits(&self, input: &InputText) -> (_, _, _, _) {
        let model = self.model.lock().await;
        let tokenizer = self.tokenizer.lock().await;

        let texts = vec![input.text.as_str()];
        let tokenizer_encodings = tokenizer.encode_batch(texts.clone(), true).unwrap();


        let attention_mask = tokenizer_encodings
            .iter()
            .map(|tokens| {
                let tokens = tokens.get_attention_mask().to_vec();
                CandleTensor::new(tokens.as_slice(), &self.device)
            })
            .collect::<CandleResult<Vec<_>>>()?;
        let attention_mask = CandleTensor::stack(&attention_mask, 0)?;


        let token_ids: CandleResult<Vec<_>> = tokenizer_encodings
            .iter()
            .map(|tk| CandleTensor::new(tk.get_ids(), &self.device))
            .collect();

        let input_ids = CandleTensor::stack(&token_ids?, 0)?;

        // let texts = vec![input.text.as_str()];
        // let input_ids = tokenizer.encode_batch(texts.clone(), true).unwrap();
        //
        // let attention_mask = input_ids.get_attention_mask()?;

        let logits = model.forward(&input_ids, &attention_mask)?;

        let max_scores_vec = softmax(&logits, 2)?.max(2)?.to_vec2::<f32>()?;
        let max_indices_vec: Vec<Vec<u32>> = logits.argmax(2)?.to_vec2()?;
        let input_ids = input_ids.to_vec2::<u32>()?;
        let tokenizer_encodings = tokenizer_encodings.iter().collect::<Vec<_>>();
        let max_scores_vec = max_scores_vec.iter().collect::<Vec<_>>();
        (max_indices_vec, input_ids, tokenizer_encodings, max_scores_vec)
    }

    async fn process_input_ids(
        &self,
        input_ids: Vec<Vec<u32>>,
        max_indices_vec: Vec<Vec<u32>>,
        tokenizer_encodings: Vec<&tokenizers::Encoding>,
        max_scores_vec: Vec<&Vec<f32>>,
    ) -> Result<Vec<Entity>> {
        let mut entities = vec![];
        for (input_row_idx, input_id_row) in input_ids.iter().enumerate() {
            let current_row_encoding = tokenizer_encodings.get(input_row_idx).unwrap();
            let current_row_tokens = current_row_encoding.get_tokens();
            let current_row_max_scores = max_scores_vec.get(input_row_idx).unwrap();
            for (input_id_idx, _input_id) in input_id_row.iter().enumerate() {
                // Do not include special characters in output
                if current_row_encoding.get_special_tokens_mask()[input_id_idx] == 1 {
                    continue;
                }
                let max_label_idx = max_indices_vec
                    .get(input_row_idx)
                    .unwrap()
                    .get(input_id_idx)
                    .unwrap();

                let label = self.labels.clone().get(*max_label_idx as usize).unwrap().clone();

                // Do not include those labeled as "O" ("Other")
                if label == "O" {
                    continue;
                }

                entities.push(Entity {
                    entity: label,
                    word: current_row_tokens[input_id_idx].clone(),
                    score: current_row_max_scores[input_id_idx],
                    start: current_row_encoding.get_offsets()[input_id_idx].0,
                    end: current_row_encoding.get_offsets()[input_id_idx].1,
                    index: input_id_idx,
                });
            }
        }

        // Merge continuous entities
        let mut merged_entities: Vec<Entity> = Vec::new();
        let mut i = 0;
        while i < entities.len() {
            let mut current_entity = entities[i].clone();
            let mut j = i + 1;
            while j < entities.len()
                && entities[j].start == current_entity.end
                && entities[j].entity == current_entity.entity
            {
                current_entity.word.push_str(&entities[j].word);
                current_entity.end = entities[j].end;
                current_entity.score = (current_entity.score + entities[j].score) / 2.0; // Average scores
                j += 1;
            }
            merged_entities.push(current_entity);
            i = j;
        }

        Ok(merged_entities)
    }
}
