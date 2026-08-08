use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn web_mode_serves_the_embedded_workbench() {
    let port = unused_loopback_port();
    let mut child = start_case_web("text/ascii-fonts-and-styles", port);

    wait_until_listening(&mut child, port);
    let response = http_get(port, "/");
    stop(&mut child);

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("<title>ESCPost render</title>"));
    assert!(response.contains("<div id=\"sheets\""));
    // Anti-aliased renders are smoothed; the faithful dot grid stays pixelated.
    assert!(response.contains("#sheets.antialiased"));
    assert!(response.contains("id=\"margin\""));
    assert!(response.contains("Paper margin"));
    assert!(response.contains("id=\"connection\""));
    assert!(response.contains("id=\"completion\""));
    assert!(response.contains("id=\"jobStatus\""));
    assert!(response.contains("id=\"receiving\""));
    assert!(response.contains("id=\"download\""));
    assert!(response.contains("id=\"warnings\""));
    assert!(response.contains("id=\"magnifyHint\""));
    assert!(response.contains("fetch(\"/api/render\""));
}

#[test]
fn health_endpoint_reports_ok() {
    let port = unused_loopback_port();
    let mut child = start_case_web("text/ascii-fonts-and-styles", port);

    wait_until_listening(&mut child, port);
    let response = http_get_bytes(port, "/health");
    stop(&mut child);

    assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert_eq!(response_body(&response), b"ok");
}

#[test]
fn web_mode_exposes_ordered_sheet_metadata_and_png_bytes() {
    let port = unused_loopback_port();
    let case = "mechanism/reference-full-and-partial-cuts";
    let mut child = start_case_web(case, port);

    wait_until_listening(&mut child, port);
    let metadata_response = http_get_bytes(port, "/api/render");
    let metadata: serde_json::Value = serde_json::from_slice(response_body(&metadata_response))
        .expect("the metadata response should be JSON");
    let sheet_names: Vec<&str> = metadata["sheets"]
        .as_array()
        .expect("sheets should be an array")
        .iter()
        .map(|sheet| sheet["name"].as_str().expect("a sheet should have a name"))
        .collect();
    assert_eq!(
        sheet_names,
        ["sheet-001.png", "sheet-002.png", "sheet-003.png"]
    );
    assert_eq!(metadata["profile"], "REFERENCE");

    let png_response = http_get_bytes(port, "/sheets/2.png");
    stop(&mut child);
    let expected_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases")
        .join(case)
        .join("expected-002.png");
    assert_eq!(
        response_body(&png_response),
        fs::read(expected_path)
            .expect("the expected sheet should be readable")
            .as_slice()
    );
}

#[test]
fn empty_job_web_mode_exposes_an_ordered_empty_sheet_list() {
    let temporary_directory = temporary_directory("empty-job");
    let input_path = temporary_directory.join("empty.bin");
    fs::write(&input_path, []).expect("the empty input should be writable");
    let port = unused_loopback_port();
    let mut child = start_file_web(&input_path, port, false);

    wait_until_listening(&mut child, port);
    let metadata_response = http_get_bytes(port, "/api/render");
    let metadata: serde_json::Value = serde_json::from_slice(response_body(&metadata_response))
        .expect("the metadata response should be JSON");
    stop(&mut child);

    assert_eq!(metadata["profile"], "REFERENCE");
    assert_eq!(
        metadata["sheets"]
            .as_array()
            .expect("sheets should be an array")
            .len(),
        0
    );
    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn web_mode_can_publish_the_same_complete_render_to_a_file() {
    let temporary_directory = temporary_directory("file-and-web");
    let input_path = temporary_directory.join("receipt.bin");
    let output_path = temporary_directory.join("receipt.png");
    fs::write(&input_path, b"Two destinations\n").expect("the input should be writable");
    let port = unused_loopback_port();
    let mut child = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            input_path.to_str().expect("the input path should be UTF-8"),
            "--profile",
            "REFERENCE",
            "--output",
            output_path
                .to_str()
                .expect("the output path should be UTF-8"),
            "--web-listen",
            &format!("127.0.0.1:{port}"),
            "--non-interactive",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the escpost command should start");

    wait_until_listening(&mut child, port);
    let served_png = response_body(&http_get_bytes(port, "/sheets/1.png")).to_vec();
    stop(&mut child);

    assert_eq!(
        fs::read(&output_path).expect("the persisted PNG should exist"),
        served_png
    );
    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn explicit_occupied_web_port_fails_instead_of_falling_back() {
    let occupied =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a port should be reservable");
    let port = occupied
        .local_addr()
        .expect("the listener should have an address")
        .port();
    let case_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/text/ascii-fonts-and-styles");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            case_directory
                .to_str()
                .expect("the case path should be UTF-8"),
            "--web-listen",
            &format!("127.0.0.1:{port}"),
            "--non-interactive",
        ])
        .output()
        .expect("the escpost command should finish");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(&port.to_string()));
}

