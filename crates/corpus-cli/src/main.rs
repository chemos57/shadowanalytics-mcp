use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use pozsar_kb::artifacts::{write_chunks_jsonl, write_manifest, write_pages_jsonl};
use pozsar_kb::chunk::chunk_pages;
use pozsar_kb::extract::extract_manifest_pages;
use pozsar_kb::inspect::{inspect_artifacts, CorpusInspection};
use pozsar_kb::manifest::build_manifest;
use pozsar_kb::themes::tag_chunks;
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
