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

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
