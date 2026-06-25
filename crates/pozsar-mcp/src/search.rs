use crate::tools::SourceCitedPassage;
use pozsar_kb::chunk::KnowledgeChunk;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BTreeSet;

#[derive(Debug)]
struct ScoredChunk<'a> {
    score: usize,
    chunk: &'a KnowledgeChunk,
    score_breakdown: ScoreBreakdown,
    phrase_hits: Vec<String>,
    term_hits: Vec<TermHit>,
    title_boosts: Vec<String>,
    theme_boosts: Vec<String>,
    citation_boosts: Vec<String>,
    snippet: Option<String>,
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
    pub score_breakdown: ScoreBreakdown,
    pub phrase_hits: Vec<String>,
    pub term_hits: Vec<TermHit>,
    pub title_boosts: Vec<String>,
    pub theme_boosts: Vec<String>,
    pub citation_boosts: Vec<String>,
    pub duplicate_citation: bool,
    pub snippet: Option<String>,
    pub citation: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ScoreBreakdown {
    pub text_phrase: usize,
    pub text_terms: usize,
    pub title: usize,
    pub theme: usize,
    pub citation: usize,
}

impl ScoreBreakdown {
    fn total(&self) -> usize {
        self.text_phrase + self.text_terms + self.title + self.theme + self.citation
    }
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
    score_breakdown: ScoreBreakdown,
    phrase_hits: Vec<String>,
    term_hits: Vec<TermHit>,
    title_boosts: Vec<String>,
    theme_boosts: Vec<String>,
    citation_boosts: Vec<String>,
    snippet: Option<String>,
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
    let theme_query_terms = tokenize_theme_query(query);
    if (query_terms.is_empty() && theme_query_terms.is_empty()) || limit == 0 {
        return Vec::new();
    }

