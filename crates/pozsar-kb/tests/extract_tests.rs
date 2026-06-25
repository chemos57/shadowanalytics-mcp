use pozsar_kb::extract::{
    collect_pages_in_manifest_order, normalize_text, pages_from_texts, split_text_into_pages,
    ExtractedPage,
};
use pozsar_kb::manifest::PdfManifestEntry;

#[test]
fn normalizes_extracted_text() {
    let text = "  Collateral matters.  \n\n  Dollar liquidity matters. \n";
    assert_eq!(
        normalize_text(text),
        "Collateral matters.\nDollar liquidity matters."
    );
}

#[test]
fn splits_form_feed_pages() {
    let pages = split_text_into_pages("doc", "doc.pdf", "First page\u{000c}Second page");
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].page, 1);
    assert_eq!(pages[1].page, 2);
}

#[test]
fn builds_pages_from_page_text_vector_without_collapsing_page_numbers() {
    let pages = pages_from_texts(
        "doc",
        "doc.pdf",
        vec!["First page".to_string(), "Second page".to_string()],
    );

    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].page, 1);
    assert_eq!(pages[0].text, "First page");
    assert_eq!(pages[1].page, 2);
    assert_eq!(pages[1].text, "Second page");
}

#[test]
fn collects_manifest_pages_in_manifest_order() {
    let entries = vec![manifest_entry("doc-b.pdf"), manifest_entry("doc-a.pdf")];

    let pages = collect_pages_in_manifest_order(&entries, |entry| {
        Ok(vec![ExtractedPage {
            doc_id: entry.doc_id.clone(),
            file_name: entry.file_name.clone(),
            page: 1,
            text: entry.file_name.clone(),
        }])
    })
    .unwrap();

    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].file_name, "doc-b.pdf");
    assert_eq!(pages[1].file_name, "doc-a.pdf");
}

fn manifest_entry(file_name: &str) -> PdfManifestEntry {
    PdfManifestEntry {
        doc_id: file_name.trim_end_matches(".pdf").to_string(),
        file_name: file_name.to_string(),
        path: file_name.to_string(),
        sha256: "hash".to_string(),
        bytes: 1,
    }
}
