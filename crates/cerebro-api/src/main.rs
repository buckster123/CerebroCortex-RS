use std::sync::Arc;

use anyhow::Result;
use axum::{routing::get, Router};
use cerebro::CerebroCortex;
use tracing::info;

/// cerebro-api — axum REST API + dashboard.
/// Optional; cerebro-mcp is the primary integration point.
/// Mirrors Python interfaces/api_server.py.
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = cerebro::config::Config::from_env()?;
    let brain  = Arc::new(CerebroCortex::new(config).await?);

    let app = Router::new()
        .route("/health", get(health))
        // TODO: full API routes in build-order step 11
        .with_state(brain);

    let addr = std::env::var("CEREBRO_API_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8765".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("cerebro-api listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str { "ok" }
