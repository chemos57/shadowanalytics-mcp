# Example Prompts And Reports

These examples assume the MCP server is configured as `pozsar-corpus` and the corpus has been built into `data/knowledge/chunks/pozsar_chunks.jsonl`.

## MCP Prompts

Use the status tool first when checking a local setup:

```text
Use pozsar-corpus to get the Pozsar KB status. Confirm which chunk file is loaded and how many documents, citations, chunks, and themes are available.
```

Research a macro/liquidity question with a compact evidence bundle:

```text
Use pozsar-corpus to answer the research question: How does collateral scarcity affect dollar liquidity? Return the evidence bundle and citations only.
```

Inspect why a result ranked highly:

```text
Use pozsar-corpus to explain why the top results rank for "collateral dollar liquidity". Show score breakdowns, snippets, theme boosts, and citations.
```

Search with a deterministic theme filter:

```text
Use pozsar-corpus to search the Pozsar KB for "repo balance sheet constraints" with theme "repo" and limit 5.
```

Read surrounding page context after a search hit:

```text
Use pozsar-corpus to read page context for doc_id "bretton-woods-iii", page 2, radius 1.
```

Compare raw search and research-question retrieval:

```text
Use pozsar-corpus to search for "outside money commodities" directly, then use answer_pozsar_research_question for the same query. Compare returned citations.
```

Extract advisor-ready macro liquidity signals without trade recommendations:

```text
Use pozsar-corpus to extract liquidity signals for: What does the corpus say about collateral scarcity and dollar liquidity? Use assets BTC, ETH, SPY, QQQ, GLD, TLT, and DXY; themes collateral, dollar_liquidity, and repo; limit 8. Return structured evidence only, not trading advice.
```

## Advisor Policy CLI

Build a deterministic policy artifact from a generated advisor snapshot:

```bash
cargo run -p corpus-cli -- advisor-policy \
  --snapshot data/advisor/snapshot.json \
  --out data/advisor/policy.json
```

## Eval Commands

Run the public fixture through the research-question retrieval path and write a JSON artifact:

```bash
cargo run -p corpus-cli -- eval-search \
  --chunks data/knowledge/chunks/pozsar_chunks.jsonl \
  --cases eval/fixtures/pozsar_eval.json \
  --limit 5 \
  --tool research-question \
  --format json \
  --output data/eval/research-question-report.json
```

Run the same cases against the raw search scorer:

```bash
cargo run -p corpus-cli -- eval-search \
  --chunks data/knowledge/chunks/pozsar_chunks.jsonl \
  --cases eval/fixtures/pozsar_eval.json \
  --limit 5 \
  --tool search \
  --format json \
  --output data/eval/search-report.json
```

Use `--fail-fast` for larger private suites when debugging the first regression.

Evaluate deterministic advisor policy rules against a fixture-backed snapshot:

```bash
cargo run -p corpus-cli -- eval-advisor-policy \
  --cases eval/fixtures/advisor_policy_eval.json \
  --format json \
  --output data/eval/advisor-policy-report.json
```

## Eval Report

See [docs/examples/eval-report.json](examples/eval-report.json) for a sample JSON artifact shape.
