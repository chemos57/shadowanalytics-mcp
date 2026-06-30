use advisor_core::{build_advisor_snapshot, build_advisor_snapshot_with_health, AdvisorSnapshot};
use advisor_policy::{build_advisor_policy, AdvisorPolicy};
use anyhow::{bail, Result};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use market_context::{build_market_context_from_csv, MarketContext, MarketDataHealth};
use market_data_adapters::{
    build_market_context_from_yahoo, FetchMarketContextRequest, FetchMarketContextResult,
    MarketDataHealthStatus,
};
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
use pozsar_mcp::signals::{extract_liquidity_signals, LiquiditySignalParams};
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
    EvalAdvisor {
        #[arg(long)]
        cases: PathBuf,
        #[arg(long, value_enum, default_value_t = EvalOutputFormat::Text)]
        format: EvalOutputFormat,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    EvalAdvisorPolicy {
        #[arg(long)]
        cases: PathBuf,
        #[arg(long, value_enum, default_value_t = EvalOutputFormat::Text)]
        format: EvalOutputFormat,
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
    FetchMarketContext {
        #[arg(long, value_enum)]
        provider: LiveMarketDataProvider,
        #[arg(long)]
        assets: String,
        #[arg(long, default_value_t = 60)]
        lookback: u32,
        #[arg(long, default_value_t = 7)]
        max_stale_days: i64,
        #[arg(long)]
        out: PathBuf,
    },
    AdvisorSnapshot {
        #[arg(long)]
        chunks: PathBuf,
        #[arg(long)]
        market_context: PathBuf,
        #[arg(long)]
        market_health: Option<PathBuf>,
        #[arg(long)]
        question: String,
        #[arg(long)]
        assets: String,
        #[arg(long)]
        themes: Option<String>,
        #[arg(long, default_value_t = 8)]
        limit: u64,
        #[arg(long)]
        out: PathBuf,
    },
    AdvisorPolicy {
        #[arg(long)]
        snapshot: PathBuf,
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LiveMarketDataProvider {
    Yahoo,
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
        Command::EvalAdvisor {
            cases,
            format,
            output,
        } => {
            let cases = read_advisor_eval_cases(&cases)?;
            let report = eval_advisor(&cases)?;
            emit_advisor_eval_report(&report, format, output.as_ref())?;
            if report.failed > 0 {
                bail!(
                    "advisor eval failed: {}/{} passed",
                    report.passed,
                    report.total
                );
            }
        }
        Command::EvalAdvisorPolicy {
            cases,
            format,
            output,
        } => {
            let cases = read_advisor_policy_eval_cases(&cases)?;
            let report = eval_advisor_policy(&cases)?;
            emit_advisor_policy_eval_report(&report, format, output.as_ref())?;
            if report.failed > 0 {
                bail!(
                    "advisor policy eval failed: {}/{} passed",
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
        Command::FetchMarketContext {
            provider,
            assets,
            lookback,
            max_stale_days,
            out,
        } => {
            let assets = parse_csv_arg(&assets);
            let request = FetchMarketContextRequest {
                assets,
                lookback_days: lookback,
            };
            let result = match provider {
                LiveMarketDataProvider::Yahoo => {
                    build_market_context_from_yahoo(&request, max_stale_days.max(0))?
                }
            };
            let today = Utc::now().date_naive();
            persist_valid_fetched_market_context(
                &result,
                &out,
                live_market_provider_name(provider),
                &today.to_string(),
            )?;
        }
        Command::AdvisorSnapshot {
            chunks,
            market_context,
            market_health,
            question,
            assets,
            themes,
            limit,
            out,
        } => {
            let chunks = read_chunks_jsonl(&chunks)?;
            let market_context_json = fs::read_to_string(&market_context)?;
            let market_context: MarketContext = serde_json::from_str(&market_context_json)?;
            let market_context_health = market_health
                .as_deref()
                .map(load_market_data_health_json)
                .transpose()?;
            let liquidity_signals = extract_liquidity_signals(
                &chunks,
                LiquiditySignalParams {
                    question: question.clone(),
                    assets: parse_csv_arg(&assets),
                    themes: themes.as_deref().map(parse_csv_arg),
                    limit: Some(limit),
                },
            );
            let snapshot = build_advisor_snapshot_with_health(
                question,
                liquidity_signals,
                market_context,
                market_context_health,
            )?;
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(
                &out,
                format!("{}\n", serde_json::to_string_pretty(&snapshot)?),
            )?;
            println!("wrote advisor snapshot: {}", out.display());
            println!("macro_liquidity: {}", snapshot.regime.macro_liquidity);
            println!("market_risk: {}", snapshot.regime.market_risk);
            println!("combined: {}", snapshot.regime.combined);
        }
        Command::AdvisorPolicy { snapshot, out } => {
            let snapshot_json = fs::read_to_string(&snapshot)?;
            let snapshot: AdvisorSnapshot = serde_json::from_str(&snapshot_json)?;
            let policy = build_advisor_policy(snapshot);
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(
                &out,
                format!("{}\n", serde_json::to_string_pretty(&policy)?),
            )?;
            println!("wrote advisor policy: {}", out.display());
            println!("as_of: {}", policy.as_of);
            println!("regime: {}", policy.regime);
            println!("assessments: {}", policy.asset_assessments.len());
        }
    }
    Ok(())
}

fn live_market_provider_name(provider: LiveMarketDataProvider) -> &'static str {
    match provider {
        LiveMarketDataProvider::Yahoo => "yahoo",
    }
}

fn persist_valid_fetched_market_context(
    result: &FetchMarketContextResult,
    out: &Path,
    provider_name: &str,
    today: &str,
) -> Result<()> {
    println!("provider: {provider_name}");
    println!("as_of: {}", result.context.as_of);
    println!("assets: {}", result.context.assets.len());
    println!("risk_regime: {}", result.context.cross_asset.risk_regime);
    println!("health: {}", serde_json::to_string_pretty(&result.health)?);
    if result.health.status == MarketDataHealthStatus::Invalid {
        bail!("market context health invalid");
    }
    if result.context.as_of.as_str() > today {
        bail!("market context as_of is in the future");
    }
    let health_out = market_health_sidecar_path(out);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        out,
        format!("{}\n", serde_json::to_string_pretty(&result.context)?),
    )?;
    fs::write(
        &health_out,
        format!("{}\n", serde_json::to_string_pretty(&result.health)?),
    )?;
    println!("wrote market context: {}", out.display());
    println!("wrote market context health: {}", health_out.display());
    Ok(())
}

fn load_market_data_health_json(path: &Path) -> Result<MarketDataHealth> {
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

fn market_health_sidecar_path(context_path: &Path) -> PathBuf {
    let file_name = context_path
        .file_stem()
        .map(|stem| format!("{}.health.json", stem.to_string_lossy()))
        .unwrap_or_else(|| "context.health.json".to_string());
    context_path.with_file_name(file_name)
}

fn parse_csv_arg(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
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

#[derive(Debug, Deserialize)]
struct AdvisorEvalCase {
    name: String,
    question: String,
    chunks: PathBuf,
    market_context: PathBuf,
    assets: Vec<String>,
    themes: Option<Vec<String>>,
    expected: AdvisorExpected,
}

#[derive(Debug, Deserialize)]
struct AdvisorPolicyEvalCase {
    name: String,
    snapshot: PathBuf,
    expected: AdvisorPolicyExpected,
}

#[derive(Debug, Deserialize)]
struct AdvisorExpected {
    macro_liquidity: String,
    market_risk: String,
    combined: String,
    confirmations: Vec<AdvisorExpectedConfirmation>,
    forbidden_terms: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AdvisorExpectedConfirmation {
    asset: String,
    macro_bias: String,
    market_trend: String,
    alignment: String,
}

#[derive(Debug, Deserialize)]
struct AdvisorPolicyExpected {
    regime: String,
    assessments: Vec<AdvisorPolicyExpectedAssessment>,
    forbidden_terms: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AdvisorPolicyExpectedAssessment {
    asset: String,
    stance: String,
    confidence: String,
    alignment: String,
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

#[derive(Serialize)]
struct AdvisorEvalReport {
    total: usize,
    passed: usize,
    failed: usize,
    cases: Vec<AdvisorEvalCaseReport>,
}

#[derive(Serialize)]
struct AdvisorEvalCaseReport {
    name: String,
    passed: bool,
    failures: Vec<String>,
    actual_regime: AdvisorEvalRegime,
}

#[derive(Serialize)]
struct AdvisorEvalRegime {
    macro_liquidity: String,
    market_risk: String,
    combined: String,
}

#[derive(Serialize)]
struct AdvisorPolicyEvalReport {
    total: usize,
    passed: usize,
    failed: usize,
    cases: Vec<AdvisorPolicyEvalCaseReport>,
}

#[derive(Serialize)]
struct AdvisorPolicyEvalCaseReport {
    name: String,
    passed: bool,
    failures: Vec<String>,
    actual_regime: String,
    actual_assessments: Vec<AdvisorPolicyEvalAssessment>,
}

#[derive(Serialize)]
struct AdvisorPolicyEvalAssessment {
    asset: String,
    stance: String,
    confidence: String,
    alignment: String,
}

fn read_eval_cases(path: &PathBuf) -> Result<Vec<EvalCase>> {
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

fn read_advisor_eval_cases(path: &PathBuf) -> Result<Vec<AdvisorEvalCase>> {
    let json = fs::read_to_string(path)?;
    let mut cases = serde_json::from_str::<Vec<AdvisorEvalCase>>(&json)?;
    let case_file = fs::canonicalize(path)?;
    let case_dir = case_file.parent().unwrap_or_else(|| Path::new("."));
    let workspace_root = find_workspace_root(case_dir);

    for case in &mut cases {
        case.chunks = resolve_advisor_case_path(&case.chunks, case_dir, workspace_root.as_deref());
        case.market_context =
            resolve_advisor_case_path(&case.market_context, case_dir, workspace_root.as_deref());
    }

    Ok(cases)
}

fn read_advisor_policy_eval_cases(path: &PathBuf) -> Result<Vec<AdvisorPolicyEvalCase>> {
    let json = fs::read_to_string(path)?;
    let mut cases = serde_json::from_str::<Vec<AdvisorPolicyEvalCase>>(&json)?;
    let case_file = fs::canonicalize(path)?;
    let case_dir = case_file.parent().unwrap_or_else(|| Path::new("."));
    let workspace_root = find_workspace_root(case_dir);

    for case in &mut cases {
        case.snapshot =
            resolve_advisor_case_path(&case.snapshot, case_dir, workspace_root.as_deref());
    }

    Ok(cases)
}

fn resolve_advisor_case_path(
    path: &Path,
    case_dir: &Path,
    workspace_root: Option<&Path>,
) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    let case_relative = case_dir.join(path);
    if case_relative.exists() {
        return case_relative;
    }

    if let Some(workspace_root) = workspace_root {
        let workspace_relative = workspace_root.join(path);
        if workspace_relative.exists() {
            return workspace_relative;
        }
    }

    case_relative
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|candidate| {
            candidate.join("Cargo.toml").is_file() && candidate.join("crates").is_dir()
        })
        .map(Path::to_path_buf)
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

fn eval_advisor(cases: &[AdvisorEvalCase]) -> Result<AdvisorEvalReport> {
    let mut reports = Vec::new();

    for case in cases {
        reports.push(eval_advisor_case(case)?);
    }

    let passed = reports.iter().filter(|case| case.passed).count();
    let total = reports.len();

    Ok(AdvisorEvalReport {
        total,
        passed,
        failed: total.saturating_sub(passed),
        cases: reports,
    })
}

fn eval_advisor_case(case: &AdvisorEvalCase) -> Result<AdvisorEvalCaseReport> {
    let chunks = read_chunks_jsonl(&case.chunks)?;
    let market_context_json = fs::read_to_string(&case.market_context)?;
    let market_context: MarketContext = serde_json::from_str(&market_context_json)?;
    let liquidity_signals = extract_liquidity_signals(
        &chunks,
        LiquiditySignalParams {
            question: case.question.clone(),
            assets: case.assets.clone(),
            themes: case.themes.clone(),
            limit: Some(8),
        },
    );
    let snapshot = build_advisor_snapshot(case.question.clone(), liquidity_signals, market_context);
    let actual_regime = AdvisorEvalRegime {
        macro_liquidity: snapshot.regime.macro_liquidity.clone(),
        market_risk: snapshot.regime.market_risk.clone(),
        combined: snapshot.regime.combined.clone(),
    };
    let mut failures = Vec::new();

    if actual_regime.macro_liquidity != case.expected.macro_liquidity {
        failures.push(format!(
            "macro_liquidity expected {}, got {}",
            case.expected.macro_liquidity, actual_regime.macro_liquidity
        ));
    }
    if actual_regime.market_risk != case.expected.market_risk {
        failures.push(format!(
            "market_risk expected {}, got {}",
            case.expected.market_risk, actual_regime.market_risk
        ));
    }
    if actual_regime.combined != case.expected.combined {
        failures.push(format!(
            "combined expected {}, got {}",
            case.expected.combined, actual_regime.combined
        ));
    }

    for expected in &case.expected.confirmations {
        if !snapshot.confirmations.iter().any(|actual| {
            actual.asset == expected.asset
                && actual.macro_bias == expected.macro_bias
                && actual.market_trend == expected.market_trend
                && actual.alignment == expected.alignment
        }) {
            failures.push(format!(
                "missing confirmation {} {} {} {}",
                expected.asset, expected.macro_bias, expected.market_trend, expected.alignment
            ));
        }
    }

    let rendered_snapshot = render_forbidden_term_scan_fields(&snapshot)?;
    for term in &case.expected.forbidden_terms {
        if contains_forbidden_term(&rendered_snapshot, term) {
            failures.push(format!("forbidden term present: {term}"));
        }
    }

    Ok(AdvisorEvalCaseReport {
        name: case.name.clone(),
        passed: failures.is_empty(),
        failures,
        actual_regime,
    })
}

fn eval_advisor_policy(cases: &[AdvisorPolicyEvalCase]) -> Result<AdvisorPolicyEvalReport> {
    let mut reports = Vec::new();

    for case in cases {
        reports.push(eval_advisor_policy_case(case)?);
    }

    let passed = reports.iter().filter(|case| case.passed).count();
    let total = reports.len();

    Ok(AdvisorPolicyEvalReport {
        total,
        passed,
        failed: total.saturating_sub(passed),
        cases: reports,
    })
}

fn eval_advisor_policy_case(case: &AdvisorPolicyEvalCase) -> Result<AdvisorPolicyEvalCaseReport> {
    let snapshot_json = fs::read_to_string(&case.snapshot)?;
    let snapshot: AdvisorSnapshot = serde_json::from_str(&snapshot_json)?;
    let policy = build_advisor_policy(snapshot);
    let actual_assessments = policy
        .asset_assessments
        .iter()
        .map(|assessment| AdvisorPolicyEvalAssessment {
            asset: assessment.asset.clone(),
            stance: assessment.stance.clone(),
            confidence: assessment.confidence.clone(),
            alignment: assessment.alignment.clone(),
        })
        .collect::<Vec<_>>();
    let mut failures = Vec::new();

    if policy.regime != case.expected.regime {
        failures.push(format!(
            "regime expected {}, got {}",
            case.expected.regime, policy.regime
        ));
    }

    for expected in &case.expected.assessments {
        if !policy.asset_assessments.iter().any(|actual| {
            actual.asset == expected.asset
                && actual.stance == expected.stance
                && actual.confidence == expected.confidence
                && actual.alignment == expected.alignment
        }) {
            failures.push(format!(
                "missing assessment {} {} {} {}",
                expected.asset, expected.stance, expected.confidence, expected.alignment
            ));
        }
    }

    let rendered_policy = render_policy_forbidden_term_scan_fields(&policy)?;
    for term in &case.expected.forbidden_terms {
        if contains_forbidden_term(&rendered_policy, term) {
            failures.push(format!("forbidden term present: {term}"));
        }
    }

    Ok(AdvisorPolicyEvalCaseReport {
        name: case.name.clone(),
        passed: failures.is_empty(),
        failures,
        actual_regime: policy.regime,
        actual_assessments,
    })
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

fn render_forbidden_term_scan_fields(snapshot: &AdvisorSnapshot) -> Result<String> {
    let liquidity_conditions = snapshot
        .liquidity_signals
        .liquidity_conditions
        .iter()
        .map(|condition| {
            serde_json::json!({
                "label": condition.label,
                "direction": condition.direction,
                "confidence": condition.confidence,
            })
        })
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "macro_themes": &snapshot.liquidity_signals.macro_themes,
        "liquidity_conditions": liquidity_conditions,
        "cross_asset_implications": &snapshot.liquidity_signals.cross_asset_implications,
        "confirmations": &snapshot.confirmations,
        "regime": &snapshot.regime,
        "unknowns": &snapshot.unknowns,
    });

    Ok(serde_json::to_string(&value)?)
}

fn render_policy_forbidden_term_scan_fields(policy: &AdvisorPolicy) -> Result<String> {
    let value = serde_json::json!({
        "regime": &policy.regime,
        "asset_assessments": &policy.asset_assessments,
        "unknowns": &policy.unknowns,
    });

    Ok(serde_json::to_string(&value)?)
}

fn contains_forbidden_term(text: &str, term: &str) -> bool {
    let term = term.to_ascii_lowercase();
    if term.is_empty() {
        return false;
    }
    text.to_ascii_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| token == term)
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

fn emit_advisor_eval_report(
    report: &AdvisorEvalReport,
    format: EvalOutputFormat,
    output: Option<&PathBuf>,
) -> Result<()> {
    let rendered = render_advisor_eval_report(report, format)?;
    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, rendered)?;
        println!("wrote advisor eval report: {}", output.display());
        println!("summary: {}/{} passed", report.passed, report.total);
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn emit_advisor_policy_eval_report(
    report: &AdvisorPolicyEvalReport,
    format: EvalOutputFormat,
    output: Option<&PathBuf>,
) -> Result<()> {
    let rendered = render_advisor_policy_eval_report(report, format)?;
    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, rendered)?;
        println!("wrote advisor policy eval report: {}", output.display());
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

fn render_advisor_eval_report(
    report: &AdvisorEvalReport,
    format: EvalOutputFormat,
) -> Result<String> {
    if matches!(format, EvalOutputFormat::Json) {
        return Ok(format!("{}\n", serde_json::to_string_pretty(report)?));
    }

    let mut output = String::new();
    output.push_str("advisor eval\n");
    for case in &report.cases {
        let status = if case.passed { "PASS" } else { "FAIL" };
        output.push_str(&format!("{status} {}\n", case.name));
        output.push_str(&format!(
            "  regime: {}/{}/{}\n",
            case.actual_regime.macro_liquidity,
            case.actual_regime.market_risk,
            case.actual_regime.combined
        ));
        if !case.failures.is_empty() {
            output.push_str(&format!("  failures: {}\n", display_list(&case.failures)));
        }
    }
    output.push_str(&format!(
        "summary: {}/{} passed\n",
        report.passed, report.total
    ));
    Ok(output)
}

fn render_advisor_policy_eval_report(
    report: &AdvisorPolicyEvalReport,
    format: EvalOutputFormat,
) -> Result<String> {
    if matches!(format, EvalOutputFormat::Json) {
        return Ok(format!("{}\n", serde_json::to_string_pretty(report)?));
    }

    let mut output = String::new();
    output.push_str("advisor policy eval\n");
    for case in &report.cases {
        let status = if case.passed { "PASS" } else { "FAIL" };
        output.push_str(&format!("{status} {}\n", case.name));
        output.push_str(&format!("  regime: {}\n", case.actual_regime));
        let assessments = case
            .actual_assessments
            .iter()
            .map(|assessment| {
                format!(
                    "{} {}/{}/{}",
                    assessment.asset,
                    assessment.stance,
                    assessment.confidence,
                    assessment.alignment
                )
            })
            .collect::<Vec<_>>();
        output.push_str(&format!("  assessments: {}\n", display_list(&assessments)));
        if !case.failures.is_empty() {
            output.push_str(&format!("  failures: {}\n", display_list(&case.failures)));
        }
    }
    output.push_str(&format!(
        "summary: {}/{} passed\n",
        report.passed, report.total
    ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use market_context::{AssetContext, CrossAssetContext};
    use market_data_adapters::{FetchMarketContextResult, MarketDataHealth};

    #[test]
    fn invalid_fetched_market_context_does_not_overwrite_existing_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "pozsar_invalid_fetch_persist_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let out = temp_dir.join("context.json");
        std::fs::write(&out, "previous valid context").unwrap();
        let result = FetchMarketContextResult {
            context: MarketContext {
                as_of: "2026-06-21".to_string(),
                assets: vec![AssetContext {
                    symbol: "BTC".to_string(),
                    last_close: 100.0,
                    return_1d: Some(0.01),
                    return_5d: Some(0.02),
                    return_20d: Some(0.05),
                    trend_20d: "up".to_string(),
                    volatility_20d: Some(0.2),
                    drawdown_20d: Some(0.0),
                }],
                cross_asset: CrossAssetContext {
                    risk_regime: "risk_on".to_string(),
                    dxy_trend: "unknown".to_string(),
                    rates_proxy: "unknown".to_string(),
                },
            },
            health: MarketDataHealth {
                status: MarketDataHealthStatus::Invalid,
                as_of: "2026-06-21".to_string(),
                missing_assets: vec!["DXY".to_string()],
                stale_assets: vec![],
                warnings: vec![],
                blocking_issues: vec!["missing assets: DXY".to_string()],
            },
        };

        let error =
            persist_valid_fetched_market_context(&result, &out, "yahoo", "2026-06-30").unwrap_err();

        assert!(error.to_string().contains("market context health invalid"));
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "previous valid context"
        );
    }

    #[test]
    fn valid_fetched_market_context_writes_context_and_health_sidecar() {
        let temp_dir =
            std::env::temp_dir().join(format!("pozsar_valid_fetch_persist_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let out = temp_dir.join("context.json");
        let result = FetchMarketContextResult {
            context: MarketContext {
                as_of: "2026-06-30".to_string(),
                assets: vec![AssetContext {
                    symbol: "BTC".to_string(),
                    last_close: 100.0,
                    return_1d: Some(0.01),
                    return_5d: Some(0.02),
                    return_20d: Some(0.05),
                    trend_20d: "up".to_string(),
                    volatility_20d: Some(0.2),
                    drawdown_20d: Some(0.0),
                }],
                cross_asset: CrossAssetContext {
                    risk_regime: "risk_on".to_string(),
                    dxy_trend: "up".to_string(),
                    rates_proxy: "TLT_up".to_string(),
                },
            },
            health: MarketDataHealth {
                status: MarketDataHealthStatus::Ok,
                as_of: "2026-06-30".to_string(),
                missing_assets: Vec::new(),
                stale_assets: Vec::new(),
                warnings: Vec::new(),
                blocking_issues: Vec::new(),
            },
        };

        persist_valid_fetched_market_context(&result, &out, "yahoo", "2026-06-30").unwrap();

        let context: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
        let health: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp_dir.join("context.health.json")).unwrap())
                .unwrap();
        assert_eq!(context["as_of"], "2026-06-30");
        assert_eq!(health["status"], "ok");
    }
}
