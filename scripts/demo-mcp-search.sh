#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHUNKS_JSONL="${POZSAR_CHUNKS_JSONL:-$ROOT/data/knowledge/chunks/pozsar_chunks.jsonl}"
MCP_BIN="${POZSAR_MCP_BIN:-$ROOT/target/release/pozsar-mcp}"

if [[ ! -x "$MCP_BIN" ]]; then
  echo "MCP binary not found or not executable: $MCP_BIN" >&2
  echo "Build it with: cargo build --release -p pozsar-mcp" >&2
  exit 1
fi

if [[ ! -f "$CHUNKS_JSONL" ]]; then
  echo "chunks artifact not found: $CHUNKS_JSONL" >&2
  echo "Build it with: cargo run -p corpus-cli -- build --docs docs --out data/knowledge" >&2
  exit 1
fi

python3 - "$MCP_BIN" "$CHUNKS_JSONL" <<'PY'
import json
import os
import pathlib
import select
import subprocess
import sys


binary = pathlib.Path(sys.argv[1]).resolve()
chunks_jsonl = pathlib.Path(sys.argv[2]).resolve()
query = "collateral dollar liquidity"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def tool_text(response: dict) -> str:
    if "error" in response:
        raise RuntimeError(json.dumps(response["error"], separators=(",", ":")))
    content = response.get("result", {}).get("content", [])
    return "\n".join(item.get("text", "") for item in content if item.get("type") == "text")


def parse_tool_json(response: dict):
    text = tool_text(response)
    require(bool(text), "MCP tool response did not include text content")
    return json.loads(text)


def summarize_passage(passage: dict) -> dict:
    text = " ".join(str(passage.get("text", "")).split())
    return {
        "doc_id": passage.get("doc_id"),
        "file_name": passage.get("file_name"),
        "page": passage.get("page"),
        "chunk_index": passage.get("chunk_index"),
        "citation": passage.get("citation"),
        "themes": passage.get("themes", []),
        "snippet": text[:240],
    }


env = os.environ.copy()
env["POZSAR_CHUNKS_JSONL"] = str(chunks_jsonl)
proc = subprocess.Popen(
    [str(binary)],
    env=env,
    text=True,
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
)


def send(message: dict) -> None:
    assert proc.stdin is not None
    proc.stdin.write(json.dumps(message, separators=(",", ":")) + "\n")
    proc.stdin.flush()


def read_response(timeout_seconds: int = 20) -> dict:
    assert proc.stdout is not None
    ready, _, _ = select.select([proc.stdout], [], [], timeout_seconds)
    require(bool(ready), "timed out waiting for MCP response")
    line = proc.stdout.readline()
    require(bool(line), "MCP server closed stdout unexpectedly")
    return json.loads(line)


try:
    send(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {
                    "name": "pozsar-demo-mcp-search",
                    "version": "0.1.0",
                },
            },
        }
    )
    initialize = read_response()
    require("serverInfo" in initialize.get("result", {}), "missing initialize serverInfo")

    send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    send(
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "get_pozsar_kb_status",
                "arguments": {},
            },
        }
    )
    status = parse_tool_json(read_response())

    send(
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "search_pozsar_kb",
                "arguments": {
                    "query": query,
                    "limit": 3,
                },
            },
        }
    )
    search_results = parse_tool_json(read_response())
    require(bool(search_results), f"no search results for query: {query}")
    top = search_results[0]

    send(
        {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "read_pozsar_page_context",
                "arguments": {
                    "doc_id": top["doc_id"],
                    "page": top["page"],
                    "radius": 1,
                },
            },
        }
    )
    page_context = parse_tool_json(read_response())

    output = {
        "server": initialize["result"]["serverInfo"],
        "chunks_jsonl": str(chunks_jsonl),
        "status": {
            "chunk_count": status.get("chunk_count"),
            "document_count": status.get("document_count"),
            "citation_count": status.get("citation_count"),
            "theme_count": status.get("theme_count"),
            "chunks_path": status.get("chunks_path"),
        },
        "search": {
            "query": query,
            "top_citation": top.get("citation"),
            "results": [summarize_passage(passage) for passage in search_results],
        },
        "page_context": {
            "doc_id": top.get("doc_id"),
            "page": top.get("page"),
            "radius": 1,
            "chunks": [summarize_passage(passage) for passage in page_context],
        },
    }
    print(json.dumps(output, separators=(",", ":")))
finally:
    if proc.stdin is not None:
        proc.stdin.close()
    stderr = proc.stderr.read() if proc.stderr is not None else ""
    return_code = proc.wait(timeout=10)
    if return_code != 0:
        raise RuntimeError(f"MCP server exited with {return_code}: {stderr}")
PY
