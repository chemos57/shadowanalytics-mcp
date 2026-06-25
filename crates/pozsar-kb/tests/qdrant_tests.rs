use pozsar_kb::chunk::KnowledgeChunk;
use pozsar_kb::qdrant::chunk_payload;

#[test]
fn payload_contains_source_citation_fields() {
    let chunk = KnowledgeChunk {
        doc_id: "safe-asset-glut".to_string(),
        file_name: "The_Safe_Asset_Glut.pdf".to_string(),
        page: 7,
        chunk_index: 3,
        title: "The Safe Asset Glut".to_string(),
        text: "Safe assets are central to the framework.".to_string(),
        themes: vec!["collateral".to_string()],
        citation: "The_Safe_Asset_Glut.pdf:7".to_string(),
    };

    let payload = chunk_payload(&chunk);

    assert!(payload.contains_key("doc_id"));
    assert!(payload.contains_key("file_name"));
    assert!(payload.contains_key("page"));
    assert!(payload.contains_key("text"));
    assert!(payload.contains_key("themes"));
    assert!(payload.contains_key("citation"));
}
