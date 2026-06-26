# Public Publication Checklist

Use this checklist before publishing the repository or release archive publicly.

## PDF Redistribution

The repository tracks PDFs under `docs/` so the corpus is reproducible. Before public publishing, review each PDF and record the source and redistribution comfort level.

For each PDF:

- Confirm the document was downloaded from a public web source.
- Record the source URL or source page.
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
- `.env`
- logs
- private eval files under `eval/local/`

## Smoke Test

After building the archive, run:

```bash
scripts/smoke-package.sh \
  dist/pozsar-corpus-mcp-0.1.0-<target>.tar.gz \
  data/knowledge/chunks/pozsar_chunks.jsonl
```

The smoke test unpacks the archive, runs `bin/pozsar-mcp --version`, starts the packaged MCP server with `POZSAR_CHUNKS_JSONL`, and calls `get_pozsar_kb_status` over stdio.
