use std::process::Command;

#[test]
fn profiles_show_prints_vendor_and_calibration_source() {
    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["profiles", "show", "TM-T88III"])
        .output()
        .expect("the escpost command should finish");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "command failed:\n{stdout}");
    assert!(stdout.contains("Epson"), "missing vendor:\n{stdout}");
    assert!(
        stdout.contains("synthesized"),
        "missing calibration source:\n{stdout}"
    );
}

#[test]
fn profiles_show_json_parses_as_a_single_profile_object() {
    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["profiles", "show", "TM-T88III", "--json"])
        .output()
        .expect("the escpost command should finish");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "command failed:\n{stdout}");
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(
        value.is_object(),
        "--json output should be a single object: {value}"
    );
    assert!(
        value.get("canonical_profile_sha256").is_some(),
        "missing canonical_profile_sha256: {value}"
    );
}

#[test]
fn profiles_show_unknown_id_exits_non_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["profiles", "show", "definitely-not-a-profile"])
        .output()
        .expect("the escpost command should finish");

    assert!(
        !output.status.success(),
        "unknown profile id should fail:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
