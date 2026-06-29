use std::path::PathBuf;
use std::process::Command;

#[test]
fn eval_search_reports_passes_and_summary() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases = workspace
        .join("eval")
        .join("fixtures")
        .join("pozsar_eval.json");

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("retrieval eval"));
    assert!(stdout.contains("PASS collateral dollar liquidity"));
    assert!(stdout.contains("PASS bretton woods commodities"));
    assert!(stdout.contains("summary: 2/2 passed"));
}

#[test]
fn eval_search_reports_missing_expected_citations() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases =
        std::env::temp_dir().join(format!("pozsar_eval_missing_{}.json", std::process::id()));
    std::fs::write(
        &cases,
        r#"[
  {
    "name": "missing citation",
    "query": "collateral dollar liquidity",
    "expected_citations": ["Missing.pdf:99"]
  }
]"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("FAIL missing citation"));
    assert!(stdout.contains("missing: Missing.pdf:99"));
    assert!(stdout.contains("summary: 0/1 passed"));
}

#[test]
fn eval_search_can_emit_json_report() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases = workspace
        .join("eval")
        .join("fixtures")
        .join("pozsar_eval.json");

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["total"], 2);
    assert_eq!(report["passed"], 2);
    assert_eq!(report["failed"], 0);
    assert_eq!(report["cases"][0]["name"], "collateral dollar liquidity");
    assert_eq!(report["cases"][0]["passed"], true);
    assert_eq!(
        report["cases"][0]["expected_ranks"][0]["citation"],
        "The_Safe_Asset_Glut.pdf:7"
    );
    assert_eq!(report["cases"][0]["expected_ranks"][0]["rank"], 2);
}

#[test]
fn eval_search_can_run_raw_search_tool() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases = workspace
        .join("eval")
        .join("fixtures")
        .join("pozsar_eval.json");

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
            "--tool",
            "search",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["tool"], "search");
    assert_eq!(report["total"], 2);
    assert_eq!(report["passed"], 2);
    assert_eq!(
        report["cases"][0]["returned_citations"],
        serde_json::json!(["Bretton-Woods-III.pdf:2", "The_Safe_Asset_Glut.pdf:7"])
    );
}

#[test]
fn eval_search_enforces_per_case_max_rank() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases =
        std::env::temp_dir().join(format!("pozsar_eval_max_rank_{}.json", std::process::id()));
    std::fs::write(
        &cases,
        r#"[
  {
    "name": "rank too low",
    "query": "collateral dollar liquidity",
    "themes": ["collateral"],
    "expected_citations": ["The_Safe_Asset_Glut.pdf:7"],
    "max_rank": 1
  }
]"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["passed"], 0);
    assert_eq!(report["cases"][0]["max_rank"], 1);
    assert_eq!(
        report["cases"][0]["missing_citations"],
        serde_json::json!([])
    );
    assert_eq!(
        report["cases"][0]["rank_failures"],
        serde_json::json!([
            {
                "citation": "The_Safe_Asset_Glut.pdf:7",
                "rank": 2,
                "max_rank": 1
            }
        ])
    );
}

#[test]
fn eval_search_accepts_expected_citation_within_max_rank() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases = std::env::temp_dir().join(format!(
        "pozsar_eval_max_rank_pass_{}.json",
        std::process::id()
    ));
    std::fs::write(
        &cases,
        r#"[
  {
    "name": "rank within max",
    "query": "collateral dollar liquidity",
    "themes": ["collateral"],
    "expected_citations": ["The_Safe_Asset_Glut.pdf:7"],
    "max_rank": 2
  }
]"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["passed"], 1);
    assert_eq!(report["cases"][0]["max_rank"], 2);
    assert_eq!(report["cases"][0]["rank_failures"], serde_json::json!([]));
}

