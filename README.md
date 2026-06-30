# Pozsar Corpus MCP

Rust tooling for building a local, source-cited knowledge base from a Zoltan Pozsar PDF corpus, exposing it through a read-only MCP server, generating offline cross-asset market context snapshots, and composing advisor-ready context.

The project does not include live market data ingestion, trading signals, backtesting, portfolio management, broker adapters, exchange adapters, or execution code. Market context is offline and file-based only.

## Why Pozsar

Zoltan Pozsar's work is important because it connects macroeconomics to the balance-sheet plumbing that often drives markets: collateral, repo, eurodollars, FX swaps, reserves, shadow banking, safe assets, and commodity-linked money. His writing is useful for studying how liquidity moves through the financial system and why stress can appear outside the places covered by standard macro indicators.

This project turns that corpus into a local, source-cited research layer so agents and analysts can retrieve the original passages behind a macro or liquidity question instead of relying on unsourced summaries.

## What It Does

- Manifests local PDFs in `docs/`
- Extracts page-level text from PDFs
- Chunks extracted pages with source metadata
- Tags chunks with deterministic macro/liquidity themes
- Writes reproducible artifacts under `data/knowledge/`
- Validates generated corpus artifacts with `corpus inspect`
- Runs a read-only MCP server for source-cited corpus search
- Prepares Qdrant-compatible payloads for future vector indexing
- Builds deterministic market context JSON from local price CSV files
- Builds deterministic advisor snapshots that combine corpus liquidity signals with market context

## Repository Layout

```text
crates/
  advisor-core/   Offline advisor snapshot composition logic
  pozsar-kb/      Core corpus library: manifesting, extraction, chunking, themes, inspection
  corpus-cli/     CLI for building and inspecting generated corpus artifacts
  market-context/ Offline cross-asset market context library
  pozsar-mcp/     Read-only MCP server over generated corpus chunks
docs/             Source map, usage docs, and ignored downloaded PDFs
data/knowledge/   Generated corpus artifacts; ignored by git
data/market/      Local/sample market CSV inputs and ignored generated JSON context
data/advisor/     Ignored generated advisor snapshots
```

## Local PDF Inputs

This repository does not redistribute Pozsar PDFs. PDF files in `docs/` are ignored by git and are excluded from release archives. Users rebuild the local corpus by downloading the documents from public source links recorded in [docs/SOURCE_MAP.md](docs/SOURCE_MAP.md) and `Zoltan-Pozsar-Bibliography.html`.

`corpus download-sources` can rebuild the local PDF set, and `corpus verify-sources` checks local PDF hashes and source URLs before corpus generation.

See [docs/README.md](docs/README.md) for details.

## Fresh Clone Install And Run

For a shorter guided path with a raw MCP demo search, see [docs/QUICKSTART.md](docs/QUICKSTART.md).

From a fresh clone:

```bash
git clone <repo-url>
cd zp_base
cargo build --workspace
cargo run -p corpus-cli -- download-sources --docs docs --source-map docs/SOURCE_MAP.md
cargo run -p corpus-cli -- verify-sources --docs docs --bibliography Zoltan-Pozsar-Bibliography.html --source-map docs/SOURCE_MAP.md
cargo run -p corpus-cli -- build --docs docs --out data/knowledge
cargo run -p corpus-cli -- inspect --out data/knowledge
cargo build --release -p pozsar-mcp
target/release/pozsar-mcp --version
```

Run the MCP server with the generated chunk artifact:

```bash
POZSAR_CHUNKS_JSONL="$PWD/data/knowledge/chunks/pozsar_chunks.jsonl" \
  target/release/pozsar-mcp
```

The MCP server uses stdio, so this command waits for an MCP client instead of printing an interactive prompt.

## Build The Corpus

Download the local PDF corpus first:

```bash
cargo run -p corpus-cli -- download-sources --docs docs --source-map docs/SOURCE_MAP.md
```

```bash
cargo run -p corpus-cli -- build --docs docs --out data/knowledge
```

This writes:

```text
data/knowledge/manifest.json
data/knowledge/extracted_pages.jsonl
data/knowledge/chunks/pozsar_chunks.jsonl
```

## Inspect The Corpus

```bash
cargo run -p corpus-cli -- inspect --out data/knowledge
```

The inspection command reports document/page/chunk counts, empty pages, pages without chunks, validation issues, and theme distribution. It exits nonzero if validation issues are found.

## Build Market Context

Generate an offline market context snapshot from a local CSV file:

```bash
cargo run -p corpus-cli -- market-context \
  --prices data/market/sample_prices.csv \
  --out data/market/context.json
```

Input CSV columns:

```csv
date,symbol,close
2026-06-24,BTC,101000
2026-06-25,BTC,103000
```

The output includes per-asset returns, 20-observation trend, annualized volatility, drawdown, and a deterministic cross-asset regime summary. This is local market context only; it does not produce trade recommendations.

## Build Advisor Snapshot

Combine corpus liquidity evidence with a local market context snapshot:

