use pozsar_kb::chunk::KnowledgeChunk;
use pozsar_mcp::search::{
    explain_search_chunks_with_filters, read_page_context, search_chunks,
    search_chunks_with_filters, SearchFilters,
};
use pozsar_mcp::tools::{
    load_chunks_jsonl, PozsarCorpusMcp, ReadPageContextParams, SearchPozsarParams,
};
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
        theme: None,
        doc_id: None,
        file_name: None,
        page: None,
    }));
    let results: Vec<Value> = serde_json::from_str(&response).unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["citation"], "The_Safe_Asset_Glut.pdf:7");
    assert_eq!(results[0]["doc_id"], "safe-asset-glut");
    assert_eq!(results[0]["page"], 7);
    assert_eq!(results[1]["citation"], "Bretton-Woods-III.pdf:2");
}

#[test]
fn phrase_match_outranks_scattered_term_match() {
    let chunks = vec![
        chunk(
            "scattered",
            "A-Scattered.pdf",
            1,
            0,
            "Dollar funding pressure can impair bank liquidity.",
            "Scattered",
            &[],
        ),
        chunk(
            "phrase",
            "Z-Phrase.pdf",
            1,
            0,
            "Dollar liquidity is the core constraint in this passage.",
            "Phrase",
            &[],
        ),
    ];

    let results = search_chunks(&chunks, "dollar liquidity", 2);

    assert_eq!(results[0].citation, "Z-Phrase.pdf:1");
    assert_eq!(results[1].citation, "A-Scattered.pdf:1");
}

#[test]
fn title_and_theme_matches_are_searchable_and_boosted() {
    let chunks = vec![
        chunk(
            "text",
            "A-Text.pdf",
            1,
            0,
            "This passage mentions collateral once.",
            "Generic",
            &[],
        ),
        chunk(
            "title-theme",
            "Z-Title-Theme.pdf",
            1,
            0,
            "This passage discusses balance sheet constraints.",
            "Collateral Money Markets",
            &["collateral"],
        ),
    ];

    let results = search_chunks(&chunks, "collateral", 2);

    assert_eq!(results[0].citation, "Z-Title-Theme.pdf:1");
    assert_eq!(results[1].citation, "A-Text.pdf:1");
}

#[test]
fn search_prefers_distinct_citations_before_duplicate_chunks() {
    let chunks = vec![
        chunk(
            "doc-a",
            "A-Doc.pdf",
            1,
            0,
            "Repo repo repo collateral collateral.",
            "A",
            &["repo"],
        ),
        chunk(
            "doc-a",
            "A-Doc.pdf",
            1,
            1,
            "Repo repo collateral.",
            "A",
            &["repo"],
        ),
        chunk("doc-b", "B-Doc.pdf", 1, 0, "Repo collateral.", "B", &[]),
    ];

    let results = search_chunks(&chunks, "repo collateral", 2);

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].citation, "A-Doc.pdf:1");
    assert_eq!(results[0].chunk_index, 0);
    assert_eq!(results[1].citation, "B-Doc.pdf:1");
}

#[test]
fn search_tie_breaking_is_deterministic() {
    let chunks = vec![
        chunk("z", "Z.pdf", 2, 0, "Repo collateral.", "Z", &[]),
        chunk("a", "A.pdf", 1, 0, "Repo collateral.", "A", &[]),
    ];

    let results = search_chunks(&chunks, "repo collateral", 2);

    assert_eq!(results[0].citation, "A.pdf:1");
    assert_eq!(results[1].citation, "Z.pdf:2");
}

#[test]
fn search_without_filters_preserves_existing_behavior() {
    let chunks = vec![
        chunk("a", "A.pdf", 1, 0, "Repo collateral.", "A", &["repo"]),
        chunk("b", "B.pdf", 1, 0, "Repo collateral.", "B", &["repo"]),
    ];

    let unfiltered = search_chunks(&chunks, "repo collateral", 10);
    let filtered =
        search_chunks_with_filters(&chunks, "repo collateral", 10, &SearchFilters::default());

    assert_eq!(filtered, unfiltered);
}

