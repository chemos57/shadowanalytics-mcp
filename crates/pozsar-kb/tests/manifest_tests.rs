use pozsar_kb::manifest::{build_manifest, doc_id_from_file_name};
use std::fs;
use tempfile::tempdir;

#[test]
fn doc_id_is_stable_from_file_name() {
    assert_eq!(
        doc_id_from_file_name("Bretton-Woods-III-Zoltan-Pozsar.pdf"),
        "bretton-woods-iii-zoltan-pozsar"
    );
}

#[test]
fn manifest_lists_only_pdfs() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Bretton-Woods-III-Zoltan-Pozsar.pdf"),
        b"%PDF-test",
    )
    .unwrap();
    fs::write(dir.path().join("notes.txt"), b"ignore").unwrap();

    let manifest = build_manifest(dir.path()).unwrap();

    assert_eq!(manifest.len(), 1);
    assert_eq!(manifest[0].file_name, "Bretton-Woods-III-Zoltan-Pozsar.pdf");
    assert_eq!(manifest[0].bytes, 9);
}
