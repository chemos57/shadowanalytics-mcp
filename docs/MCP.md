# MCP Usage

This project exposes the generated Pozsar corpus as a read-only stdio MCP server.

Build or refresh the corpus first:

```bash
cargo run -p corpus-cli -- build --docs docs --out data/knowledge
cargo run -p corpus-cli -- inspect --out data/knowledge
```

Then build the MCP binary:

```bash
cargo build --release -p pozsar-mcp
```

Check the binary version without starting the stdio server:

```bash
target/release/pozsar-mcp --version
```

The server reads chunks from `data/knowledge/chunks/pozsar_chunks.jsonl` by default and uses `data/market/context.json` as the default market context path for advisor snapshots. If the default market context is used and `data/market/context.health.json` exists, it is included automatically; use `POZSAR_MARKET_CONTEXT_HEALTH_JSON` to point at another health sidecar for custom contexts.

```bash
POZSAR_CHUNKS_JSONL=/absolute/path/to/pozsar_chunks.jsonl \
POZSAR_MARKET_CONTEXT_JSON=/absolute/path/to/context.json \
POZSAR_MARKET_CONTEXT_HEALTH_JSON=/absolute/path/to/context.health.json \
  /absolute/path/to/zp_base/target/release/pozsar-mcp
```

Use absolute paths in MCP client config files. Relative paths depend on the client process working directory and are easy to misconfigure.

## Claude Desktop

Add the server to Claude Desktop's MCP config.

Common config locations:

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

Example:

```json
{
  "mcpServers": {
    "pozsar-corpus": {
      "command": "/absolute/path/to/zp_base/target/release/pozsar-mcp",
      "env": {
        "POZSAR_CHUNKS_JSONL": "/absolute/path/to/zp_base/data/knowledge/chunks/pozsar_chunks.jsonl",
        "POZSAR_MARKET_CONTEXT_JSON": "/absolute/path/to/zp_base/data/market/context.json",
        "POZSAR_MARKET_CONTEXT_HEALTH_JSON": "/absolute/path/to/zp_base/data/market/context.health.json"
      }
    }
  }
}
```

Restart Claude Desktop after editing the config.

## Codex

Codex supports stdio MCP servers through `config.toml`. You can use the CLI or edit config directly.

CLI form:

```bash
codex mcp add pozsar-corpus \
  --env POZSAR_CHUNKS_JSONL=/absolute/path/to/zp_base/data/knowledge/chunks/pozsar_chunks.jsonl \
  --env POZSAR_MARKET_CONTEXT_JSON=/absolute/path/to/zp_base/data/market/context.json \
  --env POZSAR_MARKET_CONTEXT_HEALTH_JSON=/absolute/path/to/zp_base/data/market/context.health.json \
  -- /absolute/path/to/zp_base/target/release/pozsar-mcp
```

Config form, either in `~/.codex/config.toml` or project-scoped `.codex/config.toml` for a trusted project:

```toml
[mcp_servers.pozsar_corpus]
command = "/absolute/path/to/zp_base/target/release/pozsar-mcp"
startup_timeout_sec = 20
tool_timeout_sec = 60
enabled = true

[mcp_servers.pozsar_corpus.env]
POZSAR_CHUNKS_JSONL = "/absolute/path/to/zp_base/data/knowledge/chunks/pozsar_chunks.jsonl"
POZSAR_MARKET_CONTEXT_JSON = "/absolute/path/to/zp_base/data/market/context.json"
POZSAR_MARKET_CONTEXT_HEALTH_JSON = "/absolute/path/to/zp_base/data/market/context.health.json"
```

In the Codex TUI, use `/mcp` to inspect configured MCP servers.

Development alternative:

