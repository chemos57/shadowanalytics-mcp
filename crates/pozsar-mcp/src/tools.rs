use crate::research::answer_research_question;
pub use crate::research::ResearchQuestionParams;
use crate::search::{
    explain_search_chunks_with_filters, read_page_context, search_chunks_with_filters,
    SearchFilters,
};
use anyhow::{Context, Result};
use pozsar_kb::chunk::KnowledgeChunk;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct PozsarCorpusMcp {
    chunks: Arc<Vec<KnowledgeChunk>>,
    tool_router: ToolRouter<Self>,
}

impl PozsarCorpusMcp {
    pub fn new(chunks: Vec<KnowledgeChunk>) -> Self {
        Self {
            chunks: Arc::new(chunks),
            tool_router: Self::tool_router(),
        }
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

#[tool_router]
impl PozsarCorpusMcp {
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
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for PozsarCorpusMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Read-only source-cited search over the local Pozsar PDF corpus.".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
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
