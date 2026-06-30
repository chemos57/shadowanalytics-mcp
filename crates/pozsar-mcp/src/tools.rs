use crate::research::answer_research_question;
pub use crate::research::ResearchQuestionParams;
use crate::search::{
    explain_search_chunks_with_filters, read_page_context, search_chunks_with_filters,
    SearchFilters,
};
use crate::signals::extract_liquidity_signals;
pub use crate::signals::LiquiditySignalParams;
use advisor_core::build_advisor_snapshot_with_health;
use anyhow::{Context, Result};
use market_context::{MarketContext, MarketDataHealth};
use pozsar_kb::chunk::KnowledgeChunk;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub const SERVER_NAME: &str = "pozsar-corpus";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_CHUNKS_JSONL: &str = "data/knowledge/chunks/pozsar_chunks.jsonl";
pub const DEFAULT_MARKET_CONTEXT_JSON: &str = "data/market/context.json";
pub const DEFAULT_MARKET_CONTEXT_HEALTH_JSON: &str = "data/market/context.health.json";

#[derive(Clone)]
pub struct PozsarCorpusMcp {
    chunks: Arc<Vec<KnowledgeChunk>>,
    chunks_path: Option<String>,
    market_context_path: Option<String>,
    market_context_health_path: Option<String>,
    tool_router: ToolRouter<Self>,
}

impl PozsarCorpusMcp {
    pub fn new(chunks: Vec<KnowledgeChunk>) -> Self {
        Self {
            chunks: Arc::new(chunks),
            chunks_path: None,
            market_context_path: None,
            market_context_health_path: None,
            tool_router: Self::tool_router(),
        }
    }

    pub fn with_chunks_path(mut self, chunks_path: impl Into<String>) -> Self {
        self.chunks_path = Some(chunks_path.into());
        self
    }

    pub fn with_market_context_path(mut self, market_context_path: impl ToString) -> Self {
        self.market_context_path = Some(market_context_path.to_string());
        self
    }

    pub fn with_market_context_health_path(
        mut self,
        market_context_health_path: impl ToString,
    ) -> Self {
        self.market_context_health_path = Some(market_context_health_path.to_string());
        self
    }

    fn status(&self) -> PozsarKbStatus {
        let documents: BTreeSet<&str> = self
            .chunks
            .iter()
            .map(|chunk| chunk.doc_id.as_str())
            .collect();
        let citations: BTreeSet<&str> = self
            .chunks
            .iter()
            .map(|chunk| chunk.citation.as_str())
            .collect();
        let themes: BTreeSet<&str> = self
            .chunks
            .iter()
            .flat_map(|chunk| chunk.themes.iter().map(String::as_str))
            .collect();

        PozsarKbStatus {
            server_name: SERVER_NAME,
            server_version: SERVER_VERSION,
            default_chunks_jsonl: DEFAULT_CHUNKS_JSONL,
            default_market_context_json: DEFAULT_MARKET_CONTEXT_JSON,
            default_market_context_health_json: DEFAULT_MARKET_CONTEXT_HEALTH_JSON,
            chunks_path: self.chunks_path.clone(),
            market_context_path: self.market_context_path.clone(),
            market_context_health_path: self.market_context_health_path.clone(),
            chunk_count: self.chunks.len(),
            document_count: documents.len(),
            citation_count: citations.len(),
            theme_count: themes.len(),
            tools: vec![
                "get_pozsar_kb_status",
                "list_pozsar_docs",
                "list_pozsar_themes",
                "search_pozsar_kb",
                "explain_pozsar_search",
                "read_pozsar_source",
                "read_pozsar_page_context",
                "answer_pozsar_research_question",
                "extract_pozsar_liquidity_signals",
                "get_pozsar_advisor_snapshot",
            ],
        }
    }