```toml
[mcp_servers.pozsar_corpus_dev]
command = "cargo"
args = ["run", "--quiet", "-p", "pozsar-mcp"]
cwd = "/absolute/path/to/zp_base"
startup_timeout_sec = 30
tool_timeout_sec = 60
enabled = true

[mcp_servers.pozsar_corpus_dev.env]
POZSAR_CHUNKS_JSONL = "/absolute/path/to/zp_base/data/knowledge/chunks/pozsar_chunks.jsonl"
POZSAR_MARKET_CONTEXT_JSON = "/absolute/path/to/zp_base/data/market/context.json"
POZSAR_MARKET_CONTEXT_HEALTH_JSON = "/absolute/path/to/zp_base/data/market/context.health.json"
```

Prefer the release binary for normal use because it avoids compile-time startup delays.

## Tools

All tools are read-only.

### `get_pozsar_kb_status`

Returns server metadata and corpus artifact counts.

Input:

```json
{}
```

Output:

```json
{
  "server_name": "pozsar-corpus",
  "server_version": "0.1.0",
  "default_chunks_jsonl": "data/knowledge/chunks/pozsar_chunks.jsonl",
  "default_market_context_json": "data/market/context.json",
  "default_market_context_health_json": "data/market/context.health.json",
  "chunks_path": "/absolute/path/to/zp_base/data/knowledge/chunks/pozsar_chunks.jsonl",
  "market_context_path": "/absolute/path/to/zp_base/data/market/context.json",
  "market_context_health_path": "/absolute/path/to/zp_base/data/market/context.health.json",
  "chunk_count": 421,
  "document_count": 18,
  "citation_count": 217,
  "theme_count": 8,
  "tools": [
    "get_pozsar_kb_status",
    "list_pozsar_docs",
    "list_pozsar_themes",
    "search_pozsar_kb",
    "explain_pozsar_search",
    "read_pozsar_source",
    "read_pozsar_page_context",
    "answer_pozsar_research_question",
    "extract_pozsar_liquidity_signals",
    "get_pozsar_advisor_snapshot",
    "get_pozsar_advisor_policy"
  ]
}
```

Use this first when debugging an MCP client config. It confirms that the server loaded the expected chunk artifact, has the expected market context path configured, and exposes the expected tool set.

### `list_pozsar_docs`

Lists documents represented in the chunk artifact.

Input:

```json
{}
```

Output:

```json
[
  {
    "doc_id": "bretton-woods-iii-zoltan-pozsar",
    "file_name": "Bretton-Woods-III-Zoltan-Pozsar.pdf",
    "chunks": 12
  }
]
```

### `list_pozsar_themes`

Lists deterministic themes present in the corpus.

Input:

```json
{}
```

Output:

```json
[
  "collateral",
  "commodities",
  "dollar_liquidity",
  "fx_swaps",
  "repo",
  "shadow_banking"
]
```

### `search_pozsar_kb`

Searches source-cited corpus chunks.

Input:

```json
{
  "query": "collateral dollar liquidity",
  "limit": 5,
  "theme": "collateral",
  "doc_id": "bretton-woods-iii-zoltan-pozsar",
  "file_name": "Bretton-Woods-III-Zoltan-Pozsar.pdf",
  "page": 1
}
```

Parameters:

- `query` required string.
- `limit` optional integer, clamped to `1..=10`, default `5`.
- `theme` optional string, exact theme match, case-insensitive.
- `doc_id` optional string, exact match.
- `file_name` optional string, exact match, case-insensitive.
- `page` optional integer, exact match.

Filters combine with AND semantics. Omit filters to search the whole corpus.

Output:

```json
[
  {
    "doc_id": "bretton-woods-iii-zoltan-pozsar",
    "file_name": "Bretton-Woods-III-Zoltan-Pozsar.pdf",
    "page": 1,
    "chunk_index": 0,
    "themes": ["collateral", "dollar_liquidity"],
    "text": "Chunk text...",
    "citation": "Bretton-Woods-III-Zoltan-Pozsar.pdf:1"
  }
]
```

Example queries:

```json
{"query": "repo balance sheet constraints", "limit": 5}
```