#[test]
fn theme_filter_limits_search_results() {
    let chunks = vec![
        chunk(
            "repo",
            "Repo.pdf",
            1,
            0,
            "Dollar liquidity.",
            "Repo",
            &["repo"],
        ),
        chunk(
            "fx",
            "Fx.pdf",
            1,
            0,
            "Dollar liquidity.",
            "Fx",
            &["fx_swaps"],
        ),
    ];
    let filters = SearchFilters {
        theme: Some("REPO".to_string()),
        ..Default::default()
    };

    let results = search_chunks_with_filters(&chunks, "dollar liquidity", 10, &filters);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].citation, "Repo.pdf:1");
}

#[test]
fn doc_id_and_page_filters_combine_with_and_semantics() {
    let chunks = vec![
        chunk("doc", "Doc.pdf", 1, 0, "Repo collateral.", "Doc", &[]),
        chunk("doc", "Doc.pdf", 2, 0, "Repo collateral.", "Doc", &[]),
        chunk("other", "Other.pdf", 2, 0, "Repo collateral.", "Other", &[]),
    ];
    let filters = SearchFilters {
        doc_id: Some("doc".to_string()),
        page: Some(2),
        ..Default::default()
    };

    let results = search_chunks_with_filters(&chunks, "repo collateral", 10, &filters);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].citation, "Doc.pdf:2");
}

#[test]
fn file_name_filter_is_case_insensitive() {
    let chunks = vec![
        chunk(
            "a",
            "Liquidity-Dispatch.pdf",
            1,
            0,
            "Repo collateral.",
            "A",
            &[],
        ),
        chunk("b", "Other.pdf", 1, 0, "Repo collateral.", "B", &[]),
    ];
    let filters = SearchFilters {
        file_name: Some("liquidity-dispatch.pdf".to_string()),
        ..Default::default()
    };

    let results = search_chunks_with_filters(&chunks, "repo collateral", 10, &filters);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].citation, "Liquidity-Dispatch.pdf:1");
}

#[test]
fn page_context_returns_previous_current_and_next_pages_sorted() {
    let chunks = vec![
        chunk(
            "doc",
            "Doc.pdf",
            3,
            1,
            "third page second chunk",
            "Doc",
            &[],
        ),
        chunk("doc", "Doc.pdf", 1, 0, "first page", "Doc", &[]),
        chunk("doc", "Doc.pdf", 2, 0, "second page", "Doc", &[]),
        chunk("doc", "Doc.pdf", 3, 0, "third page first chunk", "Doc", &[]),
        chunk("other", "Other.pdf", 2, 0, "other doc", "Other", &[]),
    ];

    let results = read_page_context(&chunks, "doc", 2, Some(1));

    assert_eq!(
        results
            .iter()
            .map(|passage| (passage.page, passage.chunk_index))
            .collect::<Vec<_>>(),
        vec![(1, 0), (2, 0), (3, 0), (3, 1)]
    );
}

#[test]
fn page_context_radius_is_clamped_to_five() {
    let chunks = (1..=8)
        .map(|page| chunk("doc", "Doc.pdf", page, 0, "page", "Doc", &[]))
        .collect::<Vec<_>>();

    let results = read_page_context(&chunks, "doc", 1, Some(50));

    assert_eq!(
        results
            .iter()
            .map(|passage| passage.page)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
}

#[test]
fn mcp_page_context_tool_returns_json_passages() {
    let chunks = vec![
        chunk("doc", "Doc.pdf", 1, 0, "first page", "Doc", &[]),
        chunk("doc", "Doc.pdf", 2, 0, "second page", "Doc", &[]),
        chunk("doc", "Doc.pdf", 3, 0, "third page", "Doc", &[]),
    ];
    let service = PozsarCorpusMcp::new(chunks);

    let response = service.read_pozsar_page_context(Parameters(ReadPageContextParams {
        doc_id: "doc".to_string(),
        page: 2,
        radius: Some(1),
    }));
    let results: Vec<Value> = serde_json::from_str(&response).unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["citation"], "Doc.pdf:1");
    assert_eq!(results[2]["citation"], "Doc.pdf:3");
}

#[test]
fn explanation_records_phrase_hits_and_term_counts() {
    let chunks = vec![chunk(
        "doc",
        "Dollar-Liquidity.pdf",
        1,
        0,
        "Dollar liquidity matters. Dollar liquidity returns.",
        "Dollar Liquidity Dispatch",
        &["dollar_liquidity"],
    )];

    let results = explain_search_chunks_with_filters(
        &chunks,
        "dollar liquidity",
        1,
        &SearchFilters::default(),
    );

    assert_eq!(results.len(), 1);
    assert!(results[0].score > 0);
    assert!(results[0]
        .phrase_hits
        .contains(&"text:dollar liquidity".to_string()));
    let dollar_hit = results[0]
        .term_hits
        .iter()
        .find(|hit| hit.term == "dollar")
        .unwrap();
    assert_eq!(dollar_hit.text_count, 2);
    assert_eq!(dollar_hit.title_count, 1);
    assert_eq!(dollar_hit.theme_count, 1);
    assert_eq!(dollar_hit.citation_count, 1);
}