    fn advisor_snapshot(
        &self,
        params: AdvisorSnapshotParams,
    ) -> Result<advisor_core::AdvisorSnapshot> {
        let uses_request_market_context = params.market_context_path.is_some();
        let market_context_path = if let Some(market_context_path) =
            params.market_context_path.clone()
        {
            market_context_path
        } else {
            self.market_context_path.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "market context path is required via market_context_path or POZSAR_MARKET_CONTEXT_JSON"
                )
            })?
        };
        let market_context = load_market_context_json(Path::new(&market_context_path))?;
        let market_context_health_path = params.market_context_health_path.clone().or_else(|| {
            (!uses_request_market_context)
                .then(|| self.market_context_health_path.clone())
                .flatten()
        });
        let market_context_health = market_context_health_path
            .as_deref()
            .map(|path| load_market_context_health_json(Path::new(path)))
            .transpose()?;
        let liquidity_signals = extract_liquidity_signals(
            &self.chunks,
            LiquiditySignalParams {
                question: params.question.clone(),
                assets: params.assets,
                themes: params.themes,
                limit: params.limit,
            },
        );

        Ok(build_advisor_snapshot_with_health(
            params.question,
            liquidity_signals,
            market_context,
            market_context_health,
        )?)
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchPozsarParams {
    pub query: String,
    pub limit: Option<u64>,
    pub theme: Option<String>,
    pub doc_id: Option<String>,
    pub file_name: Option<String>,
    pub page: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadSourceParams {
    pub doc_id: String,
    pub page: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadPageContextParams {
    pub doc_id: String,
    pub page: u32,
    pub radius: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AdvisorSnapshotParams {
    pub question: String,
    pub assets: Vec<String>,
    pub themes: Option<Vec<String>>,
    pub market_context_path: Option<String>,
    pub market_context_health_path: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct CorpusDocument {
    pub doc_id: String,
    pub file_name: String,
    pub chunks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceCitedPassage {
    pub doc_id: String,
    pub file_name: String,
    pub page: u32,
    pub chunk_index: u32,
    pub themes: Vec<String>,
    pub text: String,
    pub citation: String,
}

#[derive(Debug, Serialize)]
pub struct PozsarKbStatus {
    pub server_name: &'static str,
    pub server_version: &'static str,
    pub default_chunks_jsonl: &'static str,
    pub default_market_context_json: &'static str,
    pub default_market_context_health_json: &'static str,
    pub chunks_path: Option<String>,
    pub market_context_path: Option<String>,
    pub market_context_health_path: Option<String>,
    pub chunk_count: usize,
    pub document_count: usize,
    pub citation_count: usize,
    pub theme_count: usize,
    pub tools: Vec<&'static str>,
}

#[tool_router]
impl PozsarCorpusMcp {
    #[tool(
        description = "Return MCP server metadata and local Pozsar corpus artifact counts. Read-only."
    )]
    pub fn get_pozsar_kb_status(&self) -> String {
        serde_json::to_string_pretty(&self.status()).unwrap()
    }

    #[tool(description = "List documents available in the local Pozsar corpus. Read-only.")]
    pub fn list_pozsar_docs(&self) -> String {
        let mut docs: BTreeMap<String, CorpusDocument> = BTreeMap::new();
        for chunk in self.chunks.iter() {
            docs.entry(chunk.doc_id.clone())
                .and_modify(|doc| doc.chunks += 1)
                .or_insert_with(|| CorpusDocument {
                    doc_id: chunk.doc_id.clone(),
                    file_name: chunk.file_name.clone(),
                    chunks: 1,
                });
        }
        serde_json::to_string_pretty(&docs.into_values().collect::<Vec<_>>()).unwrap()
    }

    #[tool(description = "List deterministic themes found in the local Pozsar corpus. Read-only.")]
    pub fn list_pozsar_themes(&self) -> String {
        let themes: BTreeSet<String> = self
            .chunks
            .iter()
            .flat_map(|chunk| chunk.themes.iter().cloned())
            .collect();
        serde_json::to_string_pretty(&themes.into_iter().collect::<Vec<_>>()).unwrap()
    }

    #[tool(
        description = "Search the Pozsar corpus and return source-cited passages with file name and page. Read-only."
    )]
    pub fn search_pozsar_kb(&self, Parameters(params): Parameters<SearchPozsarParams>) -> String {
        let filters = search_filters_from_params(&params);
        let passages = search_chunks_with_filters(
            &self.chunks,
            &params.query,
            params.limit.unwrap_or(5).clamp(1, 10) as usize,
            &filters,
        );
        serde_json::to_string_pretty(&passages).unwrap()
    }

    #[tool(
        description = "Search the Pozsar corpus and return source-cited passages with scoring explanations. Read-only."
    )]
    pub fn explain_pozsar_search(
        &self,
        Parameters(params): Parameters<SearchPozsarParams>,
    ) -> String {
        let filters = search_filters_from_params(&params);
        let passages = explain_search_chunks_with_filters(
            &self.chunks,
            &params.query,
            params.limit.unwrap_or(5).clamp(1, 10) as usize,
            &filters,
        );
        serde_json::to_string_pretty(&passages).unwrap()
    }

    #[tool(description = "Read all extracted chunks for one source document page. Read-only.")]
    pub fn read_pozsar_source(&self, Parameters(params): Parameters<ReadSourceParams>) -> String {
        let passages: Vec<SourceCitedPassage> = self
            .chunks
            .iter()
            .filter(|chunk| chunk.doc_id == params.doc_id && chunk.page == params.page)
            .map(source_cited_passage)
            .collect();
        serde_json::to_string_pretty(&passages).unwrap()
    }

    #[tool(
        description = "Read source-cited chunks around one document page, including neighboring pages. Read-only."
    )]
    pub fn read_pozsar_page_context(
        &self,
        Parameters(params): Parameters<ReadPageContextParams>,
    ) -> String {
        let passages = read_page_context(&self.chunks, &params.doc_id, params.page, params.radius);
        serde_json::to_string_pretty(&passages).unwrap()
    }

    #[tool(
        description = "Build a compact source-cited evidence bundle for a Pozsar corpus research question. Returns evidence only, not a generated answer. Read-only."
    )]
    pub fn answer_pozsar_research_question(
        &self,
        Parameters(params): Parameters<ResearchQuestionParams>,
    ) -> String {
        serde_json::to_string_pretty(&answer_research_question(&self.chunks, params)).unwrap()
    }

    #[tool(
        description = "Extract deterministic, evidence-only macro liquidity signals and cross-asset implications from the Pozsar corpus. Does not generate trade recommendations. Read-only."
    )]
    pub fn extract_pozsar_liquidity_signals(
        &self,
        Parameters(params): Parameters<LiquiditySignalParams>,
    ) -> String {
        serde_json::to_string_pretty(&extract_liquidity_signals(&self.chunks, params)).unwrap()
    }

    #[tool(
        description = "Build a deterministic advisor snapshot from Pozsar corpus liquidity signals plus offline market context. Does not generate trade recommendations. Read-only."
    )]
    pub fn get_pozsar_advisor_snapshot(
        &self,
        Parameters(params): Parameters<AdvisorSnapshotParams>,
    ) -> String {
        match self.advisor_snapshot(params) {
            Ok(snapshot) => serde_json::to_string_pretty(&snapshot).unwrap(),
            Err(error) => serde_json::to_string_pretty(&serde_json::json!({
                "error": error.to_string()
            }))
            .unwrap(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for PozsarCorpusMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Read-only source-cited search over the local Pozsar PDF corpus.".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: SERVER_NAME.into(),
                title: Some("Pozsar Corpus MCP".into()),
                version: SERVER_VERSION.into(),
                icons: None,
                website_url: None,
            },
            ..Default::default()
        }
    }
}

