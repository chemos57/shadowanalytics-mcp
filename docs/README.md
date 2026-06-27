# PDF Corpus

This directory contains source PDFs used by the Pozsar corpus pipeline.

PDF files are tracked so the corpus can be rebuilt from the same source documents across machines. Before adding new PDFs to a public repository, confirm their redistribution terms.

See [SOURCE_MAP.md](SOURCE_MAP.md) for the public source URL and SHA-256 hash of each tracked PDF.

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

Generated artifacts under `data/knowledge/` are reproducible and ignored by git.
