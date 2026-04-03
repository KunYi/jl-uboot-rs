use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn destructive_command_requires_yes() {
    let mut cmd = Command::cargo_bin("jluboot").expect("binary");
    cmd.args(["flash-erase-chip", "--device", "/definitely/not/present"]);
    cmd.assert()
        .code(14)
        .stderr(predicate::str::contains("pass --yes to continue"));
}

#[test]
fn destructive_command_with_yes_reaches_device_lookup() {
    let mut cmd = Command::cargo_bin("jluboot").expect("binary");
    cmd.args([
        "--yes",
        "flash-erase-chip",
        "--device",
        "/definitely/not/present",
    ]);
    cmd.assert()
        .code(11)
        .stderr(predicate::str::contains("device path not found"));
}

#[test]
fn json_mode_does_not_emit_stdout_on_device_error() {
    let mut cmd = Command::cargo_bin("jluboot").expect("binary");
    cmd.args(["--json", "read-id", "--device", "/definitely/not/present"]);
    cmd.assert()
        .code(11)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("device path not found"));
}

#[test]
fn find_command_succeeds_without_hardware() {
    let mut cmd = Command::cargo_bin("jluboot").expect("binary");
    cmd.arg("find");
    cmd.assert().code(0);
}

#[test]
fn find_json_command_succeeds_without_hardware() {
    let mut cmd = Command::cargo_bin("jluboot").expect("binary");
    cmd.args(["--json", "find"]);
    cmd.assert()
        .code(0)
        .stdout(predicate::str::contains("[").and(predicate::str::contains("]")));
}
