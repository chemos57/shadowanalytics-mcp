# PDF Corpus

This directory contains source-map and documentation files for the Pozsar corpus pipeline.

PDF files are ignored by git. Rebuild them locally from [SOURCE_MAP.md](SOURCE_MAP.md):

```bash
cargo run -p corpus-cli -- download-sources --docs docs --source-map docs/SOURCE_MAP.md
cargo run -p corpus-cli -- verify-sources --docs docs --bibliography Zoltan-Pozsar-Bibliography.html --source-map docs/SOURCE_MAP.md
```

See [SOURCE_MAP.md](SOURCE_MAP.md) for the public source URL and SHA-256 hash of each PDF.

```text
docs/
  Bretton-Woods-III-Zoltan-Pozsar.pdf
  The_Safe_Asset_Glut.pdf
  ...
```

Rebuild generated artifacts after changing the PDF set:

```bash
cargo run -p corpus-cli -- build --docs docs --out data/knowledge
cargo run -p corpus-cli -- inspect --out data/knowledge
```

Downloaded PDFs and generated artifacts under `data/knowledge/` are reproducible and ignored by git.
