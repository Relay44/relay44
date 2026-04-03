use std::process::Command;

use tempfile::TempDir;

fn r44() -> Command {
    Command::new(env!("CARGO_BIN_EXE_r44"))
}

fn config_env() -> (TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().display().to_string();
    (dir, path)
}

fn write_config(root: &str, contents: &str) {
    let config_dir = config_dir(root);
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.json"), contents).unwrap();
}

fn config_dir(root: &str) -> std::path::PathBuf {
    let home = std::path::Path::new(root);
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/r44")
    }
    #[cfg(not(target_os = "macos"))]
    {
        home.join(".config/r44")
    }
}

#[test]
fn help_exits_zero() {
    let out = r44().arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("prediction markets"));
    assert!(stdout.contains("profile"));
    assert!(stdout.contains("workflow"));
    assert!(stdout.contains("session"));
    assert!(stdout.contains("doctor"));
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
fn profile_list_uses_config_profiles() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "prod",
          "profiles": {
            "default": { "api_url": "https://default.example.com/v1" },
            "prod": {
              "api_url": "https://prod.example.com/v1",
              "wallet": "wallet-1",
              "output": "json"
            }
          }
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["--output", "json", "profile", "list"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"active\": \"prod\""));
    assert!(stdout.contains("\"name\": \"prod\""));
    assert!(stdout.contains("https://prod.example.com/v1"));
}

#[test]
fn workflow_validate_accepts_placeholder_steps() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "default",
          "profiles": {
            "default": { "api_url": "https://relay44-api.onrender.com/v1" }
          },
          "workflows": {
            "market-check": {
              "steps": ["markets get {{1}}", "markets trades {{1}} --limit 5"]
            }
          }
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["workflow", "validate", "market-check"])
        .output()
        .unwrap();

    assert!(out.status.success(), "{:?}", out);
}

#[test]
fn session_export_reads_jsonl() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "default",
          "profiles": {
            "default": { "api_url": "https://relay44-api.onrender.com/v1" }
          }
        }"#,
    );

    let session_path = config_dir(&config_root).join("sessions.jsonl");
    std::fs::write(
        &session_path,
        "{\"timestamp\":\"2026-04-03T00:00:00Z\",\"profile\":\"default\",\"command\":\"markets list\",\"exit_status\":0,\"duration_ms\":15}\n",
    )
    .unwrap();

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["--output", "json", "session", "export", "--limit", "1"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"command\": \"markets list\""));
}
