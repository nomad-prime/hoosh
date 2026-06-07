use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn hoosh_bin() -> String {
    env!("CARGO_BIN_EXE_hoosh").to_string()
}

/// Write a minimal config so hoosh doesn't enter the setup wizard on CI.
/// Returns the config path to pass via --config.
fn write_min_config(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("hoosh-config.toml");
    fs::write(&path, "default_backend = \"mock\"\n[backends.mock]\n").unwrap();
    path
}

#[test]
fn resume_without_storage_errors() {
    let dir = TempDir::new().unwrap();
    let cfg = write_min_config(&dir);
    let output = Command::new(hoosh_bin())
        .current_dir(dir.path())
        .args([
            "--config",
            cfg.to_str().unwrap(),
            "--mode",
            "tagged",
            "--resume",
            "conv_20260101_000000",
            "--skip-permissions",
            "hi",
        ])
        .output()
        .expect("failed to run hoosh");

    assert!(!output.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("conversation_storage"),
        "stderr should mention conversation_storage; got: {stderr}"
    );
}

#[test]
fn resume_unknown_id_errors() {
    let dir = TempDir::new().unwrap();
    let cfg = write_min_config(&dir);
    fs::create_dir_all(dir.path().join(".hoosh")).unwrap();
    fs::write(
        dir.path().join(".hoosh/config.toml"),
        "conversation_storage = true\n",
    )
    .unwrap();

    let output = Command::new(hoosh_bin())
        .current_dir(dir.path())
        .args([
            "--config",
            cfg.to_str().unwrap(),
            "--mode",
            "tagged",
            "--resume",
            "conv_does_not_exist",
            "--skip-permissions",
            "hi",
        ])
        .output()
        .expect("failed to run hoosh");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No conversation found"),
        "stderr should report missing conversation; got: {stderr}"
    );
}

#[test]
fn resume_and_continue_conflict() {
    let dir = TempDir::new().unwrap();
    let output = Command::new(hoosh_bin())
        .current_dir(dir.path())
        .args(["--mode", "tagged", "--resume", "x", "--continue", "hi"])
        .output()
        .expect("failed to run hoosh");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used") || stderr.contains("conflicts"),
        "stderr should indicate flag conflict; got: {stderr}"
    );
}

#[test]
fn help_lists_new_flags() {
    let output = Command::new(hoosh_bin())
        .arg("--help")
        .output()
        .expect("failed to run hoosh --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--output-format"));
    assert!(stdout.contains("--resume"));
}
