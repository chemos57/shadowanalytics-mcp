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
- Tracked PDF corpus inputs under `docs/`.
- Claude Desktop and Codex MCP configuration examples.
- Release packaging script and package smoke-test script.

### Release Checklist

- Run `cargo fmt --all --check`.
- Run `cargo check --workspace`.
- Run `cargo test --workspace`.
- Run `cargo run -p corpus-cli -- build --docs docs --out data/knowledge`.
- Run `cargo run -p corpus-cli -- inspect --out data/knowledge`.
- Run `scripts/package-release.sh`.
- Run `scripts/smoke-package.sh dist/<archive>.tar.gz data/knowledge/chunks/pozsar_chunks.jsonl`.
- Complete [docs/PUBLICATION_CHECKLIST.md](docs/PUBLICATION_CHECKLIST.md) before public publishing.
