use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn print_help_exposes_a_named_target_instead_of_transport_coordinates() {
    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["print", "--help"])
        .output()
        .expect("the escpost command should finish");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("--printer <PRINTER>"));
    assert!(stdout.contains("--config <FILE>"));
    for obsolete in [
        "--usb-vendor-id",
        "--usb-product-id",
        "--usb-interface",
        "--usb-out-endpoint",
        "--host",
        "--port",
    ] {
        assert!(
            !stdout.contains(obsolete),
            "print should not expose transport option {obsolete}:\n{stdout}"
        );
    }
}

#[test]
fn print_sends_exact_bytes_to_a_named_network_printer() {
    let directory = temporary_directory("named-network");
    let source = directory.join("receipt.bin");
    let configuration = directory.join("printers.toml");
    let expected = b"\x1b@Named network printer\n";
    fs::create_dir_all(&directory).expect("the test directory should be creatable");
    fs::write(&source, expected).expect("the ESC/POS source should be writable");
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("the loopback printer should bind");
    let port = listener
        .local_addr()
        .expect("the listener should have an address")
        .port();
    fs::write(
        &configuration,
        format!(
            "\
[kitchen]
transport = \"network\"
host = \"127.0.0.1\"
port = {port}
"
        ),
    )
    .expect("the printer configuration should be writable");
    let receiver = thread::spawn(move || {
        let (mut connection, _) = listener
            .accept()
            .expect("the named printer should receive a connection");
        let mut bytes = Vec::new();
        connection
            .read_to_end(&mut bytes)
            .expect("the print connection should close cleanly");
        bytes
    });

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "print",
            source.to_str().expect("the source path should be UTF-8"),
            "--printer",
            "kitchen",
            "--config",
            configuration
                .to_str()
                .expect("the configuration path should be UTF-8"),
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should finish");

    assert!(
        output.status.success(),
        "named network printing should succeed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        receiver.join().expect("the receiver should finish"),
        expected
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Printer: kitchen"));
    assert!(stderr.contains("Transport: network"));
    assert!(stderr.contains(&format!("Network target: 127.0.0.1:{port}")));
    assert!(stderr.contains(&format!("Bytes sent: {}", expected.len())));
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[test]
fn non_interactive_print_requires_a_named_printer() {
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
    assert!(
        stderr.contains("printer is required; pass --printer <NAME>"),
        "the missing named target should be actionable:\n{stderr}"
    );
}

#[test]
fn unknown_named_printer_fails_without_attempting_a_connection() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/text/ascii-fonts-and-styles/input.hex");
    let directory = temporary_directory("unknown-printer");
    fs::create_dir_all(&directory).expect("the test directory should be creatable");
    let configuration = directory.join("printers.toml");
    fs::write(
        &configuration,
        "\
[counter]
transport = \"network\"
host = \"127.0.0.1\"
port = 1
",
    )
    .expect("the printer configuration should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "print",
            source.to_str().expect("the source path should be UTF-8"),
            "--printer",
            "missing",
            "--config",
            configuration
                .to_str()
                .expect("the configuration path should be UTF-8"),
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should finish");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(
        "printer \"missing\" is not configured; use `escpost printers list` to see available names"
    ));
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

fn temporary_directory(case: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the system clock should be after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "escpost-print-command-{case}-{}-{unique}",
        std::process::id()
    ))
}