#[test]
fn eval_search_fail_fast_stops_after_first_failure() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases =
        std::env::temp_dir().join(format!("pozsar_eval_fail_fast_{}.json", std::process::id()));
    std::fs::write(
        &cases,
        r#"[
  {
    "name": "first pass",
    "query": "outside money commodities",
    "expected_citations": ["Bretton-Woods-III.pdf:2"]
  },
  {
    "name": "first fail",
    "query": "collateral dollar liquidity",
    "expected_citations": ["Missing.pdf:99"]
  },
  {
    "name": "not evaluated",
    "query": "safe assets",
    "expected_citations": ["The_Safe_Asset_Glut.pdf:7"]
  }
]"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
            "--format",
            "json",
            "--fail-fast",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["fail_fast"], true);
    assert_eq!(report["stopped_early"], true);
    assert_eq!(report["total"], 2);
    assert_eq!(report["passed"], 1);
    assert_eq!(report["failed"], 1);
    assert_eq!(report["cases"].as_array().unwrap().len(), 2);
    assert_eq!(report["cases"][0]["name"], "first pass");
    assert_eq!(report["cases"][1]["name"], "first fail");
}

#[test]
fn eval_search_writes_json_report_to_output_path() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases = workspace
        .join("eval")
        .join("fixtures")
        .join("pozsar_eval.json");
    let report_path = std::env::temp_dir()
        .join(format!("pozsar_eval_output_{}", std::process::id()))
        .join("nested")
        .join("report.json");

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
            "--format",
            "json",
            "--output",
            report_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("wrote eval report:"));
    assert!(stdout.contains("summary: 2/2 passed"));
    assert!(!stdout.contains("\"cases\""));

    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
    assert_eq!(report["tool"], "research-question");
    assert_eq!(report["passed"], 2);
}

#[test]
fn eval_search_writes_failure_report_before_nonzero_exit() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases = std::env::temp_dir().join(format!(
        "pozsar_eval_output_fail_{}.json",
        std::process::id()
    ));
    let report_path = std::env::temp_dir()
        .join(format!("pozsar_eval_output_fail_{}", std::process::id()))
        .join("report.json");
    std::fs::write(
        &cases,
        r#"[
  {
    "name": "missing citation",
    "query": "collateral dollar liquidity",
    "expected_citations": ["Missing.pdf:99"]
  }
]"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
            "--format",
            "json",
            "--output",
            report_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("wrote eval report:"));
    assert!(stdout.contains("summary: 0/1 passed"));

    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
    assert_eq!(report["failed"], 1);
    assert_eq!(report["cases"][0]["missing_citations"][0], "Missing.pdf:99");
}

#[test]
fn eval_search_includes_case_category_and_notes() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases =
        std::env::temp_dir().join(format!("pozsar_eval_metadata_{}.json", std::process::id()));
    std::fs::write(
        &cases,
        r#"[
  {
    "name": "metadata case",
    "category": "dollar_liquidity",
    "notes": "Checks that metadata is preserved in eval reports.",
    "query": "collateral dollar liquidity",
    "expected_citations": ["The_Safe_Asset_Glut.pdf:7"]
  }
]"#,
    )
    .unwrap();

    let json_output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        json_output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&json_output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&json_output.stdout).unwrap();
    assert_eq!(report["cases"][0]["category"], "dollar_liquidity");
    assert_eq!(
        report["cases"][0]["notes"],
        "Checks that metadata is preserved in eval reports."
    );

    let text_output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
        ])
        .output()
        .unwrap();
    assert!(
        text_output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&text_output.stderr)
    );
    let stdout = String::from_utf8(text_output.stdout).unwrap();
    assert!(stdout.contains("  category: dollar_liquidity"));
    assert!(!stdout.contains("Checks that metadata is preserved"));
}