#[test]
fn explanation_records_title_theme_and_citation_boosts() {
    let chunks = vec![chunk(
        "doc",
        "Collateral-Dispatch.pdf",
        1,
        0,
        "Balance sheet constraints matter.",
        "Collateral Markets",
        &["collateral"],
    )];

    let results =
        explain_search_chunks_with_filters(&chunks, "collateral", 1, &SearchFilters::default());

    assert_eq!(results.len(), 1);
    assert!(results[0]
        .title_boosts
        .contains(&"title:collateral".to_string()));
    assert!(results[0]
        .theme_boosts
        .contains(&"theme:collateral".to_string()));
    assert!(results[0]
        .citation_boosts
        .contains(&"citation:collateral".to_string()));
}

#[test]
fn explanation_honors_search_filters() {
    let chunks = vec![
        chunk(
            "repo",
            "Repo.pdf",
            1,
            0,
            "Dollar liquidity.",
            "Repo",
            &["repo"],
        ),
        chunk(
            "fx",
            "Fx.pdf",
            1,
            0,
            "Dollar liquidity.",
            "Fx",
            &["fx_swaps"],
        ),
    ];
    let filters = SearchFilters {
        theme: Some("repo".to_string()),
        ..Default::default()
    };

    let results = explain_search_chunks_with_filters(&chunks, "dollar liquidity", 10, &filters);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].citation, "Repo.pdf:1");
}

#[test]
fn explanation_marks_duplicate_citation_results() {
    let chunks = vec![
        chunk(
            "doc-a",
            "A-Doc.pdf",
            1,
            0,
            "Repo repo collateral collateral.",
            "A",
            &[],
        ),
        chunk("doc-a", "A-Doc.pdf", 1, 1, "Repo collateral.", "A", &[]),
        chunk("doc-b", "B-Doc.pdf", 1, 0, "Repo collateral.", "B", &[]),
    ];

    let results = explain_search_chunks_with_filters(
        &chunks,
        "repo collateral",
        3,
        &SearchFilters::default(),
    );

    assert_eq!(results[0].citation, "A-Doc.pdf:1");
    assert!(!results[0].duplicate_citation);
    assert_eq!(results[1].citation, "B-Doc.pdf:1");
    assert!(!results[1].duplicate_citation);
    assert_eq!(results[2].citation, "A-Doc.pdf:1");
    assert!(results[2].duplicate_citation);
}

#[test]
fn mcp_explain_search_returns_scoring_json() {
    let chunks = vec![chunk(
        "doc",
        "Dollar-Liquidity.pdf",
        1,
        0,
        "Dollar liquidity matters.",
        "Dollar Liquidity",
        &["dollar_liquidity"],
    )];
    let service = PozsarCorpusMcp::new(chunks);

    let response = service.explain_pozsar_search(Parameters(SearchPozsarParams {
        query: "dollar liquidity".to_string(),
        limit: Some(1),
        theme: None,
        doc_id: None,
        file_name: None,
        page: None,
    }));
    let results: Vec<Value> = serde_json::from_str(&response).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["citation"], "Dollar-Liquidity.pdf:1");
    assert!(results[0]["score"].as_u64().unwrap() > 0);
    assert_eq!(results[0]["passage"]["citation"], "Dollar-Liquidity.pdf:1");
    assert_eq!(results[0]["phrase_hits"][0], "text:dollar liquidity");
}

fn chunk(
    doc_id: &str,
    file_name: &str,
    page: u32,
    chunk_index: u32,
    text: &str,
    title: &str,
    themes: &[&str],
) -> KnowledgeChunk {
    KnowledgeChunk {
        doc_id: doc_id.to_string(),
        file_name: file_name.to_string(),
        page,
        chunk_index,
        title: title.to_string(),
        text: text.to_string(),
        themes: themes.iter().map(|theme| theme.to_string()).collect(),
        citation: format!("{file_name}:{page}"),
    }
}
