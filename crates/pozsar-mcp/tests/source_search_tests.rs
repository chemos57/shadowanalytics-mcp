use pozsar_kb::chunk::KnowledgeChunk;
use pozsar_mcp::tools::{load_chunks_jsonl, search_chunks, PozsarCorpusMcp, SearchPozsarParams};
use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn search_returns_source_cited_passages() {
    let chunks = vec![
        KnowledgeChunk {
            doc_id: "safe-asset-glut".to_string(),
            file_name: "The_Safe_Asset_Glut.pdf".to_string(),
            page: 7,
            chunk_index: 0,
            title: "The Safe Asset Glut".to_string(),
            text: "Safe assets and collateral demand shape dollar liquidity.".to_string(),
            themes: vec!["collateral".to_string(), "dollar_liquidity".to_string()],
            citation: "The_Safe_Asset_Glut.pdf:7".to_string(),
        },
        KnowledgeChunk {
            doc_id: "unrelated".to_string(),
            file_name: "Other.pdf".to_string(),
            page: 1,
            chunk_index: 0,
            title: "Other".to_string(),
            text: "This passage is about another topic.".to_string(),
            themes: Vec::new(),
            citation: "Other.pdf:1".to_string(),
        },
    ];

    let results = search_chunks(&chunks, "collateral dollar", 5);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].citation, "The_Safe_Asset_Glut.pdf:7");
    assert_eq!(results[0].doc_id, "safe-asset-glut");
}

#[test]
fn mcp_search_smoke_test_uses_jsonl_fixture_and_returns_citations() {
    let chunks_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let chunks = load_chunks_jsonl(&chunks_path).unwrap();
    let service = PozsarCorpusMcp::new(chunks);

    let response = service.search_pozsar_kb(Parameters(SearchPozsarParams {
        query: "safe collateral dollar liquidity".to_string(),
        limit: Some(2),
    }));
    let results: Vec<Value> = serde_json::from_str(&response).unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["citation"], "The_Safe_Asset_Glut.pdf:7");
    assert_eq!(results[0]["doc_id"], "safe-asset-glut");
    assert_eq!(results[0]["page"], 7);
    assert_eq!(results[1]["citation"], "Bretton-Woods-III.pdf:2");
}
