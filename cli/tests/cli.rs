use std::process::Command;

fn r44() -> Command {
    Command::new(env!("CARGO_BIN_EXE_r44"))
}

#[test]
fn help_exits_zero() {
    let out = r44().arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("prediction markets"));
    assert!(stdout.contains("markets"));
    assert!(stdout.contains("orders"));
    assert!(stdout.contains("agents"));
}

#[test]
fn version_flag() {
    let out = r44().arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("r44"));
}

#[test]
fn unknown_command_fails() {
    let out = r44().arg("nonexistent").output().unwrap();
    assert!(!out.status.success());
}

#[test]
fn completions_bash() {
    let out = r44().args(["completions", "bash"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("r44"));
}

#[test]
fn json_output_flag() {
    let out = r44()
        .args(["--output", "json", "config", "path"])
        .output()
        .unwrap();
    assert!(out.status.success());
}
