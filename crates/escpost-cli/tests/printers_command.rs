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
