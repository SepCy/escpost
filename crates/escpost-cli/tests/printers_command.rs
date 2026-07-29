use std::process::Command;

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::io::Read;
#[cfg(unix)]
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::process::Output;
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn printers_list_is_a_rust_cli_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["printers", "list", "--help"])
        .output()
        .expect("the escpost command should finish");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "command failed:\n{stdout}");
    assert!(stdout.contains("List currently usable printers"));
    assert!(stdout.contains("Usage: escpost printers list"));
    assert!(stdout.contains("--transport <TRANSPORT>"));
}

#[test]
fn printers_add_documents_its_registration_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["printers", "add", "--help"])
        .output()
        .expect("the escpost command should finish");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "command failed:\n{stdout}");
    assert!(stdout.contains("Register a printer in the local configuration"));
    assert!(stdout.contains("Usage: escpost printers add [OPTIONS] [NAME]"));
    assert!(stdout.contains("--transport <TRANSPORT>"));
    assert!(stdout.contains("usb"));
    assert!(stdout.contains("network"));
    assert!(stdout.contains("--host <HOST>"));
    assert!(stdout.contains("--port <PORT>"));
    assert!(stdout.contains("--profile <PROFILE>"));
}

#[cfg(unix)]
#[test]
fn printers_add_requires_a_terminal_for_usb_registration() {
    let directory = temporary_directory("non-interactive-usb");
    let config = directory.join("printers.toml");

    let output = run_non_interactive_add(&config, &["counter", "--transport", "usb"]);

    assert_failed_without_configuration(
        &output,
        &config,
        "USB printer registration requires an interactive terminal",
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_registers_a_network_printer_in_an_explicit_configuration() {
    let directory = temporary_directory("add-network-printer");
    let config = directory.join("printers.toml");

    let output = run_non_interactive_add(
        &config,
        &[
            "kitchen",
            "--transport",
            "network",
            "--host",
            "10.42.0.71",
            "--port",
            "9100",
            "--profile",
            "REFERENCE",
        ],
    );

    assert_command_succeeded(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Printer: kitchen"));
    assert!(stderr.contains("Transport: network"));
    assert!(
        stderr.contains(&format!("Configuration: {}", config.display())),
        "the command should report where it saved the printer:\n{stderr}"
    );
    let document = fs::read_to_string(&config).expect("the printer config should be readable");
    let table = toml::from_str::<toml::Table>(&document)
        .expect("the printer config should contain valid TOML");
    let printer = table["kitchen"]
        .as_table()
        .expect("the named printer should be a table");
    assert_eq!(printer["transport"].as_str(), Some("network"));
    assert_eq!(printer["host"].as_str(), Some("10.42.0.71"));
    assert_eq!(printer["port"].as_integer(), Some(9100));
    assert_eq!(printer["profile"].as_str(), Some("REFERENCE"));
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_preserves_existing_configuration_text() {
    let directory = temporary_directory("preserve-config");
    let config = directory.join("printers.toml");
    let existing = concat!(
        "# Keep this developer note.\n",
        "[netum-usb]\n",
        "transport = \"usb\"\n",
        "profile = \"NT-5890K\"\n",
        "vendor_id = \"0x0416\"\n",
        "product_id = \"0x5011\"\n",
        "interface_number = 0\n",
        "out_endpoint = \"0x01\"\n",
    );
    fs::write(&config, existing).expect("the existing config should be writable");

    let output = run_non_interactive_add(
        &config,
        &["kitchen", "--transport", "network", "--host", "10.42.0.71"],
    );

    assert_command_succeeded(&output);
    let updated = fs::read_to_string(&config).expect("the printer config should be readable");
    assert!(
        updated.starts_with(existing),
        "existing hand-written configuration should remain unchanged:\n{updated}"
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_refuses_to_replace_an_existing_name() {
    let directory = temporary_directory("duplicate-name");
    let config = directory.join("printers.toml");
    let existing = concat!(
        "[kitchen]\n",
        "transport = \"network\"\n",
        "host = \"10.42.0.20\"\n",
        "port = 9100\n",
    );
    fs::write(&config, existing).expect("the existing config should be writable");

    let output = run_non_interactive_add(
        &config,
        &["kitchen", "--transport", "network", "--host", "10.42.0.71"],
    );

    assert!(!output.status.success(), "duplicate names must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("already configured"),
        "the error should explain the conflict:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&config).expect("the printer config should remain readable"),
        existing,
        "a failed add must not modify existing configuration"
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_uses_the_raw_tcp_default_without_requiring_a_profile() {
    let directory = temporary_directory("network-defaults");
    let config = directory.join("printers.toml");

    let output = run_non_interactive_add(
        &config,
        &[
            "kitchen",
            "--transport",
            "network",
            "--host",
            "printer.local",
        ],
    );

    assert_command_succeeded(&output);
    let document = fs::read_to_string(&config).expect("the printer config should be readable");
    let table = toml::from_str::<toml::Table>(&document)
        .expect("the printer config should contain valid TOML");
    let printer = table["kitchen"]
        .as_table()
        .expect("the named printer should be a table");
    assert_eq!(printer["port"].as_integer(), Some(9100));
    assert!(
        !printer.contains_key("profile"),
        "a profile is optional for raw printing"
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_list_reports_saved_network_reachability_without_discovery() {
    let directory = temporary_directory("network-reachability");
    let config = directory.join("printers.toml");
    let listener =
        TcpListener::bind("127.0.0.1:0").expect("the reachable test endpoint should bind");
    let port = listener
        .local_addr()
        .expect("the test endpoint should have an address")
        .port()
        .to_string();
    let add = run_non_interactive_add(
        &config,
        &[
            "kitchen",
            "--transport",
            "network",
            "--host",
            "127.0.0.1",
            "--port",
            &port,
        ],
    );
    assert_command_succeeded(&add);

    let received = thread::spawn(move || {
        let (mut connection, _) = listener
            .accept()
            .expect("the configured endpoint should receive the probe");
        let mut bytes = Vec::new();
        connection
            .read_to_end(&mut bytes)
            .expect("the probe connection should close cleanly");
        bytes
    });
    let connected = run_printers_list(&config);
    assert_command_succeeded(&connected);
    let stdout = String::from_utf8_lossy(&connected.stdout);
    assert!(stdout.contains("] kitchen\n"));
    assert!(stdout.contains("status: connected"));
    assert!(stdout.contains("profile: unassigned"));
    assert!(stdout.contains("transport: network"));
    assert!(stdout.contains(&format!("network: 127.0.0.1:{port}")));
    assert!(
        received
            .join()
            .expect("the probe receiver should finish")
            .is_empty(),
        "reachability checks must never send printable bytes"
    );

    let unavailable = run_printers_list(&config);
    assert_command_succeeded(&unavailable);
    assert!(
        String::from_utf8_lossy(&unavailable.stdout).contains("status: unavailable"),
        "a refused connection should make the saved target unavailable:\n{}",
        String::from_utf8_lossy(&unavailable.stdout)
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_non_interactive_reports_the_first_missing_required_value() {
    let directory = temporary_directory("non-interactive-missing-value");
    let config = directory.join("printers.toml");

    let output = run_non_interactive_add(&config, &["kitchen", "--transport", "network"]);

    assert_failed_without_configuration(&output, &config, "network printer host is required");
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_rejects_port_zero_without_writing_configuration() {
    let directory = temporary_directory("invalid-network-port");
    let config = directory.join("printers.toml");

    let output = run_non_interactive_add(
        &config,
        &[
            "kitchen",
            "--transport",
            "network",
            "--host",
            "10.42.0.71",
            "--port",
            "0",
        ],
    );

    assert_failed_without_configuration(&output, &config, "port must be between 1 and 65535");
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_rejects_a_blank_network_host() {
    let directory = temporary_directory("blank-network-host");
    let config = directory.join("printers.toml");

    let output = run_non_interactive_add(
        &config,
        &["kitchen", "--transport", "network", "--host", "   "],
    );

    assert_failed_without_configuration(&output, &config, "network printer host must not be blank");
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_rejects_a_blank_printer_name() {
    let directory = temporary_directory("blank-printer-name");
    let config = directory.join("printers.toml");

    let output = run_non_interactive_add(
        &config,
        &[" ", "--transport", "network", "--host", "10.42.0.71"],
    );

    assert_failed_without_configuration(&output, &config, "printer name must not be blank");
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_rejects_a_blank_optional_profile() {
    let directory = temporary_directory("blank-printer-profile");
    let config = directory.join("printers.toml");

    let output = run_non_interactive_add(
        &config,
        &[
            "kitchen",
            "--transport",
            "network",
            "--host",
            "10.42.0.71",
            "--profile",
            " ",
        ],
    );

    assert_failed_without_configuration(&output, &config, "printer profile must not be blank");
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_refuses_to_modify_invalid_existing_configuration() {
    let directory = temporary_directory("invalid-existing-config");
    let config = directory.join("printers.toml");
    let existing = "[broken-network]\ntransport = \"network\"\n";
    fs::write(&config, existing).expect("the existing config should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "--non-interactive",
            "printers",
            "--config",
            config.to_str().expect("the config path should be UTF-8"),
            "add",
            "kitchen",
            "--transport",
            "network",
            "--host",
            "10.42.0.71",
        ])
        .output()
        .expect("the escpost command should finish");

    assert!(
        !output.status.success(),
        "invalid existing configuration must fail"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("invalid printer configuration"),
        "the error should identify the invalid config:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&config).expect("the printer config should remain readable"),
        existing,
        "a failed add must leave invalid input available for correction"
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_creates_a_private_configuration_file_and_its_directory() {
    let directory = temporary_directory("private-config");
    let config = directory.join("nested/config/printers.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "--non-interactive",
            "printers",
            "--config",
            config.to_str().expect("the config path should be UTF-8"),
            "add",
            "kitchen",
            "--transport",
            "network",
            "--host",
            "10.42.0.71",
        ])
        .output()
        .expect("the escpost command should finish");

    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mode = fs::metadata(&config)
        .expect("the printer config should have metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "new user configuration should be private");
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_accepts_a_configuration_filename_in_the_current_directory() {
    let directory = temporary_directory("relative-config");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "--non-interactive",
            "printers",
            "--config",
            "printers.toml",
            "add",
            "kitchen",
            "--transport",
            "network",
            "--host",
            "10.42.0.71",
        ])
        .current_dir(&directory)
        .output()
        .expect("the escpost command should finish");

    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        directory.join("printers.toml").is_file(),
        "the relative config should be created in the working directory"
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_uses_the_platform_configuration_directory() {
    let directory = temporary_directory("add-platform-config");
    let expected = directory.join("escpost/printers.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "--non-interactive",
            "printers",
            "add",
            "kitchen",
            "--transport",
            "network",
            "--host",
            "10.42.0.71",
        ])
        .env_remove("ESCPOST_CONFIG_DIR")
        .env("XDG_CONFIG_HOME", &directory)
        .output()
        .expect("the escpost command should finish");

    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        expected.is_file(),
        "the printer should be stored in the platform config directory"
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_add_does_not_prompt_without_a_terminal() {
    let directory = temporary_directory("no-terminal");
    let config = directory.join("printers.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args([
            "printers",
            "--config",
            config.to_str().expect("the config path should be UTF-8"),
            "add",
        ])
        .output()
        .expect("the escpost command should finish instead of waiting for input");

    assert!(!output.status.success(), "missing values must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("printer name is required"),
        "the error should explain why prompting was unavailable:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !config.exists(),
        "a failed command must not create configuration"
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_list_reads_an_explicit_configuration_path() {
    let directory = temporary_directory("explicit-config");
    let config = directory.join("printers.toml");
    let ignored_directory = directory.join("ignored");
    fs::create_dir(&ignored_directory).expect("the override directory should be creatable");
    fs::write(ignored_directory.join("printers.toml"), "also not TOML")
        .expect("the ignored config should be writable");
    fs::write(&config, "this is not TOML").expect("the invalid config should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["printers", "list", "--config"])
        .arg(&config)
        .env("ESCPOST_CONFIG_DIR", ignored_directory)
        .output()
        .expect("the escpost command should finish");

    assert!(!output.status.success(), "invalid configuration must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(config.to_string_lossy().as_ref()),
        "the explicit path should take precedence:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_list_uses_the_escpost_configuration_directory() {
    let directory = temporary_directory("environment-config");
    let config = directory.join("printers.toml");
    fs::write(&config, "this is not TOML").expect("the invalid config should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["printers", "list"])
        .env("ESCPOST_CONFIG_DIR", &directory)
        .output()
        .expect("the escpost command should finish");

    assert!(!output.status.success(), "invalid configuration must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(config.to_string_lossy().as_ref()),
        "the error should identify the resolved config:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_list_uses_the_platform_configuration_directory() {
    let directory = temporary_directory("platform-config");
    let config_directory = directory.join("escpost");
    fs::create_dir(&config_directory).expect("the app config directory should be creatable");
    let config = config_directory.join("printers.toml");
    fs::write(&config, "this is not TOML").expect("the invalid config should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["printers", "list"])
        .env_remove("ESCPOST_CONFIG_DIR")
        .env("XDG_CONFIG_HOME", &directory)
        .output()
        .expect("the escpost command should finish");

    assert!(!output.status.success(), "invalid configuration must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(config.to_string_lossy().as_ref()),
        "the error should identify the platform config:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn printers_list_does_not_create_missing_configuration() {
    let directory = temporary_directory("read-only-config");
    let application_directory = directory.join("escpost");

    let _output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["printers", "list"])
        .env_remove("ESCPOST_CONFIG_DIR")
        .env("XDG_CONFIG_HOME", &directory)
        .output()
        .expect("the escpost command should finish");

    assert!(
        !application_directory.exists(),
        "read-only listing must not create its config directory"
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn development_wrapper_creates_the_checkout_local_config_source() {
    let directory = temporary_directory("wrapper-config");
    let executable_directory = directory.join("bin");
    fs::create_dir(&executable_directory).expect("the fake PATH should be creatable");
    let repository = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::copy(repository.join("escpost"), directory.join("escpost"))
        .expect("the development wrapper should be copyable");
    write_executable(
        &executable_directory.join("docker"),
        "#!/bin/sh\nprintf 'docker: %s\\n' \"$*\"\n",
    );

    let output = Command::new(directory.join("escpost"))
        .args(["printers", "list"])
        .env_remove("ESCPOST_CONFIG_HOST_DIR")
        .env(
            "PATH",
            format!(
                "{}:/usr/bin:/bin",
                executable_directory
                    .to_str()
                    .expect("the test path should be UTF-8")
            ),
        )
        .output()
        .expect("the development wrapper should finish");

    assert!(
        output.status.success(),
        "wrapper failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("local/config").is_dir());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "docker: compose run --rm cli printers list\n"
    );
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn development_wrapper_routes_printers_list_without_python() {
    let directory = temporary_directory("wrapper-route");
    let target_directory = directory.join("target");
    fs::create_dir_all(target_directory.join("debug"))
        .expect("the fake target directory should be creatable");
    write_executable(&directory.join("cargo"), "#!/bin/sh\nexit 0\n");
    write_executable(
        &directory.join("maturin"),
        "#!/bin/sh\necho legacy-python-path >&2\nexit 99\n",
    );
    write_executable(
        &target_directory.join("debug/escpost"),
        "#!/bin/sh\nprintf 'rust-command: %s\\n' \"$*\"\n",
    );
    let repository = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let output = Command::new("sh")
        .arg(repository.join("scripts/cli-entrypoint"))
        .args(["printers", "list", "--help"])
        .env(
            "PATH",
            format!(
                "{}:/usr/bin:/bin",
                directory.to_str().expect("the test path should be UTF-8")
            ),
        )
        .env("CARGO_TARGET_DIR", &target_directory)
        .current_dir(repository)
        .output()
        .expect("the development wrapper should finish");

    assert!(
        output.status.success(),
        "wrapper failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "rust-command: printers list --help\n"
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("legacy-python-path"));
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
#[test]
fn development_wrapper_routes_printers_add_with_a_config_option_without_python() {
    let directory = temporary_directory("wrapper-add-route");
    let target_directory = directory.join("target");
    fs::create_dir_all(target_directory.join("debug"))
        .expect("the fake target directory should be creatable");
    write_executable(&directory.join("cargo"), "#!/bin/sh\nexit 0\n");
    write_executable(
        &directory.join("maturin"),
        "#!/bin/sh\necho legacy-python-path >&2\nexit 99\n",
    );
    write_executable(
        &target_directory.join("debug/escpost"),
        "#!/bin/sh\nprintf 'rust-command: %s\\n' \"$*\"\n",
    );
    let repository = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let output = Command::new("sh")
        .arg(repository.join("scripts/cli-entrypoint"))
        .args([
            "printers",
            "--config",
            "/tmp/printers.toml",
            "add",
            "--help",
        ])
        .env(
            "PATH",
            format!(
                "{}:/usr/bin:/bin",
                directory.to_str().expect("the test path should be UTF-8")
            ),
        )
        .env("CARGO_TARGET_DIR", &target_directory)
        .current_dir(repository)
        .output()
        .expect("the development wrapper should finish");

    assert!(
        output.status.success(),
        "wrapper failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "rust-command: printers --config /tmp/printers.toml add --help\n"
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("legacy-python-path"));
    fs::remove_dir_all(directory).expect("the test directory should be removable");
}

#[cfg(unix)]
fn run_non_interactive_add(config: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["--non-interactive", "printers", "--config"])
        .arg(config)
        .arg("add")
        .args(arguments)
        .output()
        .expect("the escpost command should finish")
}

#[cfg(unix)]
fn run_printers_list(config: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["printers", "--config"])
        .arg(config)
        .args(["list", "--transport", "network"])
        .output()
        .expect("the escpost command should finish")
}

#[cfg(unix)]
fn assert_command_succeeded(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn assert_failed_without_configuration(output: &Output, config: &Path, message: &str) {
    assert!(!output.status.success(), "an invalid command must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "the error should explain the invalid value:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !config.exists(),
        "an invalid command must not create configuration"
    );
}

#[cfg(unix)]
fn write_executable(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("the fake executable should be writable");
    let mut permissions = fs::metadata(path)
        .expect("the fake executable should have metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("the fake executable should be executable");
}

#[cfg(unix)]
fn temporary_directory(case: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock should be after the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "escpost-printers-{case}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("the test directory should be creatable");
    path
}
