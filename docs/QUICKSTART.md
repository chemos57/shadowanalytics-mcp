# Quickstart

This guide builds a local Pozsar corpus, starts the release MCP binary, and runs a raw stdio MCP demo search.

The repository does not redistribute Pozsar PDFs. The commands below download local copies from the public source URLs recorded in `docs/SOURCE_MAP.md`.

## 1. Download Sources

```bash
cargo run -p corpus-cli -- download-sources --docs docs --source-map docs/SOURCE_MAP.md
```

Verify source URLs and SHA-256 hashes:

```bash
cargo run -p corpus-cli -- verify-sources \
  --docs docs \
  --bibliography Zoltan-Pozsar-Bibliography.html \
  --source-map docs/SOURCE_MAP.md
```

## 2. Build Corpus

```bash
cargo run -p corpus-cli -- build --docs docs --out data/knowledge
cargo run -p corpus-cli -- inspect --out data/knowledge
```

The MCP server reads this artifact by default:

```text
data/knowledge/chunks/pozsar_chunks.jsonl
```

## 3. Build MCP Binary

```bash
cargo build --release -p pozsar-mcp
target/release/pozsar-mcp --version
```

## 4. Run Demo Search

```bash
scripts/demo-mcp-search.sh
```

The script starts `target/release/pozsar-mcp`, initializes the MCP server over stdio, calls `get_pozsar_kb_status`, searches for `collateral dollar liquidity`, reads page context for the top result, and prints compact JSON.

To use a custom chunk artifact or binary:

```bash
POZSAR_CHUNKS_JSONL=/absolute/path/to/pozsar_chunks.jsonl \
POZSAR_MCP_BIN=/absolute/path/to/pozsar-mcp \
  scripts/demo-mcp-search.sh
```

## 5. Configure MCP Client

For Claude Desktop and Codex config examples, see [MCP.md](MCP.md).

Use absolute paths for both the MCP binary and `POZSAR_CHUNKS_JSONL`; relative paths depend on the client process working directory.
