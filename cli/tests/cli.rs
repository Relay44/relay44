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
fn exit_code_auth_when_not_logged_in() {
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

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["orders", "list"])
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2)); // ExitCode::Auth
}

#[test]
fn exit_code_config_for_missing_profile() {
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

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["--profile", "nonexistent", "markets", "list"])
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(6)); // ExitCode::Config
}

#[test]
fn typo_suggestion_in_stderr() {
    let out = r44().arg("markts").output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("markets"), "expected 'markets' suggestion in: {stderr}");
}

#[test]
fn completions_zsh() {
    let out = r44().args(["completions", "zsh"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("r44"));
}

#[test]
fn completions_fish() {
    let out = r44().args(["completions", "fish"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("r44"));
}

#[test]
fn profile_show_json() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "default",
          "profiles": {
            "default": {
              "api_url": "https://relay44-api.onrender.com/v1",
              "access_token": "tok-secret"
            }
          }
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["--output", "json", "profile", "show"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"active\": true"));
    assert!(stdout.contains("\"authenticated\": true"));
    assert!(!stdout.contains("tok-secret"), "token must not leak into show output");
}

#[test]
fn profile_use_switches_active() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "default",
          "profiles": {
            "default": { "api_url": "https://default.example.com/v1" },
            "staging": { "api_url": "https://staging.example.com/v1" }
          }
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["profile", "use", "staging"])
        .output()
        .unwrap();

    assert!(out.status.success());

    let verify = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["--output", "json", "profile", "list"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&verify.stdout);
    assert!(stdout.contains("\"active\": \"staging\""));
}

#[test]
fn profile_use_nonexistent_fails() {
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

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["profile", "use", "ghost"])
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(6));
}

#[test]
fn config_show_json() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "default",
          "profiles": {
            "default": { "api_url": "https://relay44-api.onrender.com/v1" }
          },
          "workflows": {
            "check": { "steps": ["markets list"] }
          },
          "hooks": [
            { "command": "orders place", "run": "echo hook", "stage": "pre", "enabled": true }
          ]
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["--output", "json", "config", "show"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"workflows\": 1"));
    assert!(stdout.contains("\"hooks\": 1"));
}

#[test]
fn config_set_url_persists() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "default",
          "profiles": {
            "default": { "api_url": "https://old.example.com/v1" }
          }
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["config", "set-url", "https://new.example.com/v1"])
        .output()
        .unwrap();

    assert!(out.status.success());

    let verify = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["--output", "json", "config", "show"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&verify.stdout);
    assert!(stdout.contains("https://new.example.com/v1"));
}

#[test]
fn config_reset_with_yes_flag() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "custom",
          "profiles": {
            "custom": { "api_url": "https://custom.example.com/v1" }
          },
          "workflows": { "w1": { "steps": ["markets list"] } }
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["config", "reset", "--yes"])
        .output()
        .unwrap();

    assert!(out.status.success());

    let verify = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["--output", "json", "config", "show"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&verify.stdout);
    assert!(stdout.contains("\"workflows\": 0"));
}

#[test]
fn config_path_prints_path() {
    let out = r44().args(["config", "path"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("config.json"));
}

#[test]
fn workflow_list_shows_workflows() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "default",
          "profiles": {
            "default": { "api_url": "https://relay44-api.onrender.com/v1" }
          },
          "workflows": {
            "snapshot": {
              "description": "market snapshot",
              "steps": ["markets list --limit 5", "leaderboard top"]
            }
          }
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["--output", "json", "workflow", "list"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("snapshot"));
    assert!(stdout.contains("market snapshot"));
}

#[test]
fn workflow_run_dry_run() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "default",
          "profiles": {
            "default": { "api_url": "https://relay44-api.onrender.com/v1" }
          },
          "workflows": {
            "peek": {
              "steps": ["markets get {{1}}"]
            }
          }
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["workflow", "run", "peek", "--dry-run", "market-42"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("market-42"));
}

#[test]
fn workflow_validate_rejects_nested_workflow() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "active_profile": "default",
          "profiles": {
            "default": { "api_url": "https://relay44-api.onrender.com/v1" }
          },
          "workflows": {
            "bad": {
              "steps": ["workflow run bad"]
            }
          }
        }"#,
    );

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .args(["workflow", "validate", "bad"])
        .output()
        .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("nested"), "expected nested workflow error in: {stderr}");
}

#[test]
fn env_var_output_format_override() {
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

    let out = r44()
        .env("HOME", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .env("R44_OUTPUT", "json")
        .args(["config", "show"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"activeProfile\""), "R44_OUTPUT=json should produce JSON output");
}

#[test]
fn quiet_flag_suppresses_output() {
    let out = r44().args(["--quiet", "--help"]).output().unwrap();
    assert!(out.status.success());
}

#[test]
fn subcommand_help_flags() {
    for cmd in ["markets", "orders", "positions", "agents", "wallet", "config", "profile", "workflow", "session"] {
        let out = r44().args([cmd, "--help"]).output().unwrap();
        assert!(out.status.success(), "{cmd} --help failed");
    }
}

#[test]
fn edge_scanner_help() {
    let out = r44().args(["edge-scanner", "--help"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("signals"));
    assert!(stdout.contains("curve"));
}

#[test]
fn new_commands_appear_in_help() {
    let out = r44().arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("decisions"));
    assert!(stdout.contains("leaderboard"));
    assert!(stdout.contains("activity"));
    assert!(stdout.contains("edge-scanner"));
}

#[test]
fn decisions_help() {
    let out = r44().args(["decisions", "--help"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("get"));
    assert!(stdout.contains("create"));
}

#[test]
fn leaderboard_help() {
    let out = r44().args(["leaderboard", "--help"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("top"));
    assert!(stdout.contains("rank"));
}

#[test]
fn activity_help() {
    let out = r44().args(["activity", "--help"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("list"));
}

#[test]
fn legacy_config_migration() {
    let (_dir, config_root) = config_env();
    write_config(
        &config_root,
        r#"{
          "api_url": "https://legacy.example.com/v1",
          "access_token": "tok123",
          "wallet": "wallet-1"
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
    assert!(stdout.contains("https://legacy.example.com/v1"));
}

#[test]
fn no_color_flag() {
    let out = r44().args(["--no-color", "--help"]).output().unwrap();
    assert!(out.status.success());
}

#[test]
fn timeout_flag_accepted() {
    let out = r44().args(["--timeout", "60", "--help"]).output().unwrap();
    assert!(out.status.success());
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
