use crate::tools::SourceCitedPassage;
use pozsar_kb::chunk::KnowledgeChunk;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BTreeSet;

#[derive(Debug)]
struct ScoredChunk<'a> {
    score: usize,
    chunk: &'a KnowledgeChunk,
    phrase_hits: Vec<String>,
    term_hits: Vec<TermHit>,
    title_boosts: Vec<String>,
    theme_boosts: Vec<String>,
    citation_boosts: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchFilters {
    pub theme: Option<String>,
    pub doc_id: Option<String>,
    pub file_name: Option<String>,
    pub page: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExplainedSearchPassage {
    pub passage: SourceCitedPassage,
    pub score: usize,
    pub phrase_hits: Vec<String>,
    pub term_hits: Vec<TermHit>,
    pub title_boosts: Vec<String>,
    pub theme_boosts: Vec<String>,
    pub citation_boosts: Vec<String>,
    pub duplicate_citation: bool,
    pub citation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TermHit {
    pub term: String,
    pub text_count: usize,
    pub title_count: usize,
    pub theme_count: usize,
    pub citation_count: usize,
}

#[derive(Debug, Default)]
struct ScoreDetails {
    score: usize,
    phrase_hits: Vec<String>,
    term_hits: Vec<TermHit>,
    title_boosts: Vec<String>,
    theme_boosts: Vec<String>,
    citation_boosts: Vec<String>,
}

pub fn search_chunks(
    chunks: &[KnowledgeChunk],
    query: &str,
    limit: usize,
) -> Vec<SourceCitedPassage> {
    search_chunks_with_filters(chunks, query, limit, &SearchFilters::default())
}

pub fn search_chunks_with_filters(
    chunks: &[KnowledgeChunk],
    query: &str,
    limit: usize,
    filters: &SearchFilters,
) -> Vec<SourceCitedPassage> {
    explain_search_chunks_with_filters(chunks, query, limit, filters)
        .into_iter()
        .map(|result| result.passage)
        .collect()
}

pub fn explain_search_chunks_with_filters(
    chunks: &[KnowledgeChunk],
    query: &str,
    limit: usize,
    filters: &SearchFilters,
) -> Vec<ExplainedSearchPassage> {
    let query_terms = tokenize(query);
    if query_terms.is_empty() || limit == 0 {
        return Vec::new();
    }

    let normalized_query = query_terms.join(" ");
    let mut scored = chunks
        .iter()
        .filter(|chunk| filters.matches(chunk))
        .filter_map(|chunk| {
            let details = score_chunk(chunk, &query_terms, &normalized_query);
            (details.score > 0).then_some(ScoredChunk {
                score: details.score,
                chunk,
                phrase_hits: details.phrase_hits,
                term_hits: details.term_hits,
                title_boosts: details.title_boosts,
                theme_boosts: details.theme_boosts,
                citation_boosts: details.citation_boosts,
            })
        })
        .collect::<Vec<_>>();

    scored.sort_by_key(|candidate| {
        (
            Reverse(candidate.score),
            candidate.chunk.citation.clone(),
            candidate.chunk.chunk_index,
        )
    });

    diversify_by_citation(scored, limit)
}

pub fn read_page_context(
    chunks: &[KnowledgeChunk],
    doc_id: &str,
    page: u32,
    radius: Option<u32>,
) -> Vec<SourceCitedPassage> {
    let radius = radius.unwrap_or(1).min(5);
    let start_page = page.saturating_sub(radius).max(1);
    let end_page = page.saturating_add(radius);
    let mut passages = chunks
        .iter()
        .filter(|chunk| chunk.doc_id == doc_id)
        .filter(|chunk| (start_page..=end_page).contains(&chunk.page))
        .collect::<Vec<_>>();

    passages.sort_by_key(|chunk| (chunk.page, chunk.chunk_index, chunk.citation.clone()));

    passages.into_iter().map(source_cited_passage).collect()
}

impl SearchFilters {
    fn matches(&self, chunk: &KnowledgeChunk) -> bool {
        self.matches_theme(chunk)
            && self.matches_doc_id(chunk)
            && self.matches_file_name(chunk)
            && self.matches_page(chunk)
    }

    fn matches_theme(&self, chunk: &KnowledgeChunk) -> bool {
        self.theme.as_ref().is_none_or(|theme| {
            chunk
                .themes
                .iter()
                .any(|chunk_theme| chunk_theme.eq_ignore_ascii_case(theme))
        })
    }

    fn matches_doc_id(&self, chunk: &KnowledgeChunk) -> bool {
        self.doc_id
            .as_ref()
            .is_none_or(|doc_id| chunk.doc_id == *doc_id)
    }

    fn matches_file_name(&self, chunk: &KnowledgeChunk) -> bool {
        self.file_name
            .as_ref()
            .is_none_or(|file_name| chunk.file_name.eq_ignore_ascii_case(file_name))
    }

    fn matches_page(&self, chunk: &KnowledgeChunk) -> bool {
        self.page.is_none_or(|page| chunk.page == page)
    }
}

fn score_chunk(
    chunk: &KnowledgeChunk,
    query_terms: &[String],
    normalized_query: &str,
) -> ScoreDetails {
    let text = normalize_search_text(&chunk.text);
    let title = normalize_search_text(&chunk.title);
    let themes = normalize_search_text(&chunk.themes.join(" "));
    let citation = normalize_search_text(&format!("{} {}", chunk.file_name, chunk.citation));

    let text_tokens = tokenize_normalized(&text);
    let title_tokens = tokenize_normalized(&title);
    let theme_tokens = tokenize_normalized(&themes);
    let citation_tokens = tokenize_normalized(&citation);

    let mut details = ScoreDetails::default();

    if text.contains(normalized_query) {
        details.score += 80;
        push_unique(&mut details.phrase_hits, format!("text:{normalized_query}"));
    }
    if title.contains(normalized_query) {
        details.score += 50;
        push_unique(
            &mut details.title_boosts,
            format!("title:{normalized_query}"),
        );
    }
    if themes.contains(normalized_query) {
        details.score += 40;
        push_unique(
            &mut details.theme_boosts,
            format!("theme:{normalized_query}"),
        );
    }

    for phrase in query_terms.windows(2).map(|terms| terms.join(" ")) {
        if text.contains(&phrase) {
            details.score += 25;
            push_unique(&mut details.phrase_hits, format!("text:{phrase}"));
        }
        if title.contains(&phrase) {
            details.score += 20;
            push_unique(&mut details.title_boosts, format!("title:{phrase}"));
        }
        if themes.contains(&phrase) {
            details.score += 16;
            push_unique(&mut details.theme_boosts, format!("theme:{phrase}"));
        }
    }

    for term in query_terms {
        let text_count = capped_frequency(&text_tokens, term, 3);
        let title_count = capped_frequency(&title_tokens, term, 2);
        let theme_count = capped_frequency(&theme_tokens, term, 2);
        let citation_count = capped_frequency(&citation_tokens, term, 1);

        details.score += text_count * 8;
        details.score += title_count * 12;
        details.score += theme_count * 18;
        details.score += citation_count * 3;

        if title_count > 0 {
            push_unique(&mut details.title_boosts, format!("title:{term}"));
        }
        if theme_count > 0 {
            push_unique(&mut details.theme_boosts, format!("theme:{term}"));
        }
        if citation_count > 0 {
            push_unique(&mut details.citation_boosts, format!("citation:{term}"));
        }
        if text_count + title_count + theme_count + citation_count > 0 {
            details.term_hits.push(TermHit {
                term: term.clone(),
                text_count,
                title_count,
                theme_count,
                citation_count,
            });
        }
    }

    details
}

fn diversify_by_citation(
    scored: Vec<ScoredChunk<'_>>,
    limit: usize,
) -> Vec<ExplainedSearchPassage> {
    let mut seen = BTreeSet::<String>::new();
    let mut primary = Vec::new();
    let mut duplicates = Vec::new();

    for candidate in scored {
        if seen.insert(candidate.chunk.citation.clone()) {
            primary.push(explained_search_passage(candidate, false));
        } else {
            duplicates.push(explained_search_passage(candidate, true));
        }
    }

    primary.into_iter().chain(duplicates).take(limit).collect()
}

fn explained_search_passage(
    scored: ScoredChunk<'_>,
    duplicate_citation: bool,
) -> ExplainedSearchPassage {
    let passage = source_cited_passage(scored.chunk);
    ExplainedSearchPassage {
        citation: passage.citation.clone(),
        passage,
        score: scored.score,
        phrase_hits: scored.phrase_hits,
        term_hits: scored.term_hits,
        title_boosts: scored.title_boosts,
        theme_boosts: scored.theme_boosts,
        citation_boosts: scored.citation_boosts,
        duplicate_citation,
    }
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

fn tokenize(text: &str) -> Vec<String> {
    tokenize_normalized(&normalize_search_text(text))
}

fn tokenize_normalized(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|term| term.len() >= 3)
        .map(str::to_string)
        .collect()
}

fn normalize_search_text(text: &str) -> String {
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

    normalized.trim().to_string()
}

fn capped_frequency(tokens: &[String], term: &str, cap: usize) -> usize {
    tokens
        .iter()
        .filter(|token| token.as_str() == term)
        .count()
        .min(cap)
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}
