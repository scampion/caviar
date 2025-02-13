use axum_macros;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use env_logger;
use log::info;
use std::sync::Arc;

mod piidetect;
use piidetect::{InputText, PIIResponse, PiiDetector};

struct AppState {
    detector: Arc<PiiDetector>,
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
        .route("/", get(|| async { "PII Detection" }))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    info!("listening on {}", listener.local_addr()?);
    println!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}


