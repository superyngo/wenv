//! Integration tests for Bash functionality

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_list_command() {
    let dir = tempdir().unwrap();
    let rc_file = dir.path().join(".bashrc");
    fs::write(
        &rc_file,
        "alias ll='ls -la'\nalias gs='git status'\nexport EDITOR=nvim\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args(["--file", rc_file.to_str().unwrap(), "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ll"))
        .stdout(predicate::str::contains("gs"))
        .stdout(predicate::str::contains("EDITOR"));
}

#[test]
fn test_list_alias_only() {
    let dir = tempdir().unwrap();
    let rc_file = dir.path().join(".bashrc");
    fs::write(
        &rc_file,
        "alias ll='ls -la'\nexport EDITOR=nvim\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args(["--file", rc_file.to_str().unwrap(), "list", "alias"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ll"))
        .stdout(predicate::str::contains("EDITOR").not());
}

#[test]
fn test_check_no_issues() {
    let dir = tempdir().unwrap();
    let rc_file = dir.path().join(".bashrc");
    fs::write(
        &rc_file,
        "alias ll='ls -la'\nalias gs='git status'\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args(["--file", rc_file.to_str().unwrap(), "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
fn test_check_duplicate() {
    let dir = tempdir().unwrap();
    let rc_file = dir.path().join(".bashrc");
    fs::write(
        &rc_file,
        "alias ll='ls -la'\nalias ll='ls -l'\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args(["--file", rc_file.to_str().unwrap(), "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Duplicate"));
}

#[test]
fn test_export_command() {
    let dir = tempdir().unwrap();
    let rc_file = dir.path().join(".bashrc");
    let export_file = dir.path().join("exported.sh");

    fs::write(
        &rc_file,
        "alias ll='ls -la'\nexport EDITOR=nvim\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args([
        "--file",
        rc_file.to_str().unwrap(),
        "export",
        "-o",
        export_file.to_str().unwrap(),
    ])
    .assert()
    .success();

    assert!(export_file.exists());
    let content = fs::read_to_string(&export_file).unwrap();
    assert!(content.contains("ll"));
    assert!(content.contains("EDITOR"));
}

#[test]
fn test_format_dry_run() {
    let dir = tempdir().unwrap();
    let rc_file = dir.path().join(".bashrc");
    let original = "alias gs='git status'\nalias ll='ls -la'\nexport EDITOR=nvim\n";
    fs::write(&rc_file, original).unwrap();

    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args(["--file", rc_file.to_str().unwrap(), "format", "--dry-run"])
        .assert()
        .success();

    // File should not be modified in dry-run mode
    let content = fs::read_to_string(&rc_file).unwrap();
    assert_eq!(content, original);
}

#[test]
fn test_backup_list_empty() {
    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args(["backup", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No backups found").or(predicate::str::contains("backup")));
}

#[test]
fn test_help() {
    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Shell"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("check"))
        .stdout(predicate::str::contains("add"));
}

#[test]
fn test_version() {
    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("srcman"));
}
