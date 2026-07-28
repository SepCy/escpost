use std::path::PathBuf;
use std::process::Command;

#[test]
fn print_requires_the_complete_explicit_usb_target() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/text/ascii-fonts-and-styles/input.hex");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "print",
            source.to_str().expect("the source path should be UTF-8"),
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should finish");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    for required in [
        "--usb-vendor-id",
        "--usb-product-id",
        "--usb-interface",
        "--usb-out-endpoint",
    ] {
        assert!(
            stderr.contains(required),
            "missing required option {required} was not reported:\n{stderr}"
        );
    }
}

#[test]
fn hexadecimal_usb_values_are_parsed_before_out_endpoint_validation() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/text/ascii-fonts-and-styles/input.hex");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "print",
            source.to_str().expect("the source path should be UTF-8"),
            "--usb-vendor-id",
            "0x0416",
            "--usb-product-id",
            "0x5011",
            "--usb-interface",
            "0x00",
            "--usb-out-endpoint",
            "0x81",
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should finish");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("USB OUT endpoint must be between 0x01 and 0x0f")
    );
}

#[test]
fn decimal_usb_values_are_parsed_before_out_endpoint_validation() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/text/ascii-fonts-and-styles/input.hex");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "print",
            source.to_str().expect("the source path should be UTF-8"),
            "--usb-vendor-id",
            "1046",
            "--usb-product-id",
            "20497",
            "--usb-interface",
            "0",
            "--usb-out-endpoint",
            "129",
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should finish");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("USB OUT endpoint must be between 0x01 and 0x0f")
    );
}
