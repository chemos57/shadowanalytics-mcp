#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHUNKS_JSONL="${POZSAR_CHUNKS_JSONL:-$ROOT/data/knowledge/chunks/pozsar_chunks.jsonl}"
MARKET_CONTEXT_JSON="${POZSAR_MARKET_CONTEXT_JSON:-$ROOT/data/market/context.json}"
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

if [[ ! -f "$MARKET_CONTEXT_JSON" ]]; then
  echo "market context not found: $MARKET_CONTEXT_JSON" >&2
  echo "Build it with: cargo run -p corpus-cli -- market-context --prices data/market/sample_prices.csv --out data/market/context.json" >&2
  exit 1
fi

python3 - "$MCP_BIN" "$CHUNKS_JSONL" "$MARKET_CONTEXT_JSON" <<'PY'
import json
import os
import pathlib
import select
import subprocess
import sys


binary = pathlib.Path(sys.argv[1]).resolve()
chunks_jsonl = pathlib.Path(sys.argv[2]).resolve()
market_context_json = pathlib.Path(sys.argv[3]).resolve()


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


env = os.environ.copy()
env["POZSAR_CHUNKS_JSONL"] = str(chunks_jsonl)
env["POZSAR_MARKET_CONTEXT_JSON"] = str(market_context_json)
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
                    "name": "pozsar-demo-mcp-advisor",
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
                "name": "get_pozsar_advisor_snapshot",
                "arguments": {
                    "question": "What does collateral scarcity imply for BTC and DXY?",
                    "assets": ["BTC", "DXY"],
                    "themes": ["collateral", "dollar_liquidity", "repo"],
                    "limit": 8,
                },
            },
        }
    )
    snapshot = parse_tool_json(read_response())
    if "error" in snapshot:
        raise RuntimeError(snapshot["error"])

    output = {
        "server": initialize["result"]["serverInfo"],
        "chunks_jsonl": str(chunks_jsonl),
        "market_context_json": str(market_context_json),
        "regime": snapshot.get("regime"),
        "confirmations": snapshot.get("confirmations", []),
        "unknowns": snapshot.get("unknowns", []),
        "citations": snapshot.get("liquidity_signals", {}).get("citations", []),
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
