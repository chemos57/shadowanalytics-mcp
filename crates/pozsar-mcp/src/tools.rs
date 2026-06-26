use crate::search::{
    explain_search_chunks_with_filters, read_page_context, search_chunks_with_filters,
    ScoreBreakdown, SearchFilters,
};
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
pub struct ResearchQuestionParams {
    pub question: String,
    pub themes: Option<Vec<String>>,
    pub doc_id: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
pub struct ResearchQueryPlanStep {
    pub kind: String,
    pub query: String,
    pub theme: Option<String>,
    pub doc_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResearchEvidence {
    pub citation: String,
    pub passage: SourceCitedPassage,
    pub score: usize,
    pub score_breakdown: ScoreBreakdown,
    pub snippet: Option<String>,
    pub query_sources: Vec<String>,
    pub context: Vec<SourceCitedPassage>,
}

#[derive(Debug, Serialize)]
pub struct ResearchQuestionBundle {
    pub question: String,
    pub query_plan: Vec<ResearchQueryPlanStep>,
    pub evidence: Vec<ResearchEvidence>,
    pub citations: Vec<String>,
    pub suggested_followups: Vec<String>,
}

struct ResearchCandidate {
    passage: SourceCitedPassage,
    score: usize,
    score_breakdown: ScoreBreakdown,
    snippet: Option<String>,
    query_sources: Vec<String>,
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
        let limit = params.limit.unwrap_or(5).clamp(1, 10) as usize;
        let query_plan = research_query_plan(
            &params.question,
            params.themes.as_deref(),
            params.doc_id.as_deref(),
        );
        let mut candidates = Vec::<ResearchCandidate>::new();

        for step in &query_plan {
            let filters = SearchFilters {
                theme: step.theme.clone(),
                doc_id: params.doc_id.clone(),
                ..Default::default()
            };
            let results =
                explain_search_chunks_with_filters(&self.chunks, &step.query, limit, &filters);

            for result in results {
                upsert_research_candidate(
                    &mut candidates,
                    result.passage,
                    result.score,
                    result.score_breakdown,
                    result.snippet,
                    &step.kind,
                );
            }
        }
        candidates.sort_by_key(|candidate| {
            (
                Reverse(candidate.score),
                candidate.passage.citation.clone(),
                candidate.passage.chunk_index,
            )
        });

        let evidence = candidates
            .into_iter()
            .take(limit)
            .map(|candidate| {
                let context = read_page_context(
                    &self.chunks,
                    &candidate.passage.doc_id,
                    candidate.passage.page,
                    Some(1),
                );
                ResearchEvidence {
                    citation: candidate.passage.citation.clone(),
                    passage: candidate.passage,
                    score: candidate.score,
                    score_breakdown: candidate.score_breakdown,
                    snippet: candidate.snippet,
                    query_sources: candidate.query_sources,
                    context,
                }
            })
            .collect::<Vec<_>>();
        let citations = evidence
            .iter()
            .map(|item| item.citation.clone())
            .collect::<Vec<_>>();
        let suggested_followups = suggested_followups(
            params.themes.as_deref(),
            params.doc_id.as_deref(),
            &evidence,
        );

        serde_json::to_string_pretty(&ResearchQuestionBundle {
            question: params.question,
            query_plan,
            evidence,
            citations,
            suggested_followups,
        })
        .unwrap()
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

fn research_query_plan(
    question: &str,
    themes: Option<&[String]>,
    doc_id: Option<&str>,
) -> Vec<ResearchQueryPlanStep> {
    let mut plan = vec![ResearchQueryPlanStep {
        kind: "original_question".to_string(),
        query: question.to_string(),
        theme: None,
        doc_id: doc_id.map(str::to_string),
    }];

    if let Some(key_phrase) = key_phrase_query(question) {
        plan.push(ResearchQueryPlanStep {
            kind: "key_phrase".to_string(),
            query: key_phrase,
            theme: None,
            doc_id: doc_id.map(str::to_string),
        });
    }

    for theme in themes.unwrap_or_default() {
        plan.push(ResearchQueryPlanStep {
            kind: "theme_filtered".to_string(),
            query: question.to_string(),
            theme: Some(theme.clone()),
            doc_id: doc_id.map(str::to_string),
        });
    }

    plan
}

fn key_phrase_query(question: &str) -> Option<String> {
    let terms = normalized_terms(question)
        .into_iter()
        .filter(|term| term.len() >= 3)
        .filter(|term| !QUESTION_STOPWORDS.contains(&term.as_str()))
        .take(8)
        .collect::<Vec<_>>();

    (terms.len() >= 2).then(|| terms.join(" "))
}

fn normalized_terms(text: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(text.len());
    let mut last_was_space = true;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }

    normalized.split_whitespace().map(str::to_string).collect()
}

fn upsert_research_candidate(
    candidates: &mut Vec<ResearchCandidate>,
    passage: SourceCitedPassage,
    score: usize,
    score_breakdown: ScoreBreakdown,
    snippet: Option<String>,
    query_source: &str,
) {
    if let Some(existing) = candidates.iter_mut().find(|candidate| {
        candidate.passage.doc_id == passage.doc_id && candidate.passage.page == passage.page
    }) {
        push_unique(&mut existing.query_sources, query_source.to_string());
        if score > existing.score {
            existing.score = score;
            existing.score_breakdown = score_breakdown;
            existing.snippet = snippet;
            existing.passage = passage;
        }
        return;
    }

    candidates.push(ResearchCandidate {
        passage,
        score,
        score_breakdown,
        snippet,
        query_sources: vec![query_source.to_string()],
    });
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn suggested_followups(
    themes: Option<&[String]>,
    doc_id: Option<&str>,
    evidence: &[ResearchEvidence],
) -> Vec<String> {
    let mut followups = Vec::new();
    for theme in themes.unwrap_or_default() {
        push_unique(
            &mut followups,
            format!("Search adjacent pages for how {theme} connects to the question."),
        );
    }

    let evidence_themes = evidence
        .iter()
        .flat_map(|item| item.passage.themes.iter())
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    for theme in evidence_themes {
        push_unique(
            &mut followups,
            format!("Compare {theme} evidence across distinct source pages."),
        );
    }

    if doc_id.is_some() {
        followups
            .push("Repeat without doc_id to compare evidence across the full corpus.".to_string());
    } else {
        followups
            .push("Add a doc_id filter to inspect one source document more deeply.".to_string());
    }

    followups.truncate(5);
    followups
}

const QUESTION_STOPWORDS: &[&str] = &[
    "about", "affect", "after", "also", "among", "does", "from", "have", "into", "over", "that",
    "their", "them", "then", "there", "these", "this", "through", "what", "when", "where", "which",
    "while", "with", "would",
];