```json
{"query": "Bretton Woods III commodities", "theme": "commodities", "limit": 3}
```

```json
{"query": "dollar liquidity", "doc_id": "safe-asset-glut", "limit": 5}
```

### `explain_pozsar_search`

Runs the same search path as `search_pozsar_kb`, but returns scoring details.

Input: same as `search_pozsar_kb`.

Output:

```json
[
  {
    "passage": {
      "doc_id": "bretton-woods-iii-zoltan-pozsar",
      "file_name": "Bretton-Woods-III-Zoltan-Pozsar.pdf",
      "page": 1,
      "chunk_index": 0,
      "themes": ["commodities", "dollar_liquidity"],
      "text": "Chunk text...",
      "citation": "Bretton-Woods-III-Zoltan-Pozsar.pdf:1"
    },
    "score": 137,
    "phrase_hits": ["text:dollar liquidity"],
    "term_hits": [
      {
        "term": "dollar",
        "text_count": 2,
        "title_count": 0,
        "theme_count": 1,
        "citation_count": 0
      }
    ],
    "title_boosts": [],
    "theme_boosts": ["theme:dollar"],
    "citation_boosts": [],
    "duplicate_citation": false,
    "citation": "Bretton-Woods-III-Zoltan-Pozsar.pdf:1"
  }
]
```

Use this tool when ranking looks surprising or when tuning the search layer.

### `answer_pozsar_research_question`

Builds a compact source-cited evidence bundle for a research question. This tool does not generate an analytical answer; it returns deterministic evidence for a client or advisor layer to reason over.

Input:

```json
{
  "question": "How does collateral affect dollar liquidity?",
  "themes": ["collateral", "dollar_liquidity"],
  "doc_id": "bretton-woods-iii-zoltan-pozsar",
  "limit": 5
}
```

Parameters:

- `question` required string.
- `themes` optional array of theme labels. When present, the tool runs additional theme-filtered searches.
- `doc_id` optional string. When present, all internal searches are restricted to that document.
- `limit` optional integer, clamped to `1..=10`, default `5`.

Output:

```json
{
  "question": "How does collateral affect dollar liquidity?",
  "query_plan": [
    {
      "kind": "original_question",
      "query": "How does collateral affect dollar liquidity?",
      "theme": null,
      "doc_id": null
    },
    {
      "kind": "key_phrase",
      "query": "collateral dollar liquidity",
      "theme": null,
      "doc_id": null
    },
    {
      "kind": "theme_filtered",
      "query": "How does collateral affect dollar liquidity?",
      "theme": "collateral",
      "doc_id": null
    }
  ],
  "evidence": [
    {
      "citation": "Bretton-Woods-III-Zoltan-Pozsar.pdf:1",
      "passage": {
        "doc_id": "bretton-woods-iii-zoltan-pozsar",
        "file_name": "Bretton-Woods-III-Zoltan-Pozsar.pdf",
        "page": 1,
        "chunk_index": 0,
        "themes": ["collateral", "dollar_liquidity"],
        "text": "Chunk text...",
        "citation": "Bretton-Woods-III-Zoltan-Pozsar.pdf:1"
      },
      "score": 137,
      "score_breakdown": {
        "text_phrase": 105,
        "text_terms": 16,
        "title": 0,
        "theme": 18,
        "citation": 0
      },
      "snippet": "Matched source snippet...",
      "query_sources": ["original_question", "key_phrase"],
      "context": [
        {
          "doc_id": "bretton-woods-iii-zoltan-pozsar",
          "file_name": "Bretton-Woods-III-Zoltan-Pozsar.pdf",
          "page": 1,
          "chunk_index": 0,
          "themes": ["collateral", "dollar_liquidity"],
          "text": "Chunk text...",
          "citation": "Bretton-Woods-III-Zoltan-Pozsar.pdf:1"
        }
      ]
    }
  ],
  "citations": ["Bretton-Woods-III-Zoltan-Pozsar.pdf:1"],
  "suggested_followups": [
    "Search adjacent pages for how collateral connects to the question."
  ]
}
```

