use pozsar_kb::chunk::KnowledgeChunk;
use pozsar_mcp::search::{
    explain_search_chunks_with_filters, read_page_context, search_chunks,
    search_chunks_with_filters, SearchFilters,
};
use pozsar_mcp::tools::{
    load_chunks_jsonl, PozsarCorpusMcp, ReadPageContextParams, ResearchQuestionParams,
    SearchPozsarParams, SERVER_NAME, SERVER_VERSION,
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
fn mcp_status_reports_server_metadata_and_corpus_counts() {
    let chunks = vec![
        chunk(
            "doc-a",
            "A.pdf",
            1,
            0,
            "Repo collateral.",
            "A",
            &["repo", "collateral"],
        ),
        chunk(
            "doc-a",
            "A.pdf",
            1,
            1,
            "Dollar liquidity.",
            "A",
            &["dollar_liquidity"],
        ),
        chunk("doc-b", "B.pdf", 2, 0, "Fx swaps.", "B", &["fx_swaps"]),
    ];
    let service = PozsarCorpusMcp::new(chunks).with_chunks_path("/tmp/pozsar_chunks.jsonl");

    let response = service.get_pozsar_kb_status();
    let status: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(status["server_name"], SERVER_NAME);
    assert_eq!(status["server_version"], SERVER_VERSION);
    assert_eq!(status["chunks_path"], "/tmp/pozsar_chunks.jsonl");
    assert_eq!(status["chunk_count"], 3);
    assert_eq!(status["document_count"], 2);
    assert_eq!(status["citation_count"], 2);
    assert_eq!(status["theme_count"], 4);
    assert!(status["tools"]
        .as_array()
        .unwrap()
        .contains(&Value::String("get_pozsar_kb_status".to_string())));
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
    assert_eq!(
        results[0].score,
        results[0].score_breakdown.text_phrase
            + results[0].score_breakdown.text_terms
            + results[0].score_breakdown.title
            + results[0].score_breakdown.theme
            + results[0].score_breakdown.citation
    );
    assert_eq!(results[0].score_breakdown.text_phrase, 105);
    assert!(results[0].score_breakdown.text_terms > 0);
    assert!(results[0]
        .snippet
        .as_ref()
        .unwrap()
        .contains("Dollar liquidity matters"));
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
    assert_eq!(results[0].theme_boosts, vec!["theme:collateral"]);
    assert!(results[0]
        .citation_boosts
        .contains(&"citation:collateral".to_string()));
    assert!(results[0].score_breakdown.title > 0);
    assert!(results[0].score_breakdown.theme > 0);
    assert!(results[0].score_breakdown.citation > 0);
}

#[test]
fn explanation_uses_exact_theme_labels_not_theme_phrase_combinations() {
    let chunks = vec![chunk(
        "doc",
        "Doc.pdf",
        1,
        0,
        "Collateral and dollar funding pressure.",
        "Doc",
        &["collateral", "dollar_liquidity"],
    )];

    let results = explain_search_chunks_with_filters(
        &chunks,
        "collateral dollar liquidity",
        1,
        &SearchFilters::default(),
    );

    assert_eq!(
        results[0].theme_boosts,
        vec!["theme:collateral", "theme:dollar_liquidity"]
    );
    assert!(!results[0]
        .theme_boosts
        .contains(&"theme:collateral dollar liquidity".to_string()));
    assert!(!results[0]
        .theme_boosts
        .contains(&"theme:dollar liquidity".to_string()));
}

#[test]
fn explanation_includes_term_hits_for_theme_only_matches() {
    let chunks = vec![chunk(
        "doc",
        "Doc.pdf",
        1,
        0,
        "Balance sheet constraints.",
        "Doc",
        &["dollar_liquidity"],
    )];

    let results = explain_search_chunks_with_filters(
        &chunks,
        "dollar liquidity",
        1,
        &SearchFilters::default(),
    );

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].theme_boosts, vec!["theme:dollar_liquidity"]);
    let dollar_hit = results[0]
        .term_hits
        .iter()
        .find(|hit| hit.term == "dollar")
        .unwrap();
    assert_eq!(dollar_hit.text_count, 0);
    assert_eq!(dollar_hit.theme_count, 1);
    let liquidity_hit = results[0]
        .term_hits
        .iter()
        .find(|hit| hit.term == "liquidity")
        .unwrap();
    assert_eq!(liquidity_hit.text_count, 0);
    assert_eq!(liquidity_hit.theme_count, 1);
}

