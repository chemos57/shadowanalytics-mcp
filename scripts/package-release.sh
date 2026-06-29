#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)}"
TARGET_TRIPLE="${TARGET:-$(rustc -vV | awk '/host:/ {print $2}')}"
PACKAGE_NAME="pozsar-corpus-mcp-${VERSION}-${TARGET_TRIPLE}"
PACKAGE_DIR="$ROOT/dist/$PACKAGE_NAME"
ARCHIVE="$ROOT/dist/$PACKAGE_NAME.tar.gz"

if [[ -z "$VERSION" ]]; then
  echo "could not determine workspace version" >&2
  exit 1
fi

mkdir -p "$ROOT/dist"
cargo build --release -p pozsar-mcp

rm -rf "$PACKAGE_DIR" "$ARCHIVE"
mkdir -p "$PACKAGE_DIR/bin" "$PACKAGE_DIR/docs" "$PACKAGE_DIR/eval/fixtures"

cp "$ROOT/target/release/pozsar-mcp" "$PACKAGE_DIR/bin/"
cp "$ROOT/README.md" "$PACKAGE_DIR/"
cp "$ROOT/LICENSE" "$PACKAGE_DIR/"
cp "$ROOT/CHANGELOG.md" "$PACKAGE_DIR/"
cp "$ROOT/Zoltan-Pozsar-Bibliography.html" "$PACKAGE_DIR/"
cp -R "$ROOT/docs/." "$PACKAGE_DIR/docs/"
find "$PACKAGE_DIR/docs" -maxdepth 1 -type f -name '*.pdf' -delete
cp "$ROOT/eval/fixtures/pozsar_eval.json" "$PACKAGE_DIR/eval/fixtures/"

tar -C "$ROOT/dist" -czf "$ARCHIVE" "$PACKAGE_NAME"
echo "wrote release package: $ARCHIVE"
