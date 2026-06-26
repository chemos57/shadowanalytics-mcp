#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 2 ]]; then
  echo "usage: $0 <release-archive.tar.gz> <pozsar_chunks.jsonl>" >&2
  exit 2
fi

ARCHIVE="$1"
CHUNKS_JSONL="$2"

if [[ ! -f "$ARCHIVE" ]]; then
  echo "release archive not found: $ARCHIVE" >&2
  exit 1
fi

if [[ ! -f "$CHUNKS_JSONL" ]]; then
  echo "chunks jsonl not found: $CHUNKS_JSONL" >&2
  exit 1
fi

python3 - "$ARCHIVE" "$CHUNKS_JSONL" <<'PY'
import json
import os
import pathlib
import select
import subprocess
import sys
import tarfile
import tempfile


archive = pathlib.Path(sys.argv[1]).resolve()
chunks_jsonl = pathlib.Path(sys.argv[2]).resolve()


def safe_extract(tar: tarfile.TarFile, destination: pathlib.Path) -> None:
    root = destination.resolve()
    for member in tar.getmembers():
        target = (root / member.name).resolve()
        if root != target and root not in target.parents:
            raise RuntimeError(f"unsafe archive member path: {member.name}")
    try:
        tar.extractall(root, filter="data")
    except TypeError:
        tar.extractall(root)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


with tempfile.TemporaryDirectory(prefix="pozsar-mcp-smoke-") as tmp:
    tmpdir = pathlib.Path(tmp)
    with tarfile.open(archive, "r:gz") as tar:
        safe_extract(tar, tmpdir)

    package_roots = [path for path in tmpdir.iterdir() if path.is_dir()]
    require(len(package_roots) == 1, "archive should unpack to exactly one package directory")
    package_root = package_roots[0]
    binary = package_root / "bin" / "pozsar-mcp"

    require(binary.is_file(), f"missing packaged binary: {binary}")
    for required_path in ["README.md", "LICENSE", "CHANGELOG.md", "docs", "eval/fixtures/pozsar_eval.json"]:
        require((package_root / required_path).exists(), f"missing packaged file: {required_path}")

    version = subprocess.run(
        [str(binary), "--version"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    require(version.stdout.strip().startswith("pozsar-mcp "), "unexpected --version output")

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
        proc.stdin.write(json.dumps(message) + "\n")
        proc.stdin.flush()

    def read_response(timeout_seconds: int = 10) -> dict:
        assert proc.stdout is not None
        ready, _, _ = select.select([proc.stdout], [], [], timeout_seconds)
        require(bool(ready), "timed out waiting for MCP response")
        line = proc.stdout.readline()
        require(bool(line), "MCP server closed stdout unexpectedly")
        return json.loads(line)

    send(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {
                    "name": "pozsar-package-smoke-test",
                    "version": "0.1.0",
                },
            },
        }
    )
    initialize = read_response()

    send(
        {
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        }
    )
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
    status_response = read_response()

    assert proc.stdin is not None
    proc.stdin.close()
    stderr = proc.stderr.read() if proc.stderr is not None else ""
    return_code = proc.wait(timeout=10)
    require(return_code == 0, f"MCP server exited with {return_code}: {stderr}")

    require(initialize is not None and "serverInfo" in initialize.get("result", {}), "missing initialize response")
    require(status_response is not None and "result" in status_response, "missing status tool response")

    content = status_response["result"].get("content", [])
    text = "\n".join(item.get("text", "") for item in content if item.get("type") == "text")
    status = json.loads(text)
    require(status["server_name"] == "pozsar-corpus", "unexpected status server_name")
    require(status["chunk_count"] > 0, "status chunk_count should be greater than zero")
    require(status["chunks_path"] == str(chunks_jsonl), "status chunks_path should match smoke input")
    require("get_pozsar_kb_status" in status["tools"], "status tool list is incomplete")

    print(f"version: {version.stdout.strip()}")
    print(f"status: {status['chunk_count']} chunks from {status['chunks_path']}")
    print("package smoke test passed")
PY
