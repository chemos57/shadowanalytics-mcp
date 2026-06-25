use pozsar_kb::artifacts::write_manifest;
use pozsar_kb::manifest::PdfManifestEntry;
use tempfile::tempdir;

#[test]
fn writes_manifest_json() {
    let dir = tempdir().unwrap();
    let output = dir.path().join("manifest.json");
    let entries = vec![PdfManifestEntry {
        doc_id: "doc".to_string(),
        file_name: "doc.pdf".to_string(),
        path: "docs/doc.pdf".to_string(),
        sha256: "abc".to_string(),
        bytes: 12,
    }];

    write_manifest(&entries, &output).unwrap();

    let written = std::fs::read_to_string(output).unwrap();
    assert!(written.contains("\"doc_id\": \"doc\""));
}
