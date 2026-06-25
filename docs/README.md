# Local PDF Corpus

This directory is for local source PDFs used by the Pozsar corpus pipeline.

PDF files are intentionally not tracked in git because this repository may become public and redistribution rights can vary by document. To build the corpus locally, place the source PDFs directly in this directory:

```text
docs/
  Bretton-Woods-III-Zoltan-Pozsar.pdf
  The_Safe_Asset_Glut.pdf
  ...
```

Then rebuild generated artifacts:

```bash
cargo run -p corpus-cli -- build --docs docs --out data/knowledge
cargo run -p corpus-cli -- inspect --out data/knowledge
```

Generated artifacts under `data/knowledge/` are reproducible and ignored by git.
