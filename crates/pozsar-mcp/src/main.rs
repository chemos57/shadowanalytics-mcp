use anyhow::Result;
use pozsar_mcp::tools::{
    load_chunks_jsonl, PozsarCorpusMcp, DEFAULT_CHUNKS_JSONL, DEFAULT_MARKET_CONTEXT_HEALTH_JSON,
    DEFAULT_MARKET_CONTEXT_JSON, SERVER_NAME, SERVER_VERSION,
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
    let configured_market_context_path = std::env::var("POZSAR_MARKET_CONTEXT_JSON").ok();
    let using_default_market_context = configured_market_context_path.is_none();
    let market_context_path =
        configured_market_context_path.unwrap_or_else(|| DEFAULT_MARKET_CONTEXT_JSON.to_string());
    let default_health_path_exists = PathBuf::from(DEFAULT_MARKET_CONTEXT_HEALTH_JSON).is_file();
    let market_context_health_path = resolve_market_context_health_path(
        std::env::var("POZSAR_MARKET_CONTEXT_HEALTH_JSON").ok(),
        using_default_market_context,
        default_health_path_exists,
    );
    let chunks = load_chunks_jsonl(&chunks_path)?;
    tracing::info!(
        server_name = SERVER_NAME,
        server_version = SERVER_VERSION,
        chunks_path = %chunks_path.display(),
        market_context_path = %market_context_path,
        market_context_health_path = ?market_context_health_path,
        chunk_count = chunks.len(),
        "starting Pozsar corpus MCP server"
    );
    let mut server = PozsarCorpusMcp::new(chunks)
        .with_chunks_path(chunks_path.display().to_string())
        .with_market_context_path(market_context_path);
    if let Some(market_context_health_path) = market_context_health_path {
        server = server.with_market_context_health_path(market_context_health_path);
    }
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn resolve_market_context_health_path(
    explicit_health_path: Option<String>,
    using_default_market_context: bool,
    default_health_path_exists: bool,
) -> Option<String> {
    explicit_health_path.or_else(|| {
        (using_default_market_context && default_health_path_exists)
            .then(|| DEFAULT_MARKET_CONTEXT_HEALTH_JSON.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_market_context_does_not_use_default_health_sidecar() {
        assert_eq!(resolve_market_context_health_path(None, false, true), None);
    }

    #[test]
    fn default_market_context_uses_default_health_sidecar_when_present() {
        assert_eq!(
            resolve_market_context_health_path(None, true, true),
            Some(DEFAULT_MARKET_CONTEXT_HEALTH_JSON.to_string())
        );
    }

    #[test]
    fn explicit_market_context_health_path_wins_for_custom_context() {
        assert_eq!(
            resolve_market_context_health_path(Some("custom.health.json".to_string()), false, true),
            Some("custom.health.json".to_string())
        );
    }
}