Use this as the default tool for open-ended corpus research. It fans out through the original question, a deterministic key-phrase query, and optional theme-filtered searches, then deduplicates evidence by document page and includes neighboring page context.

### `extract_pozsar_liquidity_signals`

Builds deterministic, evidence-only macro liquidity signals from corpus evidence and maps them into cross-asset implications for a future advisor layer. This tool does not generate trade recommendations, position sizing, execution instructions, or financial advice.

Input:

```json
{
  "question": "What does the corpus say about collateral scarcity and dollar liquidity?",
  "assets": ["BTC", "ETH", "SPY", "QQQ", "GLD", "TLT", "DXY"],
  "themes": ["collateral", "dollar_liquidity", "repo"],
  "limit": 8
}
```

Parameters:

- `question` required string.
- `assets` required array of asset symbols. Symbols are normalized to uppercase and deduplicated.
- `themes` optional array of theme labels. When present, the tool runs additional theme-filtered searches through the research bundle path.
- `limit` optional integer, clamped to `1..=10`, default `5`.

Output:

```json
{
  "question": "What does the corpus say about collateral scarcity and dollar liquidity?",
  "macro_themes": ["collateral", "dollar_liquidity", "repo"],
  "liquidity_conditions": [
    {
      "label": "collateral_scarcity",
      "direction": "tightening",
      "confidence": "medium",
      "evidence": [
        {
          "citation": "a-decade-on-money-31.pdf:3",
          "doc_id": "a-decade-on-money-31",
          "page": 3,
          "themes": ["collateral", "repo", "dollar_liquidity"],
          "snippet": "Matched source snippet...",
          "query_sources": ["original_question", "theme_filtered"]
        }
      ]
    }
  ],
  "cross_asset_implications": [
    {
      "asset": "DXY",
      "bias": "supportive",
      "reason": "Corpus evidence points to tighter dollar funding conditions, which can increase demand for dollar liquidity.",
      "citations": ["a-decade-on-money-31.pdf:3"]
    },
    {
      "asset": "BTC",
      "bias": "risk_negative",
      "reason": "Corpus evidence points to liquidity tightening, a macro condition that can pressure duration-sensitive or speculative risk assets.",
      "citations": ["a-decade-on-money-31.pdf:3"]
    }
  ],
  "unknowns": [
    "No live market data included",
    "Corpus evidence only",
    "No execution recommendation, position sizing, or risk limit included"
  ],
  "citations": ["a-decade-on-money-31.pdf:3"]
}
```

Use this as the bridge between corpus retrieval and a future trading advisor. It structures macro/liquidity evidence for downstream systems, but downstream systems must still add market data, trend, volatility, positioning, risk constraints, and execution rules.

### `get_pozsar_advisor_snapshot`

Builds a deterministic advisor-ready snapshot from Pozsar corpus liquidity evidence plus offline market context. This tool is read-only and does not generate trade recommendations, position sizing, execution instructions, or financial advice.

Input:

```json
{
  "question": "What does collateral scarcity imply for BTC and DXY?",
  "assets": ["BTC", "DXY"],
  "themes": ["collateral", "dollar_liquidity", "repo"],
  "market_context_path": "data/market/context.json",
  "market_context_health_path": "data/market/context.health.json",
  "limit": 8
}
```

Parameters:

- `question` required string.
- `assets` required array of asset symbols. Symbols are normalized by the liquidity signal layer.
- `themes` optional array of theme labels used to guide corpus evidence retrieval.
- `market_context_path` optional string. When omitted, the server uses `POZSAR_MARKET_CONTEXT_JSON`, falling back to `data/market/context.json`.
- `market_context_health_path` optional string. When omitted, the server uses `POZSAR_MARKET_CONTEXT_HEALTH_JSON` only with the server-level market context. If the request overrides `market_context_path`, provide a matching `market_context_health_path` or the snapshot omits `market_context_health`.
- When health metadata is provided, `market_context_health.as_of` must match `market_context.as_of`; otherwise the tool returns an error instead of emitting a potentially stale health block.
- `limit` optional integer, clamped by the liquidity signal layer to `1..=10`, default `5`.

