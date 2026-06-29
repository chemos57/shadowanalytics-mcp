# Public Publication Checklist

Use this checklist before publishing the repository or release archive publicly.

## PDF Redistribution

The repository tracks PDF provenance, not PDF binaries. `docs/SOURCE_MAP.md` maps every local PDF to the public source URL recorded in `Zoltan-Pozsar-Bibliography.html`. Before public publishing, review each source and record the redistribution comfort level.

For each PDF:

- Confirm the document was downloaded from a public web source.
- Confirm it appears in `docs/SOURCE_MAP.md`.
- Check whether the source allows redistribution, mirroring, or archival reuse.
- Prefer linking instead of bundling any PDF with unclear redistribution terms.
- Remove any document that is private, paywalled, restricted, or unclear.
- Keep a note of the review decision in release notes or a local audit file.

## Release Artifact Contents

Confirm the release archive includes:

- `bin/pozsar-mcp`
- `README.md`
- `LICENSE`
- `CHANGELOG.md`
- `docs/`
- `eval/fixtures/pozsar_eval.json`

Confirm the release archive excludes:

- `target/`
- `data/knowledge/`
- `docs/*.pdf`
- `.env`
- logs
- private eval files under `eval/local/`

## Provenance Gate

Run the source verifier before packaging:

```bash
cargo run -p corpus-cli -- download-sources --docs docs --source-map docs/SOURCE_MAP.md
cargo run -p corpus-cli -- verify-sources \
  --docs docs \
  --bibliography Zoltan-Pozsar-Bibliography.html \
  --source-map docs/SOURCE_MAP.md
```

The command must report `summary: PASS`, with no missing PDFs, extra links, source-map missing entries, URL mismatches, or hash mismatches.

## Smoke Test

After building the archive, run:

```bash
scripts/smoke-package.sh \
  dist/pozsar-corpus-mcp-0.1.0-<target>.tar.gz \
  data/knowledge/chunks/pozsar_chunks.jsonl
```

The smoke test unpacks the archive, runs `bin/pozsar-mcp --version`, starts the packaged MCP server with `POZSAR_CHUNKS_JSONL`, and calls `get_pozsar_kb_status` over stdio.
