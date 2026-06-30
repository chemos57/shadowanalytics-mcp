use anyhow::Result;
use pozsar_mcp::tools::{
    load_chunks_jsonl, PozsarCorpusMcp, DEFAULT_CHUNKS_JSONL, DEFAULT_MARKET_CONTEXT_JSON,
    SERVER_NAME, SERVER_VERSION,
};
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("pozsar-mcp {SERVER_VERSION}");
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let chunks_path = std::env::var("POZSAR_CHUNKS_JSONL")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_CHUNKS_JSONL));
    let market_context_path = std::env::var("POZSAR_MARKET_CONTEXT_JSON")
        .unwrap_or_else(|_| DEFAULT_MARKET_CONTEXT_JSON.to_string());
    let chunks = load_chunks_jsonl(&chunks_path)?;
    tracing::info!(
        server_name = SERVER_NAME,
        server_version = SERVER_VERSION,
        chunks_path = %chunks_path.display(),
        market_context_path = %market_context_path,
        chunk_count = chunks.len(),
        "starting Pozsar corpus MCP server"
    );
    let service = PozsarCorpusMcp::new(chunks)
        .with_chunks_path(chunks_path.display().to_string())
        .with_market_context_path(market_context_path)
        .serve(stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
