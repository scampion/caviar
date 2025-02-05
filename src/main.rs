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
use std::sync::Arc;
use clap::Parser;

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
    // Load PDF from bytes
    let mut doc = match Document::load_from(Cursor::new(pdf_bytes.as_ref())) {
        Ok(doc) => doc,
        Err(e) => {
            tracing::error!("Error loading PDF: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Process each page
    for (page_num, page_id) in doc.get_pages() {
        if let Some(content) = doc.get_page_content(page_id) {
            // Extract text from PDF content
            let text = extract_text_from_content(content);
            // Detect and replace PII
            let sanitized = match state.detector.detect_and_replace(&InputText { text }).await {
                Ok(response) => response.sanitized_text,
                Err(e) => {
                    tracing::error!("Error processing PDF text: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            };

            // Replace page content with sanitized text
            if let Err(e) = replace_page_text(&mut doc, page_id, &sanitized) {
                tracing::error!("Error replacing PDF text: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // Save modified PDF to bytes
    let mut output = Vec::new();
    if let Err(e) = doc.save_to(&mut output) {
        tracing::error!("Error saving PDF: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(output)
}

fn extract_text_from_content(content: &Vec<Object>) -> String {
    // Convert PDF content objects to text
    content.iter()
        .filter_map(|obj| {
            if let Object::String(s, _) = obj {
                Some(s.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn replace_page_text(doc: &mut Document, page_id: (u32, u32), text: &str) -> Result<(), lopdf::Error> {
    // Create new content with sanitized text
    let new_content = vec![Object::string_literal(text)];
    
    // Replace page content
    doc.change_page_content(page_id, new_content)?;
    
    Ok(())
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


