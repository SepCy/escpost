use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn serve_captures_a_raw_job_and_previews_its_sheets() {
    let mut child = start_serve_on_ephemeral_ports();
    let (raw_port, web_port) = read_listen_ports(&mut child);

    wait_until_listening(&mut child, raw_port);
    wait_until_listening(&mut child, web_port);

    // A RAW/AppSocket client sends one job and closes the connection, which is
    // the default end-of-job boundary.
    send_raw_job(raw_port, b"Captured over RAW\n");

    let metadata = wait_for_first_job(web_port);
    let sheets = metadata["sheets"]
        .as_array()
        .expect("sheets should be an array");
    assert!(!sheets.is_empty(), "the captured job should render a sheet");
    assert_eq!(metadata["profile"], "REFERENCE");

    let png = response_body(&http_get_bytes(web_port, "/sheets/1.png")).to_vec();
    stop(&mut child);
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn serve_replaces_the_preview_with_the_most_recent_job() {
    let mut child = start_serve_on_ephemeral_ports();
    let (raw_port, web_port) = read_listen_ports(&mut child);
    wait_until_listening(&mut child, raw_port);
    wait_until_listening(&mut child, web_port);

    send_raw_job(raw_port, b"First job\n");
    wait_for_first_job(web_port);
    let first = response_body(&http_get_bytes(web_port, "/sheets/1.png")).to_vec();

    send_raw_job(raw_port, b"A visibly different second job\n");
    let deadline = Instant::now() + Duration::from_secs(5);
    let second = loop {
        let candidate = response_body(&http_get_bytes(web_port, "/sheets/1.png")).to_vec();
        if candidate != first {
            break candidate;
        }
        assert!(
            Instant::now() < deadline,
            "the preview did not advance to the second job"
        );
        thread::sleep(Duration::from_millis(50));
    };
    stop(&mut child);
    assert_eq!(&second[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn serve_viewer_shows_instructions_before_the_first_job() {
    let mut child = start_serve_on_ephemeral_ports();
    let (raw_port, web_port) = read_listen_ports(&mut child);
    wait_until_listening(&mut child, web_port);

    let metadata = render_metadata(web_port);
    stop(&mut child);

    assert_eq!(
        metadata["sheets"]
            .as_array()
            .expect("sheets should be an array")
            .len(),
        0,
        "no job has been captured yet"
    );
    let hint = metadata["hint"]
        .as_str()
        .expect("an idle viewer should carry a waiting hint");
    assert!(
        hint.contains(&format!("127.0.0.1:{raw_port}")),
        "the hint should tell the developer where to send data:\n{hint}"
    );
}

#[test]
fn serve_reassembles_a_job_sent_one_byte_at_a_time() {
    let mut child = start_serve_on_ephemeral_ports();
    let (raw_port, web_port) = read_listen_ports(&mut child);
    wait_until_listening(&mut child, raw_port);
    wait_until_listening(&mut child, web_port);

    // Deliver the receipt one byte per write so the server must reassemble the
    // job across many reads, as it would with a fragmenting client.
    send_raw_job_one_byte_at_a_time(raw_port, b"Fragmented receipt\n");

    let metadata = wait_for_first_job(web_port);
    assert!(
        !metadata["sheets"]
            .as_array()
            .expect("sheets should be an array")
            .is_empty(),
        "a byte-by-byte job should still render"
    );
    let png = response_body(&http_get_bytes(web_port, "/sheets/1.png")).to_vec();
    stop(&mut child);
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn serve_defaults_the_profile_to_reference() {
    // No --profile: a virtual printer should preview with REFERENCE.
    let mut child = start_serve(&["--listen", "127.0.0.1:0", "--web-listen", "127.0.0.1:0"]);
    let (raw_port, web_port) = read_listen_ports(&mut child);
    wait_until_listening(&mut child, raw_port);
    wait_until_listening(&mut child, web_port);

    send_raw_job(raw_port, b"Default profile\n");
    let metadata = wait_for_first_job(web_port);
    stop(&mut child);

    assert_eq!(metadata["profile"], "REFERENCE");
}

fn start_serve(arguments: &[&str]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["--non-interactive", "serve"])
        .args(arguments)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the escpost command should start")
}

/// Bind both listeners to operating-system-selected ports so parallel tests do
/// not contend for the conventional 9100/9000 defaults.
fn start_serve_on_ephemeral_ports() -> Child {
    start_serve(&[
        "--profile",
        "REFERENCE",
        "--listen",
        "127.0.0.1:0",
        "--web-listen",
        "127.0.0.1:0",
    ])
}

/// Both listeners bind an operating-system-selected port, so the actual ports
/// are read back from the status the server prints.
fn read_listen_ports(child: &mut Child) -> (u16, u16) {
    let mut stderr = BufReader::new(
        child
            .stderr
            .take()
            .expect("the serve command stderr should be piped"),
    );
    let mut raw = None;
    let mut web = None;
    while raw.is_none() || web.is_none() {
        let mut line = String::new();
        let read = stderr
            .read_line(&mut line)
            .expect("serve status should be readable");
        assert!(read != 0, "serve exited before reporting its listen ports");
        if let Some(port) = line
            .trim()
            .strip_prefix("RAW printer: 127.0.0.1:")
            .and_then(|value| value.parse::<u16>().ok())
        {
            raw = Some(port);
        }
        if let Some(port) = line
            .trim()
            .strip_prefix("Web viewer: http://127.0.0.1:")
            .and_then(|value| value.strip_suffix('/'))
            .and_then(|value| value.parse::<u16>().ok())
        {
            web = Some(port);
        }
    }

    // Keep the stderr pipe open and drained for the rest of the run. Dropping it
    // here would close the read end, so the server's next status line would hit
    // a broken pipe and abort. The thread ends at EOF when the child is stopped.
    thread::spawn(move || {
        let mut discard = String::new();
        let _ = stderr.read_to_string(&mut discard);
    });

    (raw.unwrap(), web.unwrap())
}

fn send_raw_job(port: u16, bytes: &[u8]) {
    // The CLI service uses host networking, so a loopback connection can be
    // refused transiently while the host is busy. Retry until a deadline.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match raw_send_once(port, bytes) {
            Ok(()) => return,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "the RAW printer never accepted data: {error}"
                );
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

fn raw_send_once(port: u16, bytes: &[u8]) -> std::io::Result<()> {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))?;
    stream.write_all(bytes)?;
    // Dropping the stream closes the connection, completing the job.
    Ok(())
}

fn send_raw_job_one_byte_at_a_time(port: u16, bytes: &[u8]) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        match TcpStream::connect((Ipv4Addr::LOCALHOST, port)) {
            Ok(stream) => break stream,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "the RAW printer never accepted data: {error}"
                );
                thread::sleep(Duration::from_millis(25));
            }
        }
    };
    // Disable Nagle and flush each byte so the writes reach the server as
    // separate reads rather than one coalesced buffer.
    stream
        .set_nodelay(true)
        .expect("nodelay should be settable on loopback");
    for byte in bytes {
        stream
            .write_all(&[*byte])
            .expect("each byte should be writable");
        stream.flush().expect("each byte should flush");
        thread::sleep(Duration::from_millis(1));
    }
    // Dropping the stream closes the connection, completing the job.
}

