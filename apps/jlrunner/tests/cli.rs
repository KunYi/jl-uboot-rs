use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write as _;
use tempfile::NamedTempFile;

#[test]
fn missing_input_file_returns_io_exit_code() {
    let mut cmd = Command::cargo_bin("jlrunner").expect("binary");
    cmd.args([
        "--device",
        "/definitely/not/present",
        "--address",
        "0",
        "--file",
        "/definitely/not/present.bin",
    ]);
    cmd.assert()
        .code(10)
        .stderr(predicate::str::contains("No such file").or(predicate::str::contains("not found")));
}

#[test]
fn existing_file_but_missing_device_returns_device_not_found() {
    let mut file = NamedTempFile::new().expect("temp file");
    file.write_all(&[0x11, 0x22, 0x33, 0x44]).expect("write");

    let mut cmd = Command::cargo_bin("jlrunner").expect("binary");
    cmd.args([
        "--device",
        "/definitely/not/present",
        "--address",
        "0",
        "--file",
        file.path().to_str().expect("path"),
    ]);
    cmd.assert()
        .code(11)
        .stderr(predicate::str::contains("device path not found"));
}
