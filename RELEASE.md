# Release Guide

This guide is for maintainers cutting a local/public-source Pozsar Corpus MCP release.

The current release line is `v0.1.0-alpha`. The package archive name uses the workspace crate version from `Cargo.toml`, for example `pozsar-corpus-mcp-0.1.0-x86_64-apple-darwin.tar.gz`.

## 1. Preflight

Start from a clean worktree, or confirm the staged changes are exactly the release changes you intend to publish:

```bash
git status --short
git diff --cached --stat
```

Confirm Rust is available:

```bash
rustc --version
cargo --version
```

Before public publishing, complete the PDF review checklist:

```bash
open docs/PUBLICATION_CHECKLIST.md
```

If publishing from a non-GUI environment, read the file directly:

```bash
sed -n '1,220p' docs/PUBLICATION_CHECKLIST.md
```

## 2. Build And Validate The Corpus

Download local PDFs from the source map:

```bash
cargo run -p corpus-cli -- download-sources --docs docs --source-map docs/SOURCE_MAP.md
```

Build generated corpus artifacts from the downloaded PDFs:

```bash
cargo run -p corpus-cli -- build --docs docs --out data/knowledge
```

Inspect the generated corpus:

```bash
cargo run -p corpus-cli -- inspect --out data/knowledge
```

Verify PDF provenance against the bibliography and source map:

```bash
cargo run -p corpus-cli -- verify-sources \
  --docs docs \
  --bibliography Zoltan-Pozsar-Bibliography.html \
  --source-map docs/SOURCE_MAP.md
```

Expected artifacts:

```text
data/knowledge/manifest.json
data/knowledge/extracted_pages.jsonl
data/knowledge/chunks/pozsar_chunks.jsonl
```

Do not commit `docs/*.pdf` or `data/knowledge/`; both are reproducible release/build output.

## 3. Run Quality Gates

Run formatting, compile, and tests:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

Run golden-query eval for the research-question path:

```bash
cargo run -p corpus-cli -- eval-search \
  --chunks data/knowledge/chunks/pozsar_chunks.jsonl \
  --cases eval/fixtures/pozsar_eval.json \
  --limit 5 \
  --tool research-question \
  --format json \
  --output data/eval/research-question-report.json
```

Run golden-query eval for the raw search path:

```bash
cargo run -p corpus-cli -- eval-search \
  --chunks data/knowledge/chunks/pozsar_chunks.jsonl \
  --cases eval/fixtures/pozsar_eval.json \
  --limit 5 \
  --tool search \
  --format json \
  --output data/eval/search-report.json
```

Eval reports under `data/eval/` are local artifacts and should not be committed.

## 4. Package

Build the release archive:

```bash
scripts/package-release.sh
```

Resolve the archive path:

```bash
TARGET="$(rustc -vV | awk '/host:/ {print $2}')"
ARCHIVE="dist/pozsar-corpus-mcp-0.1.0-${TARGET}.tar.gz"
ls -lh "$ARCHIVE"
```

The archive should contain:

```text
bin/pozsar-mcp
README.md
LICENSE
CHANGELOG.md
Zoltan-Pozsar-Bibliography.html
docs/ excluding PDFs
eval/fixtures/pozsar_eval.json
```

## 5. Smoke-Test The Package

Run the package smoke test against the generated chunk artifact:

```bash
scripts/smoke-package.sh \
  "$ARCHIVE" \
  data/knowledge/chunks/pozsar_chunks.jsonl
```

The smoke test:

- unpacks the archive into a temporary directory
- runs `bin/pozsar-mcp --version`
- starts the packaged MCP server with `POZSAR_CHUNKS_JSONL`
- calls `get_pozsar_kb_status` over stdio
- confirms the server reports a nonzero `chunk_count`

## 6. Commit And Tag

Review the staged release diff:

```bash
git status --short
git diff --cached --stat
```

Commit:

```bash
git commit -m "Prepare v0.1.0 alpha release"
```

Tag:

```bash
git tag v0.1.0-alpha
```

Verify:

```bash
git show --stat --oneline --decorate HEAD
git tag --list "v0.1.0-alpha"
```

## 7. Publish

Push the release commit and tag:

```bash
git push origin HEAD
git push origin v0.1.0-alpha
```

If publishing via GitHub Releases:

```bash
gh release create v0.1.0-alpha \
  "$ARCHIVE" \
  --title "v0.1.0-alpha" \
  --notes-file CHANGELOG.md \
  --prerelease
```

If `gh` is unavailable, create the release in the GitHub UI and upload the archive from `dist/`.

## 8. Post-Release Check

From a fresh clone, verify the documented path still works:

```bash
git clone <repo-url> /tmp/pozsar-corpus-mcp-release-check
cd /tmp/pozsar-corpus-mcp-release-check
cargo run -p corpus-cli -- download-sources --docs docs --source-map docs/SOURCE_MAP.md
cargo run -p corpus-cli -- build --docs docs --out data/knowledge
cargo run -p corpus-cli -- inspect --out data/knowledge
cargo run -p corpus-cli -- verify-sources --docs docs --bibliography Zoltan-Pozsar-Bibliography.html --source-map docs/SOURCE_MAP.md
cargo build --release -p pozsar-mcp
target/release/pozsar-mcp --version
```

Then configure an MCP client using the examples in `docs/MCP.md`.