    let normalized_query = query_terms.join(" ");
    let mut scored = chunks
        .iter()
        .filter(|chunk| filters.matches(chunk))
        .filter_map(|chunk| {
            let details = score_chunk(chunk, &query_terms, &theme_query_terms, &normalized_query);
            (details.score > 0).then_some(ScoredChunk {
                score: details.score,
                chunk,
                score_breakdown: details.score_breakdown,
                phrase_hits: details.phrase_hits,
                term_hits: details.term_hits,
                title_boosts: details.title_boosts,
                theme_boosts: details.theme_boosts,
                citation_boosts: details.citation_boosts,
                snippet: details.snippet,
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
    theme_query_terms: &[String],
    normalized_query: &str,
) -> ScoreDetails {
    let text = normalize_search_text(&chunk.text);
    let title = normalize_search_text(&chunk.title);
    let citation = normalize_search_text(&format!("{} {}", chunk.file_name, chunk.citation));

    let text_tokens = tokenize_normalized(&text);
    let title_tokens = tokenize_normalized(&title);
    let citation_tokens = tokenize_normalized(&citation);
    let matching_themes = matching_theme_labels(&chunk.themes, theme_query_terms);
    let matching_theme_parts = matching_themes
        .iter()
        .flat_map(|theme| theme.split('_').map(str::to_string))
        .collect::<Vec<_>>();
    let term_hit_terms = term_hit_terms(query_terms, theme_query_terms);

    let mut details = ScoreDetails::default();

    if query_terms.len() > 1 && text.contains(normalized_query) {
        details.score_breakdown.text_phrase += 80;
        push_unique(&mut details.phrase_hits, format!("text:{normalized_query}"));
        details.snippet = best_snippet(&chunk.text, &[normalized_query], query_terms);
    }
    if query_terms.len() > 1 && title.contains(normalized_query) {
        details.score_breakdown.title += 50;
        push_unique(
            &mut details.title_boosts,
            format!("title:{normalized_query}"),
        );
    }

    for phrase in query_terms.windows(2).map(|terms| terms.join(" ")) {
        if text.contains(&phrase) {
            details.score_breakdown.text_phrase += 25;
            push_unique(&mut details.phrase_hits, format!("text:{phrase}"));
            if details.snippet.is_none() {
                details.snippet = best_snippet(&chunk.text, &[&phrase], query_terms);
            }
        }
        if title.contains(&phrase) {
            details.score_breakdown.title += 20;
            push_unique(&mut details.title_boosts, format!("title:{phrase}"));
        }
    }

    for term in &term_hit_terms {
        let text_count = capped_frequency(&text_tokens, term, 3);
        let title_count = capped_frequency(&title_tokens, term, 2);
        let theme_count = matching_theme_parts
            .iter()
            .filter(|part| part.as_str() == term)
            .count();
        let citation_count = capped_frequency(&citation_tokens, term, 1);

        details.score_breakdown.text_terms += text_count * 8;
        details.score_breakdown.title += title_count * 12;
        details.score_breakdown.citation += citation_count * 3;

        if title_count > 0 {
            push_unique(&mut details.title_boosts, format!("title:{term}"));
        }
        if citation_count > 0 {
            push_unique(&mut details.citation_boosts, format!("citation:{term}"));
        }
        if details.snippet.is_none() && text_count > 0 {
            details.snippet = best_snippet(&chunk.text, &[], std::slice::from_ref(term));
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

    for theme in matching_themes {
        details.score_breakdown.theme += 18;
        push_unique(&mut details.theme_boosts, format!("theme:{theme}"));
    }

    details.score = details.score_breakdown.total();
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
        score_breakdown: scored.score_breakdown,
        phrase_hits: scored.phrase_hits,
        term_hits: scored.term_hits,
        title_boosts: scored.title_boosts,
        theme_boosts: scored.theme_boosts,
        citation_boosts: scored.citation_boosts,
        duplicate_citation,
        snippet: scored.snippet,
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

fn tokenize_theme_query(text: &str) -> Vec<String> {
    normalize_search_text(text)
        .split_whitespace()
        .map(str::to_string)
        .collect()
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

fn term_hit_terms(query_terms: &[String], theme_query_terms: &[String]) -> Vec<String> {
    let mut terms = query_terms.to_vec();
    for term in theme_query_terms {
        if !terms.contains(term) {
            terms.push(term.clone());
        }
    }
    terms
}

fn matching_theme_labels(themes: &[String], theme_query_terms: &[String]) -> Vec<String> {
    let query = theme_query_terms.join(" ");
    themes
        .iter()
        .filter_map(|theme| {
            let canonical = canonical_theme_label(theme);
            let spaced = canonical.replace('_', " ");
            let exact_label_match = query == spaced || query == canonical;
            let all_theme_parts_match = spaced
                .split_whitespace()
                .all(|part| theme_query_terms.iter().any(|term| term == part));
            (exact_label_match || all_theme_parts_match).then_some(canonical)
        })
        .collect()
}

fn canonical_theme_label(theme: &str) -> String {
    normalize_search_text(theme).replace(' ', "_")
}

fn best_snippet(text: &str, phrases: &[&str], terms: &[String]) -> Option<String> {
    let indexed_tokens = indexed_tokens(text);
    let phrase_terms = phrases
        .iter()
        .find_map(|phrase| find_phrase_token_span(&indexed_tokens, &tokenize(phrase)));
    let term_span = terms
        .iter()
        .find_map(|term| find_phrase_token_span(&indexed_tokens, std::slice::from_ref(term)));
    let (start_token, end_token) = phrase_terms.or(term_span)?;
    Some(snippet_around_token_span(
        text,
        &indexed_tokens,
        start_token,
        end_token,
        240,
    ))
}

fn indexed_tokens(text: &str) -> Vec<IndexedToken> {
    let mut tokens = Vec::new();
    let mut token_start = None;

    for (index, ch) in text.char_indices() {
        if ch.is_ascii_alphanumeric() {
            token_start.get_or_insert(index);
        } else if let Some(start) = token_start.take() {
            tokens.push(indexed_token(text, start, index));
        }
    }

    if let Some(start) = token_start {
        tokens.push(indexed_token(text, start, text.len()));
    }

    tokens
}

#[derive(Debug)]
struct IndexedToken {
    normalized: String,
    start: usize,
    end: usize,
}

fn indexed_token(text: &str, start: usize, end: usize) -> IndexedToken {
    IndexedToken {
        normalized: text[start..end].to_ascii_lowercase(),
        start,
        end,
    }
}

fn find_phrase_token_span(
    tokens: &[IndexedToken],
    phrase_terms: &[String],
) -> Option<(usize, usize)> {
    if phrase_terms.is_empty() {
        return None;
    }

    tokens
        .windows(phrase_terms.len())
        .position(|window| {
            window
                .iter()
                .map(|token| token.normalized.as_str())
                .eq(phrase_terms.iter().map(String::as_str))
        })
        .map(|start| (start, start + phrase_terms.len() - 1))
}

fn snippet_around_token_span(
    text: &str,
    tokens: &[IndexedToken],
    start_token: usize,
    end_token: usize,
    max_chars: usize,
) -> String {
    let match_start = tokens[start_token].start;
    let match_end = tokens[end_token].end;
    let mut start = match_start.saturating_sub(max_chars / 3);
    let mut end = (match_end + (max_chars * 2 / 3)).min(text.len());

    start = next_char_boundary(text, start);
    end = previous_char_boundary(text, end);

    if start > 0 {
        while start < match_start {
            let Some(ch) = text[start..].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                start += ch.len_utf8();
                break;
            }
            start += ch.len_utf8();
        }
    }

    if end < text.len() {
        while end > match_end {
            let Some(ch) = text[..end].chars().next_back() else {
                break;
            };
            if ch.is_whitespace() {
                break;
            }
            end -= ch.len_utf8();
        }
    }

    if end < match_end {
        end = match_end;
    }

    while let Some(ch) = text[end..].chars().next() {
        if matches!(ch, '.' | ',' | '!' | '?' | ':' | ';') {
            end += ch.len_utf8();
        } else {
            break;
        }
    }

    normalize_display_snippet(&text[start..end])
}

fn normalize_display_snippet(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn next_char_boundary(text: &str, mut index: usize) -> usize {
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

fn previous_char_boundary(text: &str, mut index: usize) -> usize {
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}
