use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use pozsar_kb::artifacts::{
    read_chunks_jsonl, write_chunks_jsonl, write_manifest, write_pages_jsonl,
};
use pozsar_kb::chunk::chunk_pages;
use pozsar_kb::extract::extract_manifest_pages;
use pozsar_kb::inspect::{inspect_artifacts, CorpusInspection};
use pozsar_kb::manifest::build_manifest;
use pozsar_kb::themes::tag_chunks;
use pozsar_mcp::research::{answer_research_question, ResearchQuestionBundle};
use pozsar_mcp::search::{search_chunks_with_filters, SearchFilters};
use pozsar_mcp::tools::ResearchQuestionParams;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
        #[arg(long, value_enum, default_value_t = EvalRetrievalTool::ResearchQuestion)]
        tool: EvalRetrievalTool,
        #[arg(long, value_enum, default_value_t = EvalOutputFormat::Text)]
        format: EvalOutputFormat,
        #[arg(long, default_value_t = false)]
        fail_fast: bool,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum EvalRetrievalTool {
    Search,
    ResearchQuestion,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EvalOutputFormat {
    Text,
    Json,
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
            tool,
            format,
            fail_fast,
            output,
        } => {
            let chunks = read_chunks_jsonl(&chunks)?;
            let cases = read_eval_cases(&cases)?;
            let report = eval_search(&chunks, &cases, limit, tool, fail_fast);
            emit_eval_report(&report, format, output.as_ref())?;
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
    category: Option<String>,
    notes: Option<String>,
    query: String,
    themes: Option<Vec<String>>,
    doc_id: Option<String>,
    expected_citations: Vec<String>,
    max_rank: Option<usize>,
}

#[derive(Serialize)]
struct EvalReport {
    tool: EvalRetrievalTool,
    fail_fast: bool,
    stopped_early: bool,
    total: usize,
    passed: usize,
    failed: usize,
    category_summary: Vec<CategorySummary>,
    cases: Vec<EvalCaseReport>,
}

#[derive(Serialize)]
struct CategorySummary {
    category: String,
    total: usize,
    passed: usize,
    failed: usize,
}

#[derive(Serialize)]
struct EvalCaseReport {
    name: String,
    category: Option<String>,
    notes: Option<String>,
    passed: bool,
    max_rank: Option<usize>,
    returned_citations: Vec<String>,
    expected_ranks: Vec<ExpectedCitationRank>,
    missing_citations: Vec<String>,
    rank_failures: Vec<RankFailure>,
    top_scores: Vec<String>,
}

#[derive(Serialize)]
struct ExpectedCitationRank {
    citation: String,
    rank: Option<usize>,
}

#[derive(Serialize)]
struct RankFailure {
    citation: String,
    rank: usize,
    max_rank: usize,
}

fn read_eval_cases(path: &PathBuf) -> Result<Vec<EvalCase>> {
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

fn eval_search(
    chunks: &[pozsar_kb::chunk::KnowledgeChunk],
    cases: &[EvalCase],
    limit: u64,
    tool: EvalRetrievalTool,
    fail_fast: bool,
) -> EvalReport {
    let mut reports = Vec::new();
    let mut stopped_early = false;

    for (index, case) in cases.iter().enumerate() {
        let report = eval_case(chunks, case, limit, tool);
        let failed = !report.passed;
        reports.push(report);
        if fail_fast && failed && index + 1 < cases.len() {
            stopped_early = true;
            break;
        }
    }

    let passed = reports.iter().filter(|case| case.passed).count();
    let total = reports.len();
    let category_summary = category_summary(&reports);

    EvalReport {
        tool,
        fail_fast,
        stopped_early,
        total,
        passed,
        failed: total.saturating_sub(passed),
        category_summary,
        cases: reports,
    }
}

fn eval_case(
    chunks: &[pozsar_kb::chunk::KnowledgeChunk],
    case: &EvalCase,
    limit: u64,
    tool: EvalRetrievalTool,
) -> EvalCaseReport {
    let result = retrieve_eval_citations(chunks, case, limit, tool);
    let returned_citations = result.citations;
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
    let rank_failures = rank_failures(&expected_ranks, case.max_rank);

    EvalCaseReport {
        name: case.name.clone(),
        category: case.category.clone(),
        notes: case.notes.clone(),
        passed: missing_citations.is_empty() && rank_failures.is_empty(),
        max_rank: case.max_rank,
        returned_citations,
        expected_ranks,
        missing_citations,
        rank_failures,
        top_scores: result.top_scores,
    }
}

struct EvalRetrievalResult {
    citations: Vec<String>,
    top_scores: Vec<String>,
}

fn retrieve_eval_citations(
    chunks: &[pozsar_kb::chunk::KnowledgeChunk],
    case: &EvalCase,
    limit: u64,
    tool: EvalRetrievalTool,
) -> EvalRetrievalResult {
    match tool {
        EvalRetrievalTool::Search => retrieve_search_citations(chunks, case, limit),
        EvalRetrievalTool::ResearchQuestion => {
            retrieve_research_question_citations(chunks, case, limit)
        }
    }
}

fn retrieve_research_question_citations(
    chunks: &[pozsar_kb::chunk::KnowledgeChunk],
    case: &EvalCase,
    limit: u64,
) -> EvalRetrievalResult {
    let bundle = answer_research_question(
        chunks,
        ResearchQuestionParams {
            question: case.query.clone(),
            themes: case.themes.clone(),
            doc_id: case.doc_id.clone(),
            limit: Some(limit),
        },
    );

    EvalRetrievalResult {
        citations: bundle.citations.clone(),
        top_scores: top_scores(&bundle),
    }
}

fn retrieve_search_citations(
    chunks: &[pozsar_kb::chunk::KnowledgeChunk],
    case: &EvalCase,
    limit: u64,
) -> EvalRetrievalResult {
    let mut citations = Vec::new();
    let themes = case.themes.as_deref().unwrap_or(&[]);

    if themes.is_empty() {
        append_search_citations(chunks, case, limit, None, &mut citations);
    } else {
        for theme in themes {
            append_search_citations(chunks, case, limit, Some(theme.as_str()), &mut citations);
        }
    }
    citations.truncate(limit as usize);

    EvalRetrievalResult {
        citations,
        top_scores: Vec::new(),
    }
}

fn append_search_citations(
    chunks: &[pozsar_kb::chunk::KnowledgeChunk],
    case: &EvalCase,
    limit: u64,
    theme: Option<&str>,
    citations: &mut Vec<String>,
) {
    let filters = SearchFilters {
        theme: theme.map(str::to_string),
        doc_id: case.doc_id.clone(),
        ..Default::default()
    };
    let passages = search_chunks_with_filters(chunks, &case.query, limit as usize, &filters);
    for passage in passages {
        if !citations.contains(&passage.citation) {
            citations.push(passage.citation);
        }
    }
}

fn citation_rank(citations: &[String], expected: &str) -> Option<usize> {
    citations
        .iter()
        .position(|citation| citation == expected)
        .map(|index| index + 1)
}

fn rank_failures(
    expected_ranks: &[ExpectedCitationRank],
    max_rank: Option<usize>,
) -> Vec<RankFailure> {
    let Some(max_rank) = max_rank else {
        return Vec::new();
    };

    expected_ranks
        .iter()
        .filter_map(|expected| {
            let rank = expected.rank?;
            (rank > max_rank).then(|| RankFailure {
                citation: expected.citation.clone(),
                rank,
                max_rank,
            })
        })
        .collect()
}

fn top_scores(bundle: &ResearchQuestionBundle) -> Vec<String> {
    bundle
        .evidence
        .iter()
        .take(3)
        .map(|evidence| format!("{} score={}", evidence.citation, evidence.score))
        .collect()
}

fn category_summary(cases: &[EvalCaseReport]) -> Vec<CategorySummary> {
    let mut categories = BTreeMap::<String, CategorySummary>::new();
    for case in cases {
        let category = case
            .category
            .clone()
            .unwrap_or_else(|| "uncategorized".to_string());
        let summary = categories
            .entry(category.clone())
            .or_insert_with(|| CategorySummary {
                category,
                total: 0,
                passed: 0,
                failed: 0,
            });
        summary.total += 1;
        if case.passed {
            summary.passed += 1;
        } else {
            summary.failed += 1;
        }
    }
    categories.into_values().collect()
}

fn emit_eval_report(
    report: &EvalReport,
    format: EvalOutputFormat,
    output: Option<&PathBuf>,
) -> Result<()> {
    let rendered = render_eval_report(report, format)?;
    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, rendered)?;
        println!("wrote eval report: {}", output.display());
        println!("summary: {}/{} passed", report.passed, report.total);
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn render_eval_report(report: &EvalReport, format: EvalOutputFormat) -> Result<String> {
    if matches!(format, EvalOutputFormat::Json) {
        return Ok(format!("{}\n", serde_json::to_string_pretty(report)?));
    }

    let mut output = String::new();
    output.push_str("retrieval eval\n");
    output.push_str(&format!("tool: {}\n", eval_tool_name(report.tool)));
    output.push_str(&format!("fail_fast: {}\n", report.fail_fast));
    for case in &report.cases {
        let status = if case.passed { "PASS" } else { "FAIL" };
        output.push_str(&format!("{status} {}\n", case.name));
        if let Some(category) = &case.category {
            output.push_str(&format!("  category: {category}\n"));
        }
        output.push_str(&format!(
            "  returned: {}\n",
            display_list(&case.returned_citations)
        ));
        output.push_str(&format!(
            "  expected_ranks: {}",
            display_expected_ranks(&case.expected_ranks)
        ));
        output.push('\n');
        if !case.missing_citations.is_empty() {
            output.push_str(&format!(
                "  missing: {}\n",
                display_list(&case.missing_citations)
            ));
        }
        if !case.rank_failures.is_empty() {
            output.push_str(&format!(
                "  rank_failures: {}",
                display_rank_failures(&case.rank_failures)
            ));
            output.push('\n');
        }
        if !case.missing_citations.is_empty() || !case.rank_failures.is_empty() {
            output.push_str(&format!(
                "  top_scores: {}\n",
                display_list(&case.top_scores)
            ));
        }
    }
    output.push_str(&format!(
        "summary: {}/{} passed\n",
        report.passed, report.total
    ));
    output.push_str("categories:\n");
    if report.category_summary.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for category in &report.category_summary {
            output.push_str(&format!(
                "  {}: {}/{} passed\n",
                category.category, category.passed, category.total
            ));
        }
    }
    Ok(output)
}

fn eval_tool_name(tool: EvalRetrievalTool) -> &'static str {
    match tool {
        EvalRetrievalTool::Search => "search",
        EvalRetrievalTool::ResearchQuestion => "research-question",
    }
}

fn display_rank_failures(rank_failures: &[RankFailure]) -> String {
    if rank_failures.is_empty() {
        return "(none)".to_string();
    }

    rank_failures
        .iter()
        .map(|failure| {
            format!(
                "{}@{} > max_rank {}",
                failure.citation, failure.rank, failure.max_rank
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
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
