use anyhow::{Context, Result};
use pozsar_kb::chunk::KnowledgeChunk;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
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
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadSourceParams {
    pub doc_id: String,
    pub page: u32,
}

#[derive(Debug, Serialize)]
pub struct CorpusDocument {
    pub doc_id: String,
    pub file_name: String,
    pub chunks: usize,
}

#[derive(Debug, Serialize)]
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
        let passages = search_chunks(
            &self.chunks,
            &params.query,
            params.limit.unwrap_or(5).clamp(1, 10) as usize,
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

pub fn search_chunks(
    chunks: &[KnowledgeChunk],
    query: &str,
    limit: usize,
) -> Vec<SourceCitedPassage> {
    let terms: Vec<String> = query
        .to_ascii_lowercase()
        .split_whitespace()
        .map(|term| {
            term.trim_matches(|ch: char| !ch.is_ascii_alphanumeric())
                .to_string()
        })
        .filter(|term| term.len() >= 3)
        .collect();

    let mut scored: Vec<(usize, &KnowledgeChunk)> = chunks
        .iter()
        .filter_map(|chunk| {
            let haystack =
                format!("{} {}", chunk.text, chunk.themes.join(" ")).to_ascii_lowercase();
            let score = terms
                .iter()
                .filter(|term| haystack.contains(term.as_str()))
                .count();
            (score > 0).then_some((score, chunk))
        })
        .collect();

    scored.sort_by_key(|(score, chunk)| {
        (
            Reverse(*score),
            chunk.file_name.clone(),
            chunk.page,
            chunk.chunk_index,
        )
    });
    scored
        .into_iter()
        .take(limit)
        .map(|(_, chunk)| source_cited_passage(chunk))
        .collect()
}

fn source_cited_passage(chunk: &KnowledgeChunk) -> SourceCitedPassage {
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
