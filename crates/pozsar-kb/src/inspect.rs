use crate::artifacts::{read_chunks_jsonl, read_manifest, read_pages_jsonl};
use crate::chunk::KnowledgeChunk;
use crate::extract::ExtractedPage;
use anyhow::Result;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorpusInspection {
    pub document_count: usize,
    pub extracted_page_count: usize,
    pub chunk_count: usize,
    pub empty_page_count: usize,
    pub empty_page_ratio: f64,
    pub pages_without_chunks: Vec<PageInspectionRef>,
    pub theme_counts: Vec<ThemeCount>,
    pub validation_issues: Vec<String>,
}

impl CorpusInspection {
    pub fn has_validation_issues(&self) -> bool {
        !self.validation_issues.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageInspectionRef {
    pub doc_id: String,
    pub file_name: String,
    pub page: u32,
    pub citation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemeCount {
    pub theme: String,
    pub chunks: usize,
}

pub fn inspect_artifacts(knowledge_dir: &Path) -> Result<CorpusInspection> {
    let manifest = read_manifest(&knowledge_dir.join("manifest.json"))?;
    let pages = read_pages_jsonl(&knowledge_dir.join("extracted_pages.jsonl"))?;
    let chunks = read_chunks_jsonl(&knowledge_dir.join("chunks/pozsar_chunks.jsonl"))?;

    Ok(inspect_loaded_artifacts(&manifest, &pages, &chunks))
}

pub fn inspect_loaded_artifacts(
    manifest: &[crate::manifest::PdfManifestEntry],
    pages: &[ExtractedPage],
    chunks: &[KnowledgeChunk],
) -> CorpusInspection {
    let page_keys = pages
        .iter()
        .map(|page| (page.doc_id.clone(), page.page))
        .collect::<BTreeSet<_>>();
    let chunk_page_keys = chunks
        .iter()
        .map(|chunk| (chunk.doc_id.clone(), chunk.page))
        .collect::<BTreeSet<_>>();

    let empty_page_count = pages
        .iter()
        .filter(|page| page.text.trim().is_empty())
        .count();
    let pages_without_chunks = pages_without_chunks(pages, &chunk_page_keys);
    let mut validation_issues = validation_issues(pages, chunks, &page_keys, &pages_without_chunks);

    validation_issues.sort();

    CorpusInspection {
        document_count: manifest.len(),
        extracted_page_count: pages.len(),
        chunk_count: chunks.len(),
        empty_page_count,
        empty_page_ratio: ratio(empty_page_count, pages.len()),
        pages_without_chunks,
        theme_counts: theme_counts(chunks),
        validation_issues,
    }
}

fn pages_without_chunks(
    pages: &[ExtractedPage],
    chunk_page_keys: &BTreeSet<(String, u32)>,
) -> Vec<PageInspectionRef> {
    pages
        .iter()
        .filter(|page| !page.text.trim().is_empty())
        .filter(|page| !chunk_page_keys.contains(&(page.doc_id.clone(), page.page)))
        .map(|page| PageInspectionRef {
            doc_id: page.doc_id.clone(),
            file_name: page.file_name.clone(),
            page: page.page,
            citation: format!("{}:{}", page.file_name, page.page),
        })
        .collect()
}

fn validation_issues(
    pages: &[ExtractedPage],
    chunks: &[KnowledgeChunk],
    page_keys: &BTreeSet<(String, u32)>,
    pages_without_chunks: &[PageInspectionRef],
) -> Vec<String> {
    let page_file_names = pages
        .iter()
        .map(|page| ((page.doc_id.clone(), page.page), page.file_name.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut issues = Vec::new();

    for page in pages_without_chunks {
        issues.push(format!("page {} has text but no chunks", page.citation));
    }

    for chunk in chunks {
        let key = (chunk.doc_id.clone(), chunk.page);
        if !page_keys.contains(&key) {
            issues.push(format!(
                "chunk source page {}:{} is missing",
                chunk.doc_id, chunk.page
            ));
        }

        let expected_file_name = page_file_names
            .get(&key)
            .map(String::as_str)
            .unwrap_or(chunk.file_name.as_str());
        let expected_citation = format!("{}:{}", expected_file_name, chunk.page);
        if chunk.citation != expected_citation {
            issues.push(format!(
                "chunk citation {} should be {}",
                chunk.citation, expected_citation
            ));
        }
    }

    issues
}

fn theme_counts(chunks: &[KnowledgeChunk]) -> Vec<ThemeCount> {
    let mut counts = BTreeMap::<String, usize>::new();
    for theme in chunks.iter().flat_map(|chunk| chunk.themes.iter()) {
        *counts.entry(theme.clone()).or_default() += 1;
    }

    let mut counts = counts
        .into_iter()
        .map(|(theme, chunks)| ThemeCount { theme, chunks })
        .collect::<Vec<_>>();
    counts.sort_by_key(|count| (Reverse(count.chunks), count.theme.clone()));
    counts
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}
