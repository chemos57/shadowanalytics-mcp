use pozsar_kb::artifacts::{write_chunks_jsonl, write_manifest, write_pages_jsonl};
use pozsar_kb::chunk::KnowledgeChunk;
use pozsar_kb::extract::ExtractedPage;
use pozsar_kb::inspect::inspect_artifacts;
use pozsar_kb::manifest::PdfManifestEntry;
use tempfile::tempdir;

#[test]
fn inspects_artifact_counts_themes_and_validation_issues() {
    let dir = tempdir().unwrap();
    write_manifest(
        &[
            manifest_entry("doc-a", "Doc-A.pdf"),
            manifest_entry("doc-b", "Doc-B.pdf"),
        ],
        &dir.path().join("manifest.json"),
    )
    .unwrap();
    write_pages_jsonl(
        &[
            extracted_page(
                "doc-a",
                "Doc-A.pdf",
                1,
                "Collateral drives dollar liquidity.",
            ),
            extracted_page("doc-a", "Doc-A.pdf", 2, ""),
            extracted_page(
                "doc-b",
                "Doc-B.pdf",
                1,
                "This non-empty page has no chunks.",
            ),
        ],
        &dir.path().join("extracted_pages.jsonl"),
    )
    .unwrap();
    write_chunks_jsonl(
        &[
            knowledge_chunk(
                "doc-a",
                "Doc-A.pdf",
                1,
                0,
                &["collateral", "dollar_liquidity"],
                "Doc-A.pdf:1",
            ),
            knowledge_chunk("missing-doc", "Missing.pdf", 99, 0, &["collateral"], "bad"),
        ],
        &dir.path().join("chunks/pozsar_chunks.jsonl"),
    )
    .unwrap();

    let report = inspect_artifacts(dir.path()).unwrap();

    assert_eq!(report.document_count, 2);
    assert_eq!(report.extracted_page_count, 3);
    assert_eq!(report.chunk_count, 2);
    assert_eq!(report.empty_page_count, 1);
    assert_eq!(report.pages_without_chunks.len(), 1);
    assert_eq!(report.pages_without_chunks[0].citation, "Doc-B.pdf:1");
    assert_eq!(report.theme_counts[0].theme, "collateral");
    assert_eq!(report.theme_counts[0].chunks, 2);
    assert_eq!(report.theme_counts[1].theme, "dollar_liquidity");
    assert_eq!(report.validation_issues.len(), 3);
    assert!(report
        .validation_issues
        .iter()
        .any(|issue| issue.contains("chunk source page missing-doc:99 is missing")));
    assert!(report
        .validation_issues
        .iter()
        .any(|issue| issue.contains("chunk citation bad should be Missing.pdf:99")));
    assert!(report
        .validation_issues
        .iter()
        .any(|issue| issue.contains("page Doc-B.pdf:1 has text but no chunks")));
}

#[test]
fn inspect_errors_when_required_artifacts_are_missing() {
    let dir = tempdir().unwrap();

    let error = inspect_artifacts(dir.path()).unwrap_err().to_string();

    assert!(error.contains("read"));
    assert!(error.contains("manifest.json"));
}

fn manifest_entry(doc_id: &str, file_name: &str) -> PdfManifestEntry {
    PdfManifestEntry {
        doc_id: doc_id.to_string(),
        file_name: file_name.to_string(),
        path: format!("docs/{file_name}"),
        sha256: "abc".to_string(),
        bytes: 1,
    }
}

fn extracted_page(doc_id: &str, file_name: &str, page: u32, text: &str) -> ExtractedPage {
    ExtractedPage {
        doc_id: doc_id.to_string(),
        file_name: file_name.to_string(),
        page,
        text: text.to_string(),
    }
}

fn knowledge_chunk(
    doc_id: &str,
    file_name: &str,
    page: u32,
    chunk_index: u32,
    themes: &[&str],
    citation: &str,
) -> KnowledgeChunk {
    KnowledgeChunk {
        doc_id: doc_id.to_string(),
        file_name: file_name.to_string(),
        page,
        chunk_index,
        title: file_name.trim_end_matches(".pdf").to_string(),
        text: "Chunk text".to_string(),
        themes: themes.iter().map(|theme| theme.to_string()).collect(),
        citation: citation.to_string(),
    }
}
