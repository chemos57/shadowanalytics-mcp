use crate::extract::ExtractedPage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeChunk {
    pub doc_id: String,
    pub file_name: String,
    pub page: u32,
    pub chunk_index: u32,
    pub title: String,
    pub text: String,
    pub themes: Vec<String>,
    pub citation: String,
}

impl KnowledgeChunk {
    pub fn stable_id(&self) -> String {
        format!("{}:{}:{}", self.doc_id, self.page, self.chunk_index)
    }
}

pub fn chunk_pages(
    pages: &[ExtractedPage],
    max_chars: usize,
    overlap_chars: usize,
) -> Vec<KnowledgeChunk> {
    let mut chunks = Vec::new();
    for page in pages {
        let mut start = 0usize;
        let mut chunk_index = 0u32;
        while start < page.text.len() {
            let end = next_char_boundary(&page.text, (start + max_chars).min(page.text.len()));
            let text = page.text[start..end].trim().to_string();
            if !text.is_empty() {
                chunks.push(KnowledgeChunk {
                    doc_id: page.doc_id.clone(),
                    file_name: page.file_name.clone(),
                    page: page.page,
                    chunk_index,
                    title: page.file_name.trim_end_matches(".pdf").to_string(),
                    text,
                    themes: Vec::new(),
                    citation: format!("{}:{}", page.file_name, page.page),
                });
                chunk_index += 1;
            }
            if end == page.text.len() {
                break;
            }
            let next_start = previous_char_boundary(&page.text, end.saturating_sub(overlap_chars));
            start = if next_start <= start { end } else { next_start };
        }
    }
    chunks
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