Output:

```json
{
  "question": "What does collateral scarcity imply for BTC and DXY?",
  "liquidity_signals": {
    "question": "What does collateral scarcity imply for BTC and DXY?",
    "macro_themes": ["collateral", "dollar_liquidity", "repo"],
    "liquidity_conditions": [
      {
        "label": "collateral_scarcity",
        "direction": "tightening",
        "confidence": "medium",
        "evidence": [
          {
            "citation": "a-decade-on-money-31.pdf:3",
            "doc_id": "a-decade-on-money-31",
            "page": 3,
            "themes": ["collateral", "repo", "dollar_liquidity"],
            "snippet": "Matched source snippet...",
            "query_sources": ["original_question", "theme_filtered"]
          }
        ]
      }
    ],
    "cross_asset_implications": [
      {
        "asset": "BTC",
        "bias": "risk_negative",
        "reason": "Corpus evidence points to liquidity tightening, a macro condition that can pressure duration-sensitive or speculative risk assets.",
        "citations": ["a-decade-on-money-31.pdf:3"]
      },
      {
        "asset": "DXY",
        "bias": "supportive",
        "reason": "Corpus evidence points to tighter dollar funding conditions, which can increase demand for dollar liquidity.",
        "citations": ["a-decade-on-money-31.pdf:3"]
      }
    ],
    "unknowns": ["Corpus evidence only"],
    "citations": ["a-decade-on-money-31.pdf:3"]
  },
  "market_context": {
    "as_of": "2026-06-30",
    "assets": [
      {
        "symbol": "BTC",
        "trend_20d": "up"
      }
    ],
    "cross_asset": {
      "risk_regime": "risk_on",
      "dxy_trend": "up",
      "rates_proxy": "TLT_up"
    }
  },
  "market_context_health": {
    "status": "ok",
    "as_of": "2026-06-30",
    "missing_assets": [],
    "stale_assets": [],
    "warnings": [],
    "blocking_issues": []
  },
  "confirmations": [
    {
      "asset": "BTC",
      "macro_bias": "risk_negative",
      "market_trend": "up",
      "alignment": "divergent",
      "reason": "Macro liquidity bias is risk_negative, but BTC trend is up."
    }
  ],
  "regime": {
    "macro_liquidity": "tightening",
    "market_risk": "risk_on",
    "combined": "macro_tightening_market_risk_on"
  },
  "unknowns": [
    "No live data",
    "No position sizing",
    "No execution recommendation",
    "Advisor snapshot is deterministic context, not financial advice"
  ]
}
```

Run a raw stdio demo:

```bash
scripts/demo-mcp-advisor.sh
```

Use this when an MCP client needs one deterministic object combining corpus macro/liquidity evidence with current offline market context. A later advisor or portfolio layer can consume this snapshot together with risk rules, but this tool itself remains evidence/context only.

### `get_pozsar_advisor_policy`

Builds deterministic advisor policy assessments from Pozsar corpus liquidity evidence plus offline market context. This tool reuses the advisor snapshot pipeline internally, then converts the snapshot into compact per-asset policy assessments. It is read-only and does not generate trade recommendations, position sizing, execution instructions, or financial advice.

Input:

```json
{
  "question": "What does collateral scarcity imply for BTC and DXY?",
  "assets": ["BTC", "DXY"],
  "themes": ["collateral", "dollar_liquidity", "repo"],
  "market_context_path": "data/market/context.json",
  "market_context_health_path": "data/market/context.health.json",
  "limit": 8
}
```

Parameters are the same as `get_pozsar_advisor_snapshot`.

Output:

