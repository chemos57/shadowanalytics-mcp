use pozsar_kb::chunk::chunk_pages;
use pozsar_kb::extract::ExtractedPage;

#[test]
fn chunks_keep_source_metadata_and_citation() {
    let pages = vec![ExtractedPage {
        doc_id: "safe-asset-glut".to_string(),
        file_name: "The_Safe_Asset_Glut.pdf".to_string(),
        page: 4,
        text: "Dollar liquidity and safe assets interact through collateral demand.".to_string(),
    }];

    let chunks = chunk_pages(&pages, 32, 8);

    assert!(chunks.len() >= 2);
    assert_eq!(chunks[0].doc_id, "safe-asset-glut");
    assert_eq!(chunks[0].page, 4);
    assert_eq!(chunks[0].citation, "The_Safe_Asset_Glut.pdf:4");
}

#[test]
fn chunks_unicode_text_when_overlap_lands_inside_multibyte_character() {
    let pages = vec![ExtractedPage {
        doc_id: "bretton-woods-iii".to_string(),
        file_name: "Bretton-Woods-III-Zoltan-Pozsar.pdf".to_string(),
        page: 1,
        text: "abc–def".to_string(),
    }];

    let chunks = chunk_pages(&pages, 6, 1);

    assert!(chunks.len() >= 2);
    assert!(chunks.iter().all(|chunk| !chunk.text.is_empty()));
}
