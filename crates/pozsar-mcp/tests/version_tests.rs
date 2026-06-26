use std::process::Command;

#[test]
fn binary_prints_version_without_starting_stdio_server() {
    let output = Command::new(env!("CARGO_BIN_EXE_pozsar-mcp"))
        .arg("--version")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        format!("pozsar-mcp {}", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}