fn wait_for_first_job(web_port: u16) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let metadata = render_metadata(web_port);
        let has_sheets = metadata["sheets"]
            .as_array()
            .is_some_and(|sheets| !sheets.is_empty());
        if has_sheets {
            return metadata;
        }
        assert!(
            Instant::now() < deadline,
            "the captured job did not become visible"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn render_metadata(web_port: u16) -> serde_json::Value {
    let response = http_get_bytes(web_port, "/api/render");
    serde_json::from_slice(response_body(&response)).expect("the metadata response should be JSON")
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
            panic!("serve command exited early with {status}");
        }
        assert!(
            Instant::now() < deadline,
            "serve command did not listen on port {port}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

fn http_get_bytes(port: u16, path: &str) -> Vec<u8> {
    // Retry transient loopback failures (see `send_raw_job`) rather than fail on
    // the first refused connection or truncated read.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match http_get_once(port, path) {
            Ok(response) => return response,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "the web server never answered GET {path}: {error}"
                );
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

fn http_get_once(port: u16, path: &str) -> std::io::Result<Vec<u8>> {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(response)
}

fn response_body(response: &[u8]) -> &[u8] {
    let boundary = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("the HTTP response should contain a header boundary");
    &response[boundary + 4..]
}

fn stop(child: &mut Child) {
    child.kill().expect("the serve command should be stoppable");
    child.wait().expect("the serve command should be reapable");
}