#[test]
fn theme_matching_preserves_short_theme_tokens() {
    let chunks = vec![chunk(
        "doc",
        "Fx.pdf",
        1,
        0,
        "Balance sheet constraints without the matching label words.",
        "Doc",
        &["fx_swaps"],
    )];

    let results =
        explain_search_chunks_with_filters(&chunks, "fx swaps", 1, &SearchFilters::default());

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].theme_boosts, vec!["theme:fx_swaps"]);
    let fx_hit = results[0]
        .term_hits
        .iter()
        .find(|hit| hit.term == "fx")
        .unwrap();
    assert_eq!(fx_hit.theme_count, 1);
    let swaps_hit = results[0]
        .term_hits
        .iter()
        .find(|hit| hit.term == "swaps")
        .unwrap();
    assert_eq!(swaps_hit.theme_count, 1);
}

#[test]
fn theme_matching_accepts_underscore_label_queries() {
    let chunks = vec![chunk(
        "doc",
        "Fx.pdf",
        1,
        0,
        "Balance sheet constraints without the matching label words.",
        "Doc",
        &["fx_swaps"],
    )];

    let results =
        explain_search_chunks_with_filters(&chunks, "fx_swaps", 1, &SearchFilters::default());

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].theme_boosts, vec!["theme:fx_swaps"]);
}

#[test]
fn explanation_snippet_falls_back_to_first_text_term_match() {
    let chunks = vec![chunk(
        "doc",
        "Doc.pdf",
        1,
        0,
        "Funding markets tightened before collateral became scarce.",
        "Doc",
        &[],
    )];

    let results = explain_search_chunks_with_filters(
        &chunks,
        "collateral repo",
        1,
        &SearchFilters::default(),
    );

    assert_eq!(
        results[0].snippet.as_deref(),
        Some("Funding markets tightened before collateral became scarce.")
    );
}

#[test]
fn snippet_fallback_uses_whole_term_not_embedded_substring() {
    let text = format!(
        "Corporate funding pressure appears first. {} The actual rate token appears later.",
        "filler ".repeat(40)
    );
    let chunks = vec![chunk("doc", "Doc.pdf", 1, 0, &text, "Doc", &[])];

    let results = explain_search_chunks_with_filters(&chunks, "rate", 1, &SearchFilters::default());

    let snippet = results[0].snippet.as_deref().unwrap();
    assert!(snippet.contains("actual rate token"));
    assert!(!snippet.starts_with("Corporate"));
}