#[test]
fn eval_search_reports_category_summary() {
    let workspace = workspace_root();
    let chunks = workspace
        .join("crates")
        .join("pozsar-mcp")
        .join("tests")
        .join("fixtures")
        .join("pozsar_chunks.jsonl");
    let cases = std::env::temp_dir().join(format!(
        "pozsar_eval_category_summary_{}.json",
        std::process::id()
    ));
    std::fs::write(
        &cases,
        r#"[
  {
    "name": "category pass",
    "category": "dollar_liquidity",
    "query": "outside money commodities",
    "expected_citations": ["Bretton-Woods-III.pdf:2"]
  },
  {
    "name": "category fail",
    "category": "dollar_liquidity",
    "query": "collateral dollar liquidity",
    "expected_citations": ["Missing.pdf:99"]
  },
  {
    "name": "uncategorized pass",
    "query": "safe assets",
    "expected_citations": ["The_Safe_Asset_Glut.pdf:7"]
  }
]"#,
    )
    .unwrap();

    let json_output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(!json_output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&json_output.stdout).unwrap();
    assert_eq!(
        report["category_summary"],
        serde_json::json!([
            {
                "category": "dollar_liquidity",
                "total": 2,
                "passed": 1,
                "failed": 1
            },
            {
                "category": "uncategorized",
                "total": 1,
                "passed": 1,
                "failed": 0
            }
        ])
    );

    let text_output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "eval-search",
            "--chunks",
            chunks.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--limit",
            "3",
        ])
        .output()
        .unwrap();
    assert!(!text_output.status.success());
    let stdout = String::from_utf8(text_output.stdout).unwrap();
    assert!(stdout.contains("categories:"));
    assert!(stdout.contains("  dollar_liquidity: 1/2 passed"));
    assert!(stdout.contains("  uncategorized: 1/1 passed"));
}

#[test]
fn verify_sources_passes_when_bibliography_docs_and_source_map_match() {
    let fixture = source_fixture_dir("pass");
    let docs = fixture.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("alpha.pdf"), b"alpha\n").unwrap();
    std::fs::write(docs.join("beta file.pdf"), b"beta\n").unwrap();
    let bibliography = fixture.join("bibliography.html");
    std::fs::write(
        &bibliography,
        r#"<html>
  <a href="https://example.test/public/alpha.pdf">Alpha</a>
  <a href="https://example.test/public/beta%20file.pdf">Beta</a>
</html>"#,
    )
    .unwrap();
    let source_map = fixture.join("SOURCE_MAP.md");
    std::fs::write(
        &source_map,
        r#"# Source Map

| Local PDF | Source | SHA-256 |
|---|---|---|
| `alpha.pdf` | <https://example.test/public/alpha.pdf> | `b6a98d9ce9a2d9149288fa3df42d377c3e42737afdcdaf714e33c0a100b51060` |
| `beta file.pdf` | <https://example.test/public/beta%20file.pdf> | `f2c82decdd7181cf98945929a62598db7e6b477e11f6e0eb0ae97020eff151ad` |
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "verify-sources",
            "--docs",
            docs.to_str().unwrap(),
            "--bibliography",
            bibliography.to_str().unwrap(),
            "--source-map",
            source_map.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("source verification"));
    assert!(stdout.contains("docs_pdfs: 2"));
    assert!(stdout.contains("bibliography_pdf_links: 2"));
    assert!(stdout.contains("missing_pdfs: (none)"));
    assert!(stdout.contains("extra_links: (none)"));
    assert!(stdout.contains("source_map_url_mismatches: (none)"));
    assert!(stdout.contains("hash_mismatches: (none)"));
    assert!(stdout.contains("summary: PASS"));
}

