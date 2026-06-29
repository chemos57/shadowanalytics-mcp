# Changelog

All notable changes to this project are documented here.

## 0.1.0-alpha - Unreleased

Initial local corpus MCP release candidate.

### Added

- Rust workspace for PDF corpus build, inspection, eval, and MCP serving.
- Page-level PDF extraction, chunking, deterministic theme tagging, and source citations.
- Read-only stdio MCP server for the local Pozsar corpus.
- MCP tools for status, document/theme listing, source-cited search, explained search, page context, source reading, and research evidence bundles.
- Golden-query eval harness with text or JSON reports, raw-search versus research-question modes, max-rank gates, fail-fast, output files, per-case categories, notes, and category summaries.
- Source-map based PDF provenance under `docs/`; PDF binaries are local ignored files.
- Claude Desktop and Codex MCP configuration examples.
- Release packaging script and package smoke-test script.
- Source-map verifier command for reproducible PDF provenance checks.
- Source downloader command for rebuilding local ignored PDFs from `docs/SOURCE_MAP.md`.
- Public release package excludes PDF binaries while retaining bibliography and source-map provenance.

### Release Checklist

- Run `cargo fmt --all --check`.
- Run `cargo check --workspace`.
- Run `cargo test --workspace`.
- Run `cargo run -p corpus-cli -- download-sources --docs docs --source-map docs/SOURCE_MAP.md`.
- Run `cargo run -p corpus-cli -- build --docs docs --out data/knowledge`.
- Run `cargo run -p corpus-cli -- inspect --out data/knowledge`.
- Run `cargo run -p corpus-cli -- verify-sources --docs docs --bibliography Zoltan-Pozsar-Bibliography.html --source-map docs/SOURCE_MAP.md`.
- Run `scripts/package-release.sh`.
- Run `scripts/smoke-package.sh dist/<archive>.tar.gz data/knowledge/chunks/pozsar_chunks.jsonl`.
- Complete [docs/PUBLICATION_CHECKLIST.md](docs/PUBLICATION_CHECKLIST.md) before public publishing.
