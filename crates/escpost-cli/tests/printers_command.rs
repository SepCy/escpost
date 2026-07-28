use std::process::Command;

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
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
