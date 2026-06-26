use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use pozsar_kb::artifacts::{
    read_chunks_jsonl, write_chunks_jsonl, write_manifest, write_pages_jsonl,
};
use pozsar_kb::chunk::chunk_pages;
use pozsar_kb::extract::extract_manifest_pages;
use pozsar_kb::inspect::{inspect_artifacts, CorpusInspection};
use pozsar_kb::manifest::build_manifest;
use pozsar_kb::themes::tag_chunks;
use pozsar_mcp::research::{answer_research_question, ResearchQuestionBundle};
use pozsar_mcp::tools::ResearchQuestionParams;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "corpus")]
#[command(about = "Build the local Pozsar PDF corpus")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Build {
        #[arg(long, default_value = "docs")]
        docs: PathBuf,
        #[arg(long, default_value = "data/knowledge")]
        out: PathBuf,
        #[arg(long, default_value_t = 1800)]
        max_chars: usize,
        #[arg(long, default_value_t = 250)]
        overlap_chars: usize,
    },
    Inspect {
        #[arg(long, default_value = "data/knowledge")]
        out: PathBuf,
    },
    EvalSearch {
        #[arg(long)]
        chunks: PathBuf,
        #[arg(long)]
        cases: PathBuf,
        #[arg(long, default_value_t = 5)]
        limit: u64,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Build {
            docs,
            out,
            max_chars,
            overlap_chars,
        } => {
            let manifest = build_manifest(&docs)?;
            let pages = extract_manifest_pages(&manifest)?;
            let chunks = tag_chunks(chunk_pages(&pages, max_chars, overlap_chars));

            write_manifest(&manifest, &out.join("manifest.json"))?;
            write_pages_jsonl(&pages, &out.join("extracted_pages.jsonl"))?;
            write_chunks_jsonl(&chunks, &out.join("chunks/pozsar_chunks.jsonl"))?;

            println!(
                "built corpus: {} pdfs, {} pages, {} chunks",
                manifest.len(),
                pages.len(),
                chunks.len()
            );
        }
        Command::Inspect { out } => {
            let report = inspect_artifacts(&out)?;
            print_inspection_report(&report);
            if report.has_validation_issues() {
                bail!(
                    "corpus inspection found {} validation issues",
                    report.validation_issues.len()
                );
            }
        }
        Command::EvalSearch {
            chunks,
            cases,
            limit,
        } => {
            let chunks = read_chunks_jsonl(&chunks)?;
            let cases = read_eval_cases(&cases)?;
            let report = eval_search(&chunks, &cases, limit);
            print_eval_report(&report);
            if report.failed > 0 {
                bail!(
                    "retrieval eval failed: {}/{} passed",
                    report.passed,
                    report.total
                );
            }
        }
    }
    Ok(())
}

fn print_inspection_report(report: &CorpusInspection) {
    println!("corpus inspection");
    println!("documents: {}", report.document_count);
    println!("extracted_pages: {}", report.extracted_page_count);
    println!("chunks: {}", report.chunk_count);
    println!(
        "empty_pages: {} ({:.2}%)",
        report.empty_page_count,
        report.empty_page_ratio * 100.0
    );
    println!(
        "pages_without_chunks: {}",
        report.pages_without_chunks.len()
    );
    println!("validation_issues: {}", report.validation_issues.len());

    println!("themes:");
    if report.theme_counts.is_empty() {
        println!("  (none)");
    } else {
        for theme_count in &report.theme_counts {
            println!("  {}: {}", theme_count.theme, theme_count.chunks);
        }
    }

    if !report.pages_without_chunks.is_empty() {
        println!("pages without chunks:");
        for page in &report.pages_without_chunks {
            println!("  {}", page.citation);
        }
    }

    if !report.validation_issues.is_empty() {
        println!("validation issues:");
        for issue in &report.validation_issues {
            println!("  {}", issue);
        }
    }
}