#[test]
fn explicit_port_zero_reports_the_operating_system_selected_port() {
    let mut child = start_case_web("text/ascii-fonts-and-styles", 0);
    let mut stderr = BufReader::new(
        child
            .stderr
            .take()
            .expect("the web command stderr should be piped"),
    );
    let viewer_line = loop {
        let mut line = String::new();
        stderr
            .read_line(&mut line)
            .expect("web status should be readable");
        assert!(!line.is_empty(), "web viewer URL should be reported");
        if line.starts_with("Web viewer: ") {
            break line;
        }
    };
    let port = viewer_line
        .trim()
        .strip_prefix("Web viewer: http://127.0.0.1:")
        .and_then(|value| value.strip_suffix('/'))
        .and_then(|value| value.parse::<u16>().ok())
        .expect("the web viewer status should contain the selected port");

    let response = http_get(port, "/");
    stop(&mut child);

    assert_ne!(port, 0);
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
}

#[test]
fn watch_mode_replaces_the_render_after_the_source_changes() {
    let temporary_directory = temporary_directory("watch");
    let input_path = temporary_directory.join("receipt.bin");
    fs::write(&input_path, b"Before\n").expect("the initial input should be writable");
    let port = unused_loopback_port();
    let mut child = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            input_path.to_str().expect("the input path should be UTF-8"),
            "--profile",
            "REFERENCE",
            "--watch",
            "--web-listen",
            &format!("127.0.0.1:{port}"),
            "--non-interactive",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the escpost command should start");

    wait_until_listening(&mut child, port);
    let before = response_body(&http_get_bytes(port, "/sheets/1.png")).to_vec();
    fs::write(&input_path, b"After\n").expect("the changed input should be writable");
    let deadline = Instant::now() + Duration::from_secs(5);
    let after = loop {
        let candidate = response_body(&http_get_bytes(port, "/sheets/1.png")).to_vec();
        if candidate != before {
            break candidate;
        }
        assert!(
            Instant::now() < deadline,
            "the watched rendering did not change"
        );
        thread::sleep(Duration::from_millis(50));
    };
    stop(&mut child);

    assert_eq!(&after[..8], b"\x89PNG\r\n\x1a\n");
    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn watch_error_keeps_the_last_complete_render_available() {
    let temporary_directory = temporary_directory("watch-error");
    let input_path = temporary_directory.join("receipt.bin");
    fs::write(&input_path, b"Complete\n").expect("the initial input should be writable");
    let port = unused_loopback_port();
    let mut child = start_file_web(&input_path, port, true);

    wait_until_listening(&mut child, port);
    let previous_png = response_body(&http_get_bytes(port, "/sheets/1.png")).to_vec();
    fs::write(&input_path, b"\x1b").expect("the invalid input should be writable");
    let deadline = Instant::now() + Duration::from_secs(5);
    let error = loop {
        let response = http_get_bytes(port, "/api/render");
        let metadata: serde_json::Value = serde_json::from_slice(response_body(&response))
            .expect("the metadata should remain JSON");
        if let Some(error) = metadata["error"].as_str() {
            break error.to_owned();
        }
        assert!(
            Instant::now() < deadline,
            "the watched render error did not become visible"
        );
        thread::sleep(Duration::from_millis(50));
    };
    let still_available = response_body(&http_get_bytes(port, "/sheets/1.png")).to_vec();
    stop(&mut child);

    assert!(error.contains("truncated ESC command"));
    assert_eq!(still_available, previous_png);
    fs::remove_dir_all(temporary_directory).expect("the test directory should be removable");
}

#[test]
fn watch_mode_rejects_stdin_as_a_mutable_source() {
    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            "-",
            "--format",
            "binary",
            "--profile",
            "REFERENCE",
            "--watch",
            "--non-interactive",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("the escpost command should finish");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("watch mode requires a filesystem source")
    );
}

