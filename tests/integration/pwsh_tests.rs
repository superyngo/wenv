//! Integration tests for PowerShell functionality

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_list_powershell() {
    let dir = tempdir().unwrap();
    let profile = dir.path().join("profile.ps1");
    fs::write(
        &profile,
        "Set-Alias ll Get-ChildItem\n$env:EDITOR = \"code\"\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args([
        "--file",
        profile.to_str().unwrap(),
        "--shell",
        "pwsh",
        "list",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("ll"))
    .stdout(predicate::str::contains("EDITOR"));
}

#[test]
fn test_check_powershell_no_issues() {
    let dir = tempdir().unwrap();
    let profile = dir.path().join("profile.ps1");
    fs::write(
        &profile,
        "Set-Alias ll Get-ChildItem\nSet-Alias gs git-status\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args([
        "--file",
        profile.to_str().unwrap(),
        "--shell",
        "pwsh",
        "check",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("No issues found"));
}

#[test]
fn test_export_powershell() {
    let dir = tempdir().unwrap();
    let profile = dir.path().join("profile.ps1");
    let export_file = dir.path().join("exported.ps1");

    fs::write(
        &profile,
        "Set-Alias ll Get-ChildItem\n$env:EDITOR = \"code\"\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("srcman").unwrap();
    cmd.args([
        "--file",
        profile.to_str().unwrap(),
        "--shell",
        "pwsh",
        "export",
        "-o",
        export_file.to_str().unwrap(),
    ])
    .assert()
    .success();

    assert!(export_file.exists());
    let content = fs::read_to_string(&export_file).unwrap();
    assert!(content.contains("ll") || content.contains("Set-Alias"));
}