#[test]
fn snippet_stays_bounded_to_local_match_window_when_words_repeat() {
    let text = format!(
        "anchor {} middle {} anchor",
        "filler ".repeat(80),
        "target ".repeat(1)
    );
    let chunks = vec![chunk("doc", "Doc.pdf", 1, 0, &text, "Doc", &[])];

    let results =
        explain_search_chunks_with_filters(&chunks, "target", 1, &SearchFilters::default());

    let snippet = results[0].snippet.as_deref().unwrap();
    assert!(snippet.contains("target"));
    assert!(snippet.len() <= 260, "snippet too long: {}", snippet.len());
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
    assert!(
        results[0]["score_breakdown"]["text_phrase"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(results[0]["snippet"]
        .as_str()
        .unwrap()
        .contains("Dollar liquidity matters"));
}

#[test]
fn mcp_research_question_returns_evidence_bundle() {
    let chunks = vec![
        chunk(
            "doc-a",
            "Dollar.pdf",
            1,
            0,
            "Dollar liquidity depends on collateral flows through repo markets.",
            "Dollar Liquidity",
            &["dollar_liquidity", "collateral"],
        ),
        chunk(
            "doc-a",
            "Dollar.pdf",
            2,
            0,
            "Neighboring page explains dealer balance sheet constraints.",
            "Dollar Liquidity",
            &["dollar_liquidity"],
        ),
        chunk(
            "doc-b",
            "Repo.pdf",
            3,
            0,
            "Repo collateral scarcity can tighten dollar funding.",
            "Repo",
            &["repo", "collateral"],
        ),
    ];
    let service = PozsarCorpusMcp::new(chunks);

    let response = service.answer_pozsar_research_question(Parameters(ResearchQuestionParams {
        question: "How does collateral affect dollar liquidity?".to_string(),
        themes: Some(vec!["collateral".to_string()]),
        doc_id: None,
        limit: Some(3),
    }));
    let bundle: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(
        bundle["question"],
        "How does collateral affect dollar liquidity?"
    );
    assert!(bundle["query_plan"].as_array().unwrap().iter().any(|step| {
        step["kind"] == "original_question"
            && step["query"] == "How does collateral affect dollar liquidity?"
    }));
    assert!(bundle["query_plan"]
        .as_array()
        .unwrap()
        .iter()
        .any(|step| step["kind"] == "theme_filtered" && step["theme"] == "collateral"));
    assert!(!bundle["evidence"].as_array().unwrap().is_empty());
    assert!(bundle["evidence"].as_array().unwrap().iter().any(|item| {
        item["citation"] == "Dollar.pdf:1"
            && item["query_sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source == "original_question")
    }));
    assert!(bundle["citations"]
        .as_array()
        .unwrap()
        .contains(&Value::String("Dollar.pdf:1".to_string())));
    assert!(bundle["suggested_followups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|followup| followup.as_str().unwrap().contains("collateral")));
}

#[test]
fn mcp_research_question_fetches_context_around_top_hits() {
    let chunks = vec![
        chunk(
            "doc",
            "Context.pdf",
            1,
            0,
            "Previous page setup.",
            "Doc",
            &[],
        ),
        chunk(
            "doc",
            "Context.pdf",
            2,
            0,
            "Dollar liquidity and collateral are the direct hit.",
            "Doc",
            &["collateral"],
        ),
        chunk(
            "doc",
            "Context.pdf",
            3,
            0,
            "Next page implication.",
            "Doc",
            &[],
        ),
    ];
    let service = PozsarCorpusMcp::new(chunks);

    let response = service.answer_pozsar_research_question(Parameters(ResearchQuestionParams {
        question: "dollar liquidity collateral".to_string(),
        themes: None,
        doc_id: Some("doc".to_string()),
        limit: Some(1),
    }));
    let bundle: Value = serde_json::from_str(&response).unwrap();
    let context = bundle["evidence"][0]["context"].as_array().unwrap();

    assert_eq!(
        context
            .iter()
            .map(|passage| passage["page"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

#[test]
fn mcp_research_question_limits_after_sorting_merged_candidates() {
    let chunks = vec![
        chunk(
            "weak",
            "Weak.pdf",
            1,
            0,
            "What does collateral affect dollar liquidity?",
            "Weak",
            &[],
        ),
        chunk(
            "strong",
            "Strong.pdf",
            2,
            0,
            "Collateral dollar liquidity. Collateral dollar liquidity. Collateral dollar liquidity.",
            "Collateral Dollar Liquidity",
            &["collateral"],
        ),
    ];
    let service = PozsarCorpusMcp::new(chunks);

    let response = service.answer_pozsar_research_question(Parameters(ResearchQuestionParams {
        question: "What does collateral affect dollar liquidity?".to_string(),
        themes: None,
        doc_id: None,
        limit: Some(1),
    }));
    let bundle: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(bundle["evidence"].as_array().unwrap().len(), 1);
    assert_eq!(bundle["evidence"][0]["citation"], "Strong.pdf:2");
    assert!(bundle["evidence"][0]["query_sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source == "key_phrase"));
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
