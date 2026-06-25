use pozsar_kb::chunk::KnowledgeChunk;
use pozsar_kb::themes::tag_chunk;

#[test]
fn tags_repo_and_dollar_liquidity_themes() {
    let chunk = KnowledgeChunk {
        doc_id: "test".to_string(),
        file_name: "test.pdf".to_string(),
        page: 1,
        chunk_index: 0,
        title: "test".to_string(),
        text: "Repo markets and dollar liquidity depend on reserves.".to_string(),
        themes: Vec::new(),
        citation: "test.pdf:1".to_string(),
    };

    let tagged = tag_chunk(chunk);

    assert!(tagged.themes.contains(&"repo".to_string()));
    assert!(tagged.themes.contains(&"dollar_liquidity".to_string()));
}