#[derive(Debug, Deserialize)]
struct EvalCase {
    name: String,
    query: String,
    themes: Option<Vec<String>>,
    doc_id: Option<String>,
    expected_citations: Vec<String>,
}

struct EvalReport {
    total: usize,
    passed: usize,
    failed: usize,
    cases: Vec<EvalCaseReport>,
}

struct EvalCaseReport {
    name: String,
    passed: bool,
    returned_citations: Vec<String>,
    expected_ranks: Vec<ExpectedCitationRank>,
    missing_citations: Vec<String>,
    top_scores: Vec<String>,
}

struct ExpectedCitationRank {
    citation: String,
    rank: Option<usize>,
}

fn read_eval_cases(path: &PathBuf) -> Result<Vec<EvalCase>> {
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

fn eval_search(
    chunks: &[pozsar_kb::chunk::KnowledgeChunk],
    cases: &[EvalCase],
    limit: u64,
) -> EvalReport {
    let cases = cases
        .iter()
        .map(|case| eval_case(chunks, case, limit))
        .collect::<Vec<_>>();
    let passed = cases.iter().filter(|case| case.passed).count();
    let total = cases.len();

    EvalReport {
        total,
        passed,
        failed: total.saturating_sub(passed),
        cases,
    }
}

fn eval_case(
    chunks: &[pozsar_kb::chunk::KnowledgeChunk],
    case: &EvalCase,
    limit: u64,
) -> EvalCaseReport {
    let bundle = answer_research_question(
        chunks,
        ResearchQuestionParams {
            question: case.query.clone(),
            themes: case.themes.clone(),
            doc_id: case.doc_id.clone(),
            limit: Some(limit),
        },
    );
    let returned_citations = bundle.citations.clone();
    let expected_ranks = case
        .expected_citations
        .iter()
        .map(|citation| ExpectedCitationRank {
            citation: citation.clone(),
            rank: citation_rank(&returned_citations, citation),
        })
        .collect::<Vec<_>>();
    let missing_citations = expected_ranks
        .iter()
        .filter(|expected| expected.rank.is_none())
        .map(|expected| expected.citation.clone())
        .collect::<Vec<_>>();
    let top_scores = top_scores(&bundle);

    EvalCaseReport {
        name: case.name.clone(),
        passed: missing_citations.is_empty(),
        returned_citations,
        expected_ranks,
        missing_citations,
        top_scores,
    }
}

fn citation_rank(citations: &[String], expected: &str) -> Option<usize> {
    citations
        .iter()
        .position(|citation| citation == expected)
        .map(|index| index + 1)
}

fn top_scores(bundle: &ResearchQuestionBundle) -> Vec<String> {
    bundle
        .evidence
        .iter()
        .take(3)
        .map(|evidence| format!("{} score={}", evidence.citation, evidence.score))
        .collect()
}

fn print_eval_report(report: &EvalReport) {
    println!("retrieval eval");
    for case in &report.cases {
        let status = if case.passed { "PASS" } else { "FAIL" };
        println!("{status} {}", case.name);
        println!("  returned: {}", display_list(&case.returned_citations));
        println!(
            "  expected_ranks: {}",
            display_expected_ranks(&case.expected_ranks)
        );
        if !case.missing_citations.is_empty() {
            println!("  missing: {}", display_list(&case.missing_citations));
            println!("  top_scores: {}", display_list(&case.top_scores));
        }
    }
    println!("summary: {}/{} passed", report.passed, report.total);
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "(none)".to_string()
    } else {
        values.join(", ")
    }
}

fn display_expected_ranks(ranks: &[ExpectedCitationRank]) -> String {
    if ranks.is_empty() {
        return "(none)".to_string();
    }

    ranks
        .iter()
        .map(|rank| match rank.rank {
            Some(value) => format!("{}@{}", rank.citation, value),
            None => format!("{}@missing", rank.citation),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