```json
{
  "as_of": "2026-06-30",
  "regime": "macro_tightening_market_risk_on",
  "asset_assessments": [
    {
      "asset": "BTC",
      "macro_bias": "risk_negative",
      "market_trend": "up",
      "alignment": "divergent",
      "stance": "watch",
      "confidence": "medium",
      "drivers": [
        "Macro liquidity evidence is risk-negative",
        "BTC trend is still up"
      ],
      "risks": [
        "Macro/market divergence",
        "No positioning data",
        "No volatility regime rules"
      ],
      "required_checks": [
        "Confirm current volatility",
        "Check portfolio exposure",
        "Check invalidation level"
      ]
    }
  ],
  "unknowns": [
    "No live data",
    "No position sizing",
    "No execution recommendation",
    "No positions",
    "No account risk limits"
  ]
}
```

Use this when an MCP client needs the final deterministic advisor interpretation object. It remains context-only; a future portfolio/risk layer must still add positions, constraints, exposure limits, time horizon, and execution rules before any recommendation logic exists.

### `read_pozsar_source`

Reads all chunks for one exact document page.

Input:

```json
{
  "doc_id": "bretton-woods-iii-zoltan-pozsar",
  "page": 1
}
```

Output: array of `SourceCitedPassage` objects, same shape as `search_pozsar_kb`.

### `read_pozsar_page_context`

Reads neighboring page chunks around a source page.

Input:

```json
{
  "doc_id": "bretton-woods-iii-zoltan-pozsar",
  "page": 10,
  "radius": 1
}
```

Parameters:

- `doc_id` required string.
- `page` required integer.
- `radius` optional integer, default `1`, clamped to max `5`.

Output: array of `SourceCitedPassage` objects sorted by page, then chunk index.

Use this after `search_pozsar_kb` finds a relevant page and you need surrounding source context.

## Troubleshooting

If the MCP client starts but tools return empty arrays:

1. Call `get_pozsar_kb_status` and confirm `chunk_count` is greater than zero.
2. Confirm PDFs exist under `docs/`.
3. Rebuild artifacts with `corpus-cli build`.
4. Run `corpus-cli inspect` and verify `validation_issues: 0`.
5. Confirm `POZSAR_CHUNKS_JSONL` points to the intended `pozsar_chunks.jsonl`.
6. For `get_pozsar_advisor_snapshot`, confirm `POZSAR_MARKET_CONTEXT_JSON` or `market_context_path` points to a valid market context JSON file.
7. If health metadata is expected, confirm `POZSAR_MARKET_CONTEXT_HEALTH_JSON` or `market_context_health_path` points to a valid health sidecar JSON file.
8. Restart the MCP client after changing config.

If the MCP client cannot start the server:

1. Use an absolute path to `target/release/pozsar-mcp`.
2. Run the binary manually from a terminal and check for errors.
3. Increase `startup_timeout_sec` for Codex or equivalent startup timeout in the client.
4. Prefer the release binary over `cargo run` for configured clients.

## Release Package

Build a local release tarball:

```bash
scripts/package-release.sh
```

The archive is written under `dist/` and includes the `pozsar-mcp` release binary, `README.md`, `LICENSE`, `CHANGELOG.md`, `Zoltan-Pozsar-Bibliography.html`, the tracked `docs/` directory without downloaded PDFs, and the public eval fixture. It does not include generated corpus artifacts.

Smoke-test the release archive:

```bash
scripts/smoke-package.sh \
  dist/pozsar-corpus-mcp-0.1.0-<target>.tar.gz \
  data/knowledge/chunks/pozsar_chunks.jsonl
```

The smoke test unpacks the archive, runs `bin/pozsar-mcp --version`, starts the packaged MCP server with `POZSAR_CHUNKS_JSONL`, and calls `get_pozsar_kb_status` over stdio.

Before publishing a public release, complete [PUBLICATION_CHECKLIST.md](PUBLICATION_CHECKLIST.md).
