use std::process::Command;

#[test]
fn profiles_find_non_interactive_points_at_list_search() {
    let output = Command::new(env!("CARGO_BIN_EXE_escpost"))
        .args(["--non-interactive", "profiles", "find"])
        .output()
        .expect("the escpost command should finish");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "non-interactive find should fail:\n{stderr}"
    );
    assert!(
        stderr.contains("profiles list --search"),
        "missing guidance toward profiles list --search:\n{stderr}"
    );
}