#[test]
fn verify_sources_fails_on_missing_extra_and_hash_mismatch() {
    let fixture = source_fixture_dir("fail");
    let docs = fixture.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("alpha.pdf"), b"changed\n").unwrap();
    std::fs::write(docs.join("missing-from-bibliography.pdf"), b"local only\n").unwrap();
    let bibliography = fixture.join("bibliography.html");
    std::fs::write(
        &bibliography,
        r#"<html>
  <a href="https://example.test/public/alpha.pdf">Alpha</a>
  <a href="https://example.test/public/extra.pdf">Extra</a>
</html>"#,
    )
    .unwrap();
    let source_map = fixture.join("SOURCE_MAP.md");
    std::fs::write(
        &source_map,
        r#"# Source Map

| `alpha.pdf` | <https://example.test/public/alpha.pdf> | `b6a98d9ce9a2d9149288fa3df42d377c3e42737afdcdaf714e33c0a100b51060` |
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "verify-sources",
            "--docs",
            docs.to_str().unwrap(),
            "--bibliography",
            bibliography.to_str().unwrap(),
            "--source-map",
            source_map.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("missing_pdfs: missing-from-bibliography.pdf"));
    assert!(stdout.contains("extra_links: extra.pdf"));
    assert!(stdout.contains("source_map_missing_entries: missing-from-bibliography.pdf"));
    assert!(stdout.contains("hash_mismatches: alpha.pdf"));
    assert!(stdout.contains("summary: FAIL"));
}

#[test]
fn verify_sources_fails_when_source_map_url_does_not_match_bibliography() {
    let fixture = source_fixture_dir("wrong_url");
    let docs = fixture.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("alpha.pdf"), b"alpha\n").unwrap();
    let bibliography = fixture.join("bibliography.html");
    std::fs::write(
        &bibliography,
        r#"<html>
  <a href="https://example.test/public/alpha.pdf">Alpha</a>
</html>"#,
    )
    .unwrap();
    let source_map = fixture.join("SOURCE_MAP.md");
    std::fs::write(
        &source_map,
        r#"# Source Map

| Local PDF | Source | SHA-256 |
|---|---|---|
| `alpha.pdf` | <https://stale.example.test/public/alpha.pdf> | `b6a98d9ce9a2d9149288fa3df42d377c3e42737afdcdaf714e33c0a100b51060` |
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "verify-sources",
            "--docs",
            docs.to_str().unwrap(),
            "--bibliography",
            bibliography.to_str().unwrap(),
            "--source-map",
            source_map.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("missing_pdfs: (none)"));
    assert!(stdout.contains("extra_links: (none)"));
    assert!(stdout.contains("source_map_missing_entries: (none)"));
    assert!(stdout.contains("source_map_url_mismatches: alpha.pdf"));
    assert!(stdout.contains("hash_mismatches: (none)"));
    assert!(stdout.contains("summary: FAIL"));
}

#[test]
fn verify_sources_fails_when_source_map_hashes_are_swapped_between_pdfs() {
    let fixture = source_fixture_dir("swapped");
    let docs = fixture.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("alpha.pdf"), b"alpha\n").unwrap();
    std::fs::write(docs.join("beta.pdf"), b"beta\n").unwrap();
    let bibliography = fixture.join("bibliography.html");
    std::fs::write(
        &bibliography,
        r#"<html>
  <a href="https://example.test/public/alpha.pdf">Alpha</a>
  <a href="https://example.test/public/beta.pdf">Beta</a>
</html>"#,
    )
    .unwrap();
    let source_map = fixture.join("SOURCE_MAP.md");
    std::fs::write(
        &source_map,
        r#"# Source Map

| Local PDF | Source | SHA-256 |
|---|---|---|
| `alpha.pdf` | <https://example.test/public/alpha.pdf> | `f2c82decdd7181cf98945929a62598db7e6b477e11f6e0eb0ae97020eff151ad` |
| `beta.pdf` | <https://example.test/public/beta.pdf> | `b6a98d9ce9a2d9149288fa3df42d377c3e42737afdcdaf714e33c0a100b51060` |
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "verify-sources",
            "--docs",
            docs.to_str().unwrap(),
            "--bibliography",
            bibliography.to_str().unwrap(),
            "--source-map",
            source_map.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("missing_pdfs: (none)"));
    assert!(stdout.contains("extra_links: (none)"));
    assert!(stdout.contains("source_map_missing_entries: (none)"));
    assert!(stdout.contains("source_map_url_mismatches: (none)"));
    assert!(stdout.contains("hash_mismatches: alpha.pdf, beta.pdf"));
    assert!(stdout.contains("summary: FAIL"));
}

