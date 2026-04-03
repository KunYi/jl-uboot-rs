# Project Status

This file tracks the current implementation status of `jl-uboot-rs`.

## Summary

- Linux path is the primary usable path today
- Windows transport/discovery structure is implemented, but still requires
  real-hardware validation
- protocol coverage is broad enough for experimental use
- remaining uncertainty is mostly validation, not missing crate structure

## Current implementation status

### Command surface

- primary command coverage is implemented
- chunked flash/memory transfer path is implemented for loader tolerance and
  large payloads
- JSON output is implemented for query/action commands that are useful in
  automation
- destructive operations require `--yes`
- progress output is available on stderr with `--progress`

### Protocol coverage

- `LoaderV2`
  - implemented
- `LoaderV1`
  - implemented
- `UBOOT1`
  - implemented

See also:

- [`PROTOCOL_COVERAGE.md`](./PROTOCOL_COVERAGE.md)

### Test coverage

- protocol-layer mock tests cover:
  - `LoaderV2`
  - `LoaderV1`
  - `UBOOT1`
  - command mismatch and transport error propagation
- CLI integration tests cover:
  - `--yes`
  - `--json` error behavior
  - `find`
  - local file-not-found and device-not-found exit codes
- library-level fake-device tests cover successful CLI execution paths for:
  - `probe --json`
  - `flash-read --hexdump`
  - `mem-read --stdout`
  - `--progress` on `stderr`
  - transport-failure exit code mapping
  - `jlrunner` successful RAM load + jump path
  - `jlrunner` transport-failure exit code mapping

Static checks currently pass:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo check`
- `cargo test`

See also:

- [`TESTING_WITHOUT_HARDWARE.md`](./TESTING_WITHOUT_HARDWARE.md)
- [`NO_HARDWARE_TEST_MATRIX.md`](./NO_HARDWARE_TEST_MATRIX.md)
- [`REAL_DEVICE_TEST_PLAN.md`](./REAL_DEVICE_TEST_PLAN.md)

## Platform status

### Linux

- Linux `SG_IO` transport is implemented
- Linux block selectors such as `/dev/sdX` can be resolved internally to the
  matching `/dev/sgX`
- current remaining risk is real-target validation rather than missing transport
  structure

### Windows

- `WindowsScsiDevice` exists
- Win32 `CreateFileW` / `DeviceIoControl` /
  `SCSI_PASS_THROUGH_DIRECT` path is implemented
- `SetupAPI`-based USB MSC candidate enumeration and early VID filtering are
  implemented
- Windows discovery attempts to back-fill visible selectors such as `E:` / `X:`
  by correlating volume device numbers to disk-interface candidates
- Windows discovery/transport still require real-hardware validation before
  they should be considered usable

See also:

- [`WINDOWS_PORT_PLAN.md`](./WINDOWS_PORT_PLAN.md)

## Main open risks

- protocol validation on real hardware
- Windows selector and transport behavior on actual JieLi MSC targets
- behavior differences across ROM/loader revisions

## Scope boundaries

Still out of scope:

- `bfu`
- `JLFS`
- keyfile tooling
- official-tool parity claims
- GUI tooling
