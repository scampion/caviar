use axum_macros;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
    body::Bytes,
};
use lopdf::{Document, Object};
use std::io::Cursor;
use env_logger;
use log::info;
use log::debug;
use std::sync::Arc;
use clap::Parser;
use pdf_extract;

mod piidetect;
use piidetect::{InputText, PIIResponse, PIIReplacementResponse, PiiDetector};

struct AppState {
    detector: Arc<PiiDetector>,
}

/// PII detection server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Host to listen on
    #[arg(short = 'i', long, default_value = "0.0.0.0")]
    host: String,
}

#[axum_macros::debug_handler]
async fn detect_pii(
    State(state): State<Arc<AppState>>,
    Json(input): Json<InputText>,
) -> Result<Json<PIIResponse>, StatusCode> {
    match state.detector.detect(&input).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            tracing::error!("Error detecting PII: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn detect_and_replace_pii(
    State(state): State<Arc<AppState>>,
    Json(input): Json<InputText>,
) -> Result<Json<PIIReplacementResponse>, StatusCode> {
    match state.detector.detect_and_replace(&input).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            tracing::error!("Error detecting and replacing PII: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn detect_and_replace_pii_pdf(
    State(state): State<Arc<AppState>>,
    pdf_bytes: Bytes,
) -> Result<Vec<u8>, StatusCode> {

    let mut doc = match Document::load_from(Cursor::new(pdf_bytes.as_ref())) {
        Ok(doc) => doc,
        Err(e) => {
            tracing::error!("Error loading PDF: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let pages = pdf_extract::extract_text_from_mem_by_pages(&pdf_bytes).unwrap();
    for (i, page) in pages.iter().enumerate() {
        println!("Page {}: {}", i + 1, page);
        // Detect and replace PII
        let text = page.to_string();

        let input = InputText { text };
        let response = state.detector.detect(&input).await.unwrap();
        let mut text = input.text.clone();
        // Replace entities in reverse order to avoid messing up indices
        for entity in response.entities.iter().rev() {
            doc.replace_text(i, &entity.word, format!("[{}]", entity.entity));

            //let placeholder = format!("[{}]", entity.entity);
            //text.replace_range(entity.start..entity.end, &placeholder);

        }

    }

    // Load PDF from bytes

    // let mut doc = Document::load("test.pdf").unwrap();
    //
    // // Process each page
    // for (page_num, page_id) in doc.get_pages() {
    //     // use  extract_text_chunks(&self, page_numbers: &[u32]) -> Vec<Result<String>>
    //
    //     // for chunk in doc.extract_text_chunks(&[page_num]) {
    //     //     match chunk {
    //     //         Ok(text) => {
    //     //             debug!("Extracted chunk: {}", text);
    //     //         }
    //     //         Err(e) => {
    //     //             tracing::error!("Error extracting PDF text: {}", e);
    //     //             return Err(StatusCode::INTERNAL_SERVER_ERROR);
    //     //         }
    //     //     }
    //     // }
    //
    //     let text = doc.extract_text(&[page_num]).unwrap();
    //
    //     // Detect and replace PII
    //     // let sanitized = match state.detector.detect_and_replace(&InputText { text }).await {
    //     // Ok(response) => response.sanitized_text,
    //     // Err(e) => {
    //     //     tracing::error!("Error processing PDF text: {}", e);
    //     //     return Err(StatusCode::INTERNAL_SERVER_ERROR);
    //     // }
    //     // };
    //     let sanitized = text;
    //
    //     // debug!("Sanitized text: {}", sanitized);
    //
    //     // Replace page content with sanitized text
    //     if let Err(e) = replace_page_text(&mut doc, (page_id.0, page_id.1 as u32), &sanitized) {
    //         tracing::error!("Error replacing PDF text: {}", e);
    //         return Err(StatusCode::INTERNAL_SERVER_ERROR);
    //     }
    // }

    // Save modified PDF to bytes
    let mut output = Vec::new();
    if let Err(e) = doc.save_to(&mut output) {
        tracing::error!("Error saving PDF: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(output)
}



#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    info!("Starting server");
    let detector = PiiDetector::new()?;

    let shared_state = Arc::new(AppState {
        detector: Arc::new(detector),
    });

    let app = Router::new()
        .route("/detect_pii", post(detect_pii))
        .route("/detect_and_replace_pii", post(detect_and_replace_pii))
        .route("/detect_and_replace_pii_pdf", post(detect_and_replace_pii_pdf))
        .route("/", get(|| async { "PII Detection" }))
        .with_state(shared_state);

    let args = Args::parse();
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("listening on {}", listener.local_addr()?);
    println!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}


