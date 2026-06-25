use crate::chunk::KnowledgeChunk;
use crate::extract::ExtractedPage;
use crate::manifest::PdfManifestEntry;
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::Path;

pub fn write_manifest(entries: &[PdfManifestEntry], output_path: &Path) -> Result<()> {
    write_pretty_json(entries, output_path)
}

pub fn write_pages_jsonl(pages: &[ExtractedPage], output_path: &Path) -> Result<()> {
    write_jsonl(pages, output_path)
}

pub fn write_chunks_jsonl(chunks: &[KnowledgeChunk], output_path: &Path) -> Result<()> {
    write_jsonl(chunks, output_path)
}

pub fn read_manifest(input_path: &Path) -> Result<Vec<PdfManifestEntry>> {
    let json =
        fs::read_to_string(input_path).with_context(|| format!("read {}", input_path.display()))?;
    serde_json::from_str(&json).with_context(|| format!("parse {}", input_path.display()))
}

pub fn read_pages_jsonl(input_path: &Path) -> Result<Vec<ExtractedPage>> {
    read_jsonl(input_path)
}

pub fn read_chunks_jsonl(input_path: &Path) -> Result<Vec<KnowledgeChunk>> {
    read_jsonl(input_path)
}

fn write_pretty_json<T: Serialize + ?Sized>(value: &T, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn write_jsonl<T: Serialize>(values: &[T], output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = String::new();
    for value in values {
        output.push_str(&serde_json::to_string(value)?);
        output.push('\n');
    }
    fs::write(output_path, output)?;
    Ok(())
}

fn read_jsonl<T: DeserializeOwned>(input_path: &Path) -> Result<Vec<T>> {
    let jsonl =
        fs::read_to_string(input_path).with_context(|| format!("read {}", input_path.display()))?;
    jsonl
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<T>(line)
                .with_context(|| format!("parse {} line {}", input_path.display(), index + 1))
        })
        .collect()
}