#[test]
fn web_mode_does_not_serve_missing_sheets_or_filesystem_paths() {
    let port = unused_loopback_port();
    let mut child = start_case_web("text/ascii-fonts-and-styles", port);

    wait_until_listening(&mut child, port);
    let missing = http_get(port, "/sheets/999.png");
    let traversal = http_get(port, "/sheets/../../Cargo.toml");
    stop(&mut child);

    assert!(missing.starts_with("HTTP/1.1 404 Not Found\r\n"));
    assert!(traversal.starts_with("HTTP/1.1 404 Not Found\r\n"));
    assert!(!traversal.contains("[workspace]"));
}

#[test]
fn browser_mode_starts_the_same_web_viewer() {
    let port = unused_loopback_port();
    let case_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/text/ascii-fonts-and-styles");
    let mut child = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            case_directory
                .to_str()
                .expect("the case path should be UTF-8"),
            "--browser",
            "--web-listen",
            &format!("127.0.0.1:{port}"),
            "--non-interactive",
        ])
        // webbrowser honors BROWSER on Linux. A harmless executable verifies
        // the launch path without opening a GUI during automated tests.
        .env("BROWSER", "/bin/true")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the escpost command should start");

    wait_until_listening(&mut child, port);
    let response = http_get(port, "/");
    stop(&mut child);

    assert!(response.contains("<title>ESCPost render</title>"));
}

#[test]
fn non_loopback_listener_prints_a_receipt_exposure_warning() {
    let port = unused_loopback_port();
    let case_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/text/ascii-fonts-and-styles");
    let mut child = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            case_directory
                .to_str()
                .expect("the case path should be UTF-8"),
            "--web-listen",
            &format!("0.0.0.0:{port}"),
            "--non-interactive",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the escpost command should start");

    wait_until_listening(&mut child, port);
    stop(&mut child);
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr should be piped")
        .read_to_string(&mut stderr)
        .expect("stderr should be readable");

    assert!(stderr.contains("warning: receipt data is exposed beyond loopback"));
}

fn start_case_web(case: &str, port: u16) -> Child {
    let case_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases")
        .join(case);
    Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "render",
            case_directory
                .to_str()
                .expect("the case path should be UTF-8"),
            "--web",
            "--web-listen",
            &format!("127.0.0.1:{port}"),
            "--non-interactive",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the escpost command should start")
}

fn start_file_web(input_path: &Path, port: u16, watch: bool) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_escpost"));
    command.args([
        "render",
        input_path.to_str().expect("the input path should be UTF-8"),
        "--profile",
        "REFERENCE",
        "--web-listen",
        &format!("127.0.0.1:{port}"),
        "--non-interactive",
    ]);
    if watch {
        command.arg("--watch");
    }
    command
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the escpost command should start")
}

fn unused_loopback_port() -> u16 {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .expect("an ephemeral loopback port should be available")
        .local_addr()
        .expect("the listener should have a local address")
        .port()
}

fn wait_until_listening(child: &mut Child, port: u16) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok() {
            return;
        }
        if let Some(status) = child
            .try_wait()
            .expect("the child status should be readable")
        {
            let mut stderr = String::new();
            child
                .stderr
                .take()
                .expect("stderr should be piped")
                .read_to_string(&mut stderr)
                .expect("stderr should be readable");
            panic!("web command exited early with {status}:\n{stderr}");
        }
        assert!(
            Instant::now() < deadline,
            "web command did not listen on port {port}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

fn http_get(port: u16, path: &str) -> String {
    String::from_utf8(http_get_bytes(port, path)).expect("the HTTP response should be UTF-8")
}

fn http_get_bytes(port: u16, path: &str) -> Vec<u8> {
    let mut stream =
        TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("the web server should accept HTTP");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .expect("the HTTP request should be writable");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("the HTTP response should be readable");
    response
}

fn response_body(response: &[u8]) -> &[u8] {
    let boundary = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("the HTTP response should contain a header boundary");
    &response[boundary + 4..]
}

fn stop(child: &mut Child) {
    child.kill().expect("the web command should be stoppable");
    child.wait().expect("the web command should be reapable");
}

fn temporary_directory(case: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock should be after the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "escpost-web-{case}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("the test directory should be creatable");
    path
}
