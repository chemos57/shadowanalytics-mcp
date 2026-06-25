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
docs/             Local PDF inputs; PDFs are ignored by git
data/knowledge/   Generated corpus artifacts; ignored by git
```

## Local PDF Inputs

PDF files are not tracked because this repository may become public and redistribution rights can vary by document. Place local source PDFs directly under `docs/`.

See [docs/README.md](docs/README.md) for details.

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

- `list_pozsar_docs`
- `list_pozsar_themes`
- `search_pozsar_kb`
- `read_pozsar_source`

Search results include citations in `file_name:page` format.

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
- README files
- tests and small fixtures

Ignored:

- `target/`
- `data/knowledge/`
- `docs/*.pdf`
- `dev_docs/`
- local env/log files
