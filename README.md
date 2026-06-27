# Pozsar Corpus MCP

Rust tooling for building a local, source-cited knowledge base from a Zoltan Pozsar PDF corpus and exposing it through a read-only MCP server.

The project is intentionally corpus-only. It does not include market data ingestion, trading signals, backtesting, portfolio management, broker adapters, exchange adapters, or execution code.

## What It Does

- Manifests local PDFs in `docs/`
- Extracts page-level text from PDFs
- Chunks extracted pages with source metadata
- Tags chunks with deterministic macro/liquidity themes
- Writes reproducible artifacts under `data/knowledge/`
- Validates generated corpus artifacts with `corpus inspect`
- Runs a read-only MCP server for source-cited corpus search
- Prepares Qdrant-compatible payloads for future vector indexing

## Repository Layout

```text
crates/
  pozsar-kb/      Core corpus library: manifesting, extraction, chunking, themes, inspection
  corpus-cli/     CLI for building and inspecting generated corpus artifacts
  pozsar-mcp/     Read-only MCP server over generated corpus chunks
docs/             Tracked PDF corpus inputs and usage docs
data/knowledge/   Generated corpus artifacts; ignored by git
```

## Local PDF Inputs

PDF files in `docs/` are tracked so the corpus can be rebuilt from the same source documents across machines. [docs/SOURCE_MAP.md](docs/SOURCE_MAP.md) maps each PDF to the public source URL recorded in `Zoltan-Pozsar-Bibliography.html`. Before adding new PDFs to a public repository, confirm their redistribution terms.

See [docs/README.md](docs/README.md) for details.

## Fresh Clone Install And Run

From a fresh clone:

```bash
git clone <repo-url>
cd zp_base
cargo build --workspace
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

To point it at another chunk artifact:

```bash
POZSAR_CHUNKS_JSONL=/path/to/pozsar_chunks.jsonl cargo run -p pozsar-mcp
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

Search results include citations in `file_name:page` format.

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
- PDF source documents under `docs/`
- tests and small fixtures

Ignored:

- `target/`
- `data/knowledge/`
- `dev_docs/`
- local env/log files
