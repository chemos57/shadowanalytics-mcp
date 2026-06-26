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

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
