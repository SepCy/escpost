use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn raw_file_renders_one_png_with_an_explicit_profile() {
    let temporary_directory = temporary_directory("raw-file");
    let input_path = temporary_directory.join("receipt.bin");
    let output_path = temporary_directory.join("receipt.png");
    fs::write(&input_path, b"Hello\n").expect("the input fixture should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            input_path.to_str().expect("the input path should be UTF-8"),
            "--profile",
            "REFERENCE",
            "--output",
            output_path
                .to_str()
                .expect("the output path should be UTF-8"),
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should start");

    assert!(
        output.status.success(),
        "render failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        &fs::read(&output_path).expect("the output PNG should exist")[..8],
        b"\x89PNG\r\n\x1a\n"
    );

    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn hex_extension_decodes_readable_fixture_bytes() {
    let temporary_directory = temporary_directory("hex-file");
    let binary_input = temporary_directory.join("receipt.bin");
    let hex_input = temporary_directory.join("receipt.hex");
    let binary_output = temporary_directory.join("binary.png");
    let hex_output = temporary_directory.join("hex.png");
    fs::write(&binary_input, b"Hi\n").expect("the binary fixture should be writable");
    fs::write(&hex_input, "48 69 0a\n").expect("the hex fixture should be writable");

    render_file(&binary_input, &binary_output);
    render_file(&hex_input, &hex_output);

    assert_eq!(
        fs::read(hex_output).expect("the hex rendering should exist"),
        fs::read(binary_output).expect("the binary rendering should exist")
    );

    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn case_directory_supplies_input_and_profile_metadata() {
    let temporary_directory = temporary_directory("case-directory");
    let output_path = temporary_directory.join("case.png");
    let case_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/text/ascii-fonts-and-styles");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            case_directory
                .to_str()
                .expect("the case path should be UTF-8"),
            "--output",
            output_path
                .to_str()
                .expect("the output path should be UTF-8"),
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should start");

    assert!(
        output.status.success(),
        "render failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        &fs::read(output_path).expect("the case PNG should exist")[..8],
        b"\x89PNG\r\n\x1a\n"
    );

    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn stdin_to_stdout_contains_only_one_png() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            "-",
            "--format",
            "binary",
            "--profile",
            "REFERENCE",
            "--output",
            "-",
            "--non-interactive",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the escpost command should start");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(b"Pipe\n")
        .expect("the fixture should be writable to stdin");

    let output = child
        .wait_with_output()
        .expect("the escpost command should finish");

    assert!(
        output.status.success(),
        "render failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(&output.stdout[..8], b"\x89PNG\r\n\x1a\n");
    assert!(
        output.stderr.is_empty(),
        "successful binary output should not emit status text: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn output_directory_writes_every_sheet_and_ordered_manifest() {
    let temporary_directory = temporary_directory("all-sheets");
    let output_directory = temporary_directory.join("rendered");
    let case_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/mechanism/reference-full-and-partial-cuts");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            case_directory
                .to_str()
                .expect("the case path should be UTF-8"),
            "--output-dir",
            output_directory
                .to_str()
                .expect("the output path should be UTF-8"),
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should start");

    assert!(
        output.status.success(),
        "render failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(output_directory.join("manifest.json"))
            .expect("the manifest should exist"),
        "{\n  \"sheets\": [\n    \"sheet-001.png\",\n    \"sheet-002.png\",\n    \"sheet-003.png\"\n  ]\n}\n"
    );
    for sheet in ["sheet-001.png", "sheet-002.png", "sheet-003.png"] {
        assert_eq!(
            &fs::read(output_directory.join(sheet)).expect("every listed PNG should exist")[..8],
            b"\x89PNG\r\n\x1a\n"
        );
    }

    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn sheet_selection_writes_one_sheet_from_a_multi_sheet_job() {
    let temporary_directory = temporary_directory("selected-sheet");
    let output_path = temporary_directory.join("second.png");
    let case_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/mechanism/reference-full-and-partial-cuts");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            case_directory
                .to_str()
                .expect("the case path should be UTF-8"),
            "--sheet",
            "2",
            "--output",
            output_path
                .to_str()
                .expect("the output path should be UTF-8"),
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should start");

    assert!(
        output.status.success(),
        "render failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(output_path).expect("the selected output should exist"),
        fs::read(case_directory.join("expected-002.png"))
            .expect("the expected second sheet should exist")
    );

    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn global_non_interactive_flag_is_accepted_before_the_subcommand() {
    let temporary_directory = temporary_directory("global-option");
    let input_path = temporary_directory.join("receipt.bin");
    let output_path = temporary_directory.join("receipt.png");
    fs::write(&input_path, b"Global\n").expect("the input fixture should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "--non-interactive",
            "render",
            input_path.to_str().expect("the input path should be UTF-8"),
            "--profile",
            "REFERENCE",
            "--output",
            output_path
                .to_str()
                .expect("the output path should be UTF-8"),
        ])
        .output()
        .expect("the escpost command should start");

    assert!(
        output.status.success(),
        "render failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn explicit_output_is_replaced_only_after_rendering_succeeds() {
    let temporary_directory = temporary_directory("overwrite");
    let input_path = temporary_directory.join("receipt.bin");
    let output_path = temporary_directory.join("receipt.png");
    fs::write(&input_path, b"\x1b").expect("the invalid fixture should be writable");
    fs::write(&output_path, b"previous").expect("the previous output should be writable");

    let failed = render_process(&input_path, &output_path);

    assert!(!failed.status.success(), "truncated ESC should fail");
    assert_eq!(
        fs::read(&output_path).expect("the previous output should remain"),
        b"previous"
    );

    fs::write(&input_path, b"Replacement\n").expect("the valid fixture should be writable");
    render_file(&input_path, &output_path);
    assert_eq!(
        &fs::read(&output_path).expect("the output should be replaced")[..8],
        b"\x89PNG\r\n\x1a\n"
    );

    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

fn render_file(input_path: &Path, output_path: &Path) {
    let output = render_process(input_path, output_path);

    assert!(
        output.status.success(),
        "render failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn render_process(input_path: &Path, output_path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            input_path.to_str().expect("the input path should be UTF-8"),
            "--profile",
            "REFERENCE",
            "--output",
            output_path
                .to_str()
                .expect("the output path should be UTF-8"),
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should start")
}

fn temporary_directory(case: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock should be after the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "escpost-cli-{case}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("the test directory should be creatable");
    path
}