#[test]
fn download_sources_downloads_missing_pdfs_from_source_map() {
    let fixture = source_fixture_dir("download");
    let source_dir = fixture.join("source");
    let docs = fixture.join("docs");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(source_dir.join("alpha.pdf"), b"alpha\n").unwrap();
    let source_map = fixture.join("SOURCE_MAP.md");
    std::fs::write(
        &source_map,
        format!(
            r#"# Source Map

This prose mentions `.pdf` and `docs/*.pdf`, but those are not source rows.

| Local PDF | Source | SHA-256 |
|---|---|---|
| `alpha.pdf` | <file://{}> | `b6a98d9ce9a2d9149288fa3df42d377c3e42737afdcdaf714e33c0a100b51060` |
"#,
            source_dir.join("alpha.pdf").display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "download-sources",
            "--docs",
            docs.to_str().unwrap(),
            "--source-map",
            source_map.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("source download"));
    assert!(stdout.contains("sources: 1"));
    assert!(stdout.contains("downloaded: 1"));
    assert!(stdout.contains("skipped_existing: 0"));
    assert!(stdout.contains("summary: PASS"));
    assert_eq!(std::fs::read(docs.join("alpha.pdf")).unwrap(), b"alpha\n");
}

#[test]
fn download_sources_refuses_to_overwrite_existing_mismatched_pdf_by_default() {
    let fixture = source_fixture_dir("download_existing_mismatch");
    let source_dir = fixture.join("source");
    let docs = fixture.join("docs");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(source_dir.join("alpha.pdf"), b"alpha\n").unwrap();
    std::fs::write(docs.join("alpha.pdf"), b"wrong\n").unwrap();
    let source_map = fixture.join("SOURCE_MAP.md");
    std::fs::write(
        &source_map,
        format!(
            r#"# Source Map

| Local PDF | Source | SHA-256 |
|---|---|---|
| `alpha.pdf` | <file://{}> | `b6a98d9ce9a2d9149288fa3df42d377c3e42737afdcdaf714e33c0a100b51060` |
"#,
            source_dir.join("alpha.pdf").display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "download-sources",
            "--docs",
            docs.to_str().unwrap(),
            "--source-map",
            source_map.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("existing hash mismatch for alpha.pdf"));
    assert!(stdout.contains("summary: FAIL"));
    assert_eq!(std::fs::read(docs.join("alpha.pdf")).unwrap(), b"wrong\n");
}

#[test]
fn download_sources_overwrites_existing_mismatched_pdf_when_requested() {
    let fixture = source_fixture_dir("download_overwrite");
    let source_dir = fixture.join("source");
    let docs = fixture.join("docs");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(source_dir.join("alpha.pdf"), b"alpha\n").unwrap();
    std::fs::write(docs.join("alpha.pdf"), b"wrong\n").unwrap();
    let source_map = fixture.join("SOURCE_MAP.md");
    std::fs::write(
        &source_map,
        format!(
            r#"# Source Map

| Local PDF | Source | SHA-256 |
|---|---|---|
| `alpha.pdf` | <file://{}> | `b6a98d9ce9a2d9149288fa3df42d377c3e42737afdcdaf714e33c0a100b51060` |
"#,
            source_dir.join("alpha.pdf").display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_corpus"))
        .args([
            "download-sources",
            "--docs",
            docs.to_str().unwrap(),
            "--source-map",
            source_map.to_str().unwrap(),
            "--overwrite",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("downloaded: 1"));
    assert!(stdout.contains("summary: PASS"));
    assert_eq!(std::fs::read(docs.join("alpha.pdf")).unwrap(), b"alpha\n");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn source_fixture_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "pozsar_verify_sources_{}_{}",
        name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}
