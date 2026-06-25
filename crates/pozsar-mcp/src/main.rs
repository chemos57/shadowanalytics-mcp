use anyhow::Result;
use pozsar_mcp::tools::{load_chunks_jsonl, PozsarCorpusMcp};
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let chunks_path = std::env::var("POZSAR_CHUNKS_JSONL")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/knowledge/chunks/pozsar_chunks.jsonl"));
    let chunks = load_chunks_jsonl(&chunks_path)?;
    let service = PozsarCorpusMcp::new(chunks).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
