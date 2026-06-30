use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use market_context::build_market_context_from_csv;
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
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
    VerifySources {
        #[arg(long, default_value = "docs")]
        docs: PathBuf,
        #[arg(long, default_value = "Zoltan-Pozsar-Bibliography.html")]
        bibliography: PathBuf,
        #[arg(long, default_value = "docs/SOURCE_MAP.md")]
        source_map: PathBuf,
    },
    DownloadSources {
        #[arg(long, default_value = "docs")]
        docs: PathBuf,
        #[arg(long, default_value = "docs/SOURCE_MAP.md")]
        source_map: PathBuf,
        #[arg(long, default_value_t = false)]
        overwrite: bool,
    },
    MarketContext {
        #[arg(long)]
        prices: PathBuf,
        #[arg(long)]
        out: PathBuf,
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
        Command::VerifySources {
            docs,
            bibliography,
            source_map,
        } => {
            let report = verify_sources(&docs, &bibliography, &source_map)?;
            print_source_verification_report(&report);
            if report.has_mismatches() {
                bail!("source verification failed");
            }
        }
        Command::DownloadSources {
            docs,
            source_map,
            overwrite,
        } => {
            let report = download_sources(&docs, &source_map, overwrite)?;
            print_source_download_report(&report);
            if report.has_failures() {
                bail!("source download failed");
            }
        }
        Command::MarketContext { prices, out } => {
            let context = build_market_context_from_csv(&prices)?;
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(
                &out,
                format!("{}\n", serde_json::to_string_pretty(&context)?),
            )?;
            println!("wrote market context: {}", out.display());
            println!("as_of: {}", context.as_of);
            println!("assets: {}", context.assets.len());
            println!("risk_regime: {}", context.cross_asset.risk_regime);
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

#[derive(Debug)]
struct SourceVerificationReport {
    docs_pdf_count: usize,
    bibliography_pdf_link_count: usize,
    missing_pdfs: Vec<String>,
    extra_links: Vec<String>,
    source_map_missing_entries: Vec<String>,
    source_map_url_mismatches: Vec<String>,
    hash_mismatches: Vec<String>,
}

impl SourceVerificationReport {
    fn has_mismatches(&self) -> bool {
        !self.missing_pdfs.is_empty()
            || !self.extra_links.is_empty()
            || !self.source_map_missing_entries.is_empty()
            || !self.source_map_url_mismatches.is_empty()
            || !self.hash_mismatches.is_empty()
    }
}

fn verify_sources(
    docs: &Path,
    bibliography: &Path,
    source_map: &Path,
) -> Result<SourceVerificationReport> {
    let docs_pdfs = docs_pdf_hashes(docs)?;
    let bibliography_html = fs::read_to_string(bibliography)?;
    let bibliography_pdf_links = extract_pdf_links(&bibliography_html);
    let source_map_markdown = fs::read_to_string(source_map)?;
    let source_map_entries = parse_source_map_entries(&source_map_markdown);

    let docs_names = docs_pdfs.keys().cloned().collect::<BTreeSet<_>>();
    let bibliography_pdfs = bibliography_pdf_links
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_pdfs = docs_names
        .difference(&bibliography_pdfs)
        .cloned()
        .collect::<Vec<_>>();
    let extra_links = bibliography_pdfs
        .difference(&docs_names)
        .cloned()
        .collect::<Vec<_>>();

    let mut source_map_missing_entries = Vec::new();
    let mut source_map_url_mismatches = Vec::new();
    let mut hash_mismatches = Vec::new();
    for (pdf, hash) in &docs_pdfs {
        match source_map_entries.get(pdf) {
            None => source_map_missing_entries.push(pdf.clone()),
            Some(entry) => {
                if !entry.hashes.contains(hash) {
                    hash_mismatches.push(pdf.clone());
                }
                if let Some(expected_url) = bibliography_pdf_links.get(pdf) {
                    if !entry.urls.contains(expected_url) {
                        source_map_url_mismatches.push(pdf.clone());
                    }
                }
            }
        }
    }

    Ok(SourceVerificationReport {
        docs_pdf_count: docs_pdfs.len(),
        bibliography_pdf_link_count: bibliography_pdfs.len(),
        missing_pdfs,
        extra_links,
        source_map_missing_entries,
        source_map_url_mismatches,
        hash_mismatches,
    })
}

fn docs_pdf_hashes(docs: &Path) -> Result<BTreeMap<String, String>> {
    let mut pdfs = BTreeMap::new();
    for entry in fs::read_dir(docs)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !has_pdf_extension(&path) {
            continue;
        }
        let Some(file_name) = path.file_name() else {
            continue;
        };
        let file_name = file_name.to_string_lossy().to_string();
        pdfs.insert(file_name, sha256_file(&path)?);
    }
    Ok(pdfs)
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

fn has_pdf_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
}

fn extract_pdf_links(html: &str) -> BTreeMap<String, String> {
    extract_href_values(html)
        .into_iter()
        .filter_map(|href| pdf_file_name_from_href(&href).map(|file_name| (file_name, href)))
        .collect()
}

fn extract_href_values(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut hrefs = Vec::new();
    let mut offset = 0;

    while let Some(relative_index) = lower[offset..].find("href") {
        let href_index = offset + relative_index;
        let mut index = href_index + "href".len();
        index = skip_ascii_whitespace(html, index);
        if html.as_bytes().get(index) != Some(&b'=') {
            offset = index;
            continue;
        }
        index += 1;
        index = skip_ascii_whitespace(html, index);

        let Some(first) = html.as_bytes().get(index).copied() else {
            break;
        };
        let (start, end) = if first == b'"' || first == b'\'' {
            let quote = first;
            let start = index + 1;
            let end = html.as_bytes()[start..]
                .iter()
                .position(|byte| *byte == quote)
                .map(|relative_end| start + relative_end);
            let Some(end) = end else {
                break;
            };
            (start, end)
        } else {
            let start = index;
            let end = html.as_bytes()[start..]
                .iter()
                .position(|byte| byte.is_ascii_whitespace() || *byte == b'>')
                .map(|relative_end| start + relative_end)
                .unwrap_or(html.len());
            (start, end)
        };

        hrefs.push(html[start..end].to_string());
        offset = end.saturating_add(1);
    }

    hrefs
}

fn skip_ascii_whitespace(value: &str, mut index: usize) -> usize {
    while value
        .as_bytes()
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    index
}

fn pdf_file_name_from_href(href: &str) -> Option<String> {
    let path = href
        .split_once('#')
        .map(|(path, _)| path)
        .unwrap_or(href)
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(href);
    if !path.to_ascii_lowercase().ends_with(".pdf") {
        return None;
    }
    let file_name = path.rsplit('/').next()?;
    if file_name.is_empty() {
        return None;
    }
    Some(percent_decode(file_name))
}

#[derive(Debug, Default)]
struct SourceMapEntry {
    urls: BTreeSet<String>,
    hashes: BTreeSet<String>,
}

fn parse_source_map_entries(markdown: &str) -> BTreeMap<String, SourceMapEntry> {
    let mut entries = BTreeMap::<String, SourceMapEntry>::new();
    for line in markdown.lines() {
        let pdfs = pdf_names_in_text(line);
        if pdfs.is_empty() {
            continue;
        }
        let urls = pdf_urls_in_text(line);
        let hashes = sha256_hashes_in_text(line);
        for pdf in pdfs {
            let entry = entries.entry(pdf).or_default();
            entry.urls.extend(urls.iter().cloned());
            entry.hashes.extend(hashes.iter().cloned());
        }
    }
    entries
}

fn pdf_names_in_text(text: &str) -> Vec<String> {
    text.split('`')
        .enumerate()
        .filter_map(|(index, value)| {
            (index % 2 == 1 && is_source_map_pdf_name(value)).then(|| value.to_string())
        })
        .collect()
}

fn is_source_map_pdf_name(value: &str) -> bool {
    value.to_ascii_lowercase().ends_with(".pdf")
        && value.len() > ".pdf".len()
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains('*')
}

fn sha256_hashes_in_text(text: &str) -> BTreeSet<String> {
    text.split('`')
        .enumerate()
        .filter_map(|(index, value)| {
            (index % 2 == 1 && is_sha256_hex(value)).then(|| value.to_ascii_lowercase())
        })
        .collect()
}

fn pdf_urls_in_text(text: &str) -> BTreeSet<String> {
    let mut urls = BTreeSet::new();
    let mut offset = 0;
    while let Some(start_relative) = text[offset..].find('<') {
        let start = offset + start_relative + 1;
        let Some(end_relative) = text[start..].find('>') else {
            break;
        };
        let end = start + end_relative;
        let value = &text[start..end];
        if pdf_file_name_from_href(value).is_some() {
            urls.insert(value.to_string());
        }
        offset = end + 1;
    }
    urls
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                output.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).to_string()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn print_source_verification_report(report: &SourceVerificationReport) {
    println!("source verification");
    println!("docs_pdfs: {}", report.docs_pdf_count);
    println!(
        "bibliography_pdf_links: {}",
        report.bibliography_pdf_link_count
    );
    println!("missing_pdfs: {}", display_list(&report.missing_pdfs));
    println!("extra_links: {}", display_list(&report.extra_links));
    println!(
        "source_map_missing_entries: {}",
        display_list(&report.source_map_missing_entries)
    );
    println!(
        "source_map_url_mismatches: {}",
        display_list(&report.source_map_url_mismatches)
    );
    println!("hash_mismatches: {}", display_list(&report.hash_mismatches));
    let summary = if report.has_mismatches() {
        "FAIL"
    } else {
        "PASS"
    };
    println!("summary: {summary}");
}

#[derive(Debug)]
struct SourceDownloadReport {
    sources: usize,
    downloaded: usize,
    skipped_existing: usize,
    failures: Vec<String>,
}

impl SourceDownloadReport {
    fn has_failures(&self) -> bool {
        !self.failures.is_empty()
    }
}

fn download_sources(
    docs: &Path,
    source_map: &Path,
    overwrite: bool,
) -> Result<SourceDownloadReport> {
    let source_map_markdown = fs::read_to_string(source_map)?;
    let source_map_entries = parse_source_map_entries(&source_map_markdown);
    fs::create_dir_all(docs)?;

    let mut downloaded = 0;
    let mut skipped_existing = 0;
    let mut failures = Vec::new();

    for (pdf, entry) in &source_map_entries {
        let Some(expected_hash) = entry.hashes.iter().next() else {
            failures.push(format!("missing expected hash for {pdf}"));
            continue;
        };
        let Some(url) = entry.urls.iter().next() else {
            failures.push(format!("missing source URL for {pdf}"));
            continue;
        };
        let target = docs.join(pdf);

        if target.exists() {
            let current_hash = sha256_file(&target)?;
            if current_hash == *expected_hash {
                skipped_existing += 1;
                continue;
            }
            if !overwrite {
                failures.push(format!("existing hash mismatch for {pdf}"));
                continue;
            }
        }

        match download_source_bytes(url) {
            Ok(bytes) => {
                let actual_hash = sha256_bytes(&bytes);
                if actual_hash != *expected_hash {
                    failures.push(format!("downloaded hash mismatch for {pdf}"));
                    continue;
                }
                if let Err(error) = write_verified_download(&target, &bytes) {
                    failures.push(format!("write failed for {pdf}: {error}"));
                    continue;
                }
                downloaded += 1;
            }
            Err(error) => failures.push(format!("download failed for {pdf}: {error}")),
        }
    }

    Ok(SourceDownloadReport {
        sources: source_map_entries.len(),
        downloaded,
        skipped_existing,
        failures,
    })
}

fn download_source_bytes(url: &str) -> Result<Vec<u8>> {
    if let Some(path) = url.strip_prefix("file://") {
        return Ok(fs::read(percent_decode(path))?);
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        let response = reqwest::blocking::get(url)?.error_for_status()?;
        return Ok(response.bytes()?.to_vec());
    }
    bail!("unsupported URL scheme: {url}");
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn write_verified_download(target: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = target
        .file_name()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| "download.pdf".into());
    let temp_path = target.with_file_name(format!("{file_name}.download"));
    {
        let mut temp_file = fs::File::create(&temp_path)?;
        temp_file.write_all(bytes)?;
        temp_file.sync_all()?;
    }
    fs::rename(temp_path, target)?;
    Ok(())
}

fn print_source_download_report(report: &SourceDownloadReport) {
    println!("source download");
    println!("sources: {}", report.sources);
    println!("downloaded: {}", report.downloaded);
    println!("skipped_existing: {}", report.skipped_existing);
    println!("failures: {}", display_list(&report.failures));
    let summary = if report.has_failures() {
        "FAIL"
    } else {
        "PASS"
    };
    println!("summary: {summary}");
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
