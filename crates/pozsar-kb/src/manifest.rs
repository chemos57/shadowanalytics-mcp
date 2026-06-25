use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfManifestEntry {
    pub doc_id: String,
    pub file_name: String,
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

pub fn build_manifest(docs_dir: &Path) -> Result<Vec<PdfManifestEntry>> {
    let mut entries = Vec::new();
    for entry in WalkDir::new(docs_dir).min_depth(1).max_depth(1) {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("pdf") {
            continue;
        }

        let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("pdf file name must be valid utf-8")?
            .to_string();

        entries.push(PdfManifestEntry {
            doc_id: doc_id_from_file_name(&file_name),
            file_name,
            path: path.to_string_lossy().to_string(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes: bytes.len() as u64,
        });
    }
    entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(entries)
}

pub fn doc_id_from_file_name(file_name: &str) -> String {
    file_name
        .trim_end_matches(".pdf")
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