pub fn load_chunks_jsonl(path: &Path) -> Result<Vec<KnowledgeChunk>> {
    let jsonl = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    jsonl
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<KnowledgeChunk>(line)
                .with_context(|| format!("parse chunk jsonl line {}", index + 1))
        })
        .collect()
}

pub fn load_market_context_json(path: &Path) -> Result<MarketContext> {
    let json = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str::<MarketContext>(&json)
        .with_context(|| format!("parse market context {}", path.display()))
}

pub fn load_market_context_health_json(path: &Path) -> Result<MarketDataHealth> {
    let json = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str::<MarketDataHealth>(&json)
        .with_context(|| format!("parse market context health {}", path.display()))
}

pub fn source_cited_passage(chunk: &KnowledgeChunk) -> SourceCitedPassage {
    SourceCitedPassage {
        doc_id: chunk.doc_id.clone(),
        file_name: chunk.file_name.clone(),
        page: chunk.page,
        chunk_index: chunk.chunk_index,
        themes: chunk.themes.clone(),
        text: chunk.text.clone(),
        citation: chunk.citation.clone(),
    }
}

fn search_filters_from_params(params: &SearchPozsarParams) -> SearchFilters {
    SearchFilters {
        theme: params.theme.clone(),
        doc_id: params.doc_id.clone(),
        file_name: params.file_name.clone(),
        page: params.page,
    }
}