```bash
cargo run -p corpus-cli -- advisor-snapshot \
  --chunks data/knowledge/chunks/pozsar_chunks.jsonl \
  --market-context data/market/context.json \
  --question "What does collateral scarcity imply for cross-asset liquidity?" \
  --assets BTC,ETH,SPY,QQQ,GLD,TLT,DXY \
  --themes collateral,dollar_liquidity,repo \
  --out data/advisor/snapshot.json
```

The output includes source-cited liquidity signals, market context, per-asset macro/market alignment, and a combined regime label such as `macro_tightening_market_risk_on`. It is deterministic context for a future advisor layer, not trading advice.

## Verify PDF Sources

```bash
cargo run -p corpus-cli -- verify-sources \
  --docs docs \
  --bibliography Zoltan-Pozsar-Bibliography.html \
  --source-map docs/SOURCE_MAP.md
```

The verifier compares local ignored `docs/*.pdf` files with direct PDF links in the bibliography, checks each source-map URL and SHA-256 hash, and exits nonzero on mismatch.

## Evaluate Retrieval

Run fixture-backed golden-query checks against a chunk artifact:

```bash
cargo run -p corpus-cli -- eval-search \
  --chunks data/knowledge/chunks/pozsar_chunks.jsonl \
  --cases eval/fixtures/pozsar_eval.json \
  --limit 5 \
  --tool research-question \
  --format text \
  --fail-fast \
  --output data/eval/research-question-report.json
```

The eval command reports pass/fail per case, returned citations, expected citation ranks, missing citations, category summaries, and a summary count. Use `--tool search` to test raw local search or `--tool research-question` to test the research bundle path. Use `--format json` for CI or machine-readable regression artifacts. Use `--output <path>` to write the report to disk while printing only the output path and summary to stdout. Use `--fail-fast` to stop after the first failing case. It exits nonzero if any expected citation is missing or ranked too low. Keep public-safe fixtures under `eval/fixtures/`; use ignored local files under `eval/local/` or `eval/*.json` for private corpus cases.

Eval cases can include optional `max_rank` to require each expected citation to appear by that rank:

```json
{
  "name": "collateral dollar liquidity",
  "category": "dollar_liquidity",
  "notes": "Core liquidity plumbing query; should surface Safe Asset Glut early.",
  "query": "collateral dollar liquidity",
  "themes": ["collateral"],
  "expected_citations": ["The_Safe_Asset_Glut.pdf:7"],
  "max_rank": 3
}
```

## Run The MCP Server

By default, the MCP server reads `data/knowledge/chunks/pozsar_chunks.jsonl`:

```bash
cargo run -p pozsar-mcp
```

To point it at another chunk artifact or market context snapshot:

```bash
POZSAR_CHUNKS_JSONL=/path/to/pozsar_chunks.jsonl \
POZSAR_MARKET_CONTEXT_JSON=/path/to/context.json \
  cargo run -p pozsar-mcp
```

Available MCP tools:

- `get_pozsar_kb_status`
- `list_pozsar_docs`
- `list_pozsar_themes`
- `search_pozsar_kb`
- `explain_pozsar_search`
- `read_pozsar_source`
- `read_pozsar_page_context`
- `answer_pozsar_research_question`
- `extract_pozsar_liquidity_signals`
- `get_pozsar_advisor_snapshot`

Search results include citations in `file_name:page` format.

Run a raw MCP advisor snapshot demo after building the release binary and local artifacts:

```bash
scripts/demo-mcp-advisor.sh
```

For Claude Desktop and Codex configuration examples, tool parameters, and response shapes, see [docs/MCP.md](docs/MCP.md).

Check the MCP binary version without starting the stdio server:

```bash
cargo run -p pozsar-mcp -- --version
```

Package a release tarball under `dist/`:

```bash
scripts/package-release.sh
```

Smoke-test the package against a built corpus:

```bash
scripts/smoke-package.sh \
  dist/pozsar-corpus-mcp-0.1.0-<target>.tar.gz \
  data/knowledge/chunks/pozsar_chunks.jsonl
```

For example MCP prompts and a sample eval report, see [docs/EXAMPLES.md](docs/EXAMPLES.md).

For the full maintainer release flow, see [RELEASE.md](RELEASE.md).

Before public publishing, complete [docs/PUBLICATION_CHECKLIST.md](docs/PUBLICATION_CHECKLIST.md), especially the PDF redistribution review.

## License

This project's source code and documentation are distributed under the MIT License; see [LICENSE](LICENSE).

The Pozsar PDFs are not distributed by this repository or release packages. They are downloaded by users from public source URLs listed in [docs/SOURCE_MAP.md](docs/SOURCE_MAP.md), and those documents remain subject to their original publishers' terms.

## Development Checks

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

## Git Policy

Tracked:

- Rust workspace source
- `Cargo.lock`
- `LICENSE` and `CHANGELOG.md`
- README files
- source-map and bibliography files for rebuilding PDFs
- tests and small fixtures

Ignored:

- `target/`
- `data/knowledge/`
- `dev_docs/`
- `docs/*.pdf`
- local env/log files
