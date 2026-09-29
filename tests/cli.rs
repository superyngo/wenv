//! CLI smoke tests against the built `wenv` binary

use predicates::prelude::*;

#[test]
fn test_cli_help_lists_session_flags() {
    assert_cmd::cargo::cargo_bin_cmd!("wenv")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--lang").and(predicate::str::contains("--no-color")));
}

#[test]
fn test_cli_version_prints_package_version() {
    assert_cmd::cargo::cargo_bin_cmd!("wenv")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_cli_source_without_tty_fails_with_guidance() {
    let dir = tempfile::tempdir().unwrap();
    assert_cmd::cargo::cargo_bin_cmd!("wenv")
        .args(["--shell", "bash", "--config"])
        .arg(dir.path().join("config.toml"))
        .arg(".")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires a terminal"));
}
