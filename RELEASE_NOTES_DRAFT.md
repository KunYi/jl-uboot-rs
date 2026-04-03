# Release Notes Draft

## Unreleased / v0.1-draft

This is the first draft release boundary for `jl-uboot-rs`.

### Included

- Rust workspace split into:
  - `jl-sg`
  - `jl-msc`
  - `jl-uboot`
  - `jl-device-db`
  - `jluboot`
  - `jlrunner`
- Linux `SG_IO` transport
- Windows transport/discovery implementation scaffold with:
  - `CreateFileW`
  - `DeviceIoControl`
  - `SCSI_PASS_THROUGH_DIRECT`
  - `SetupAPI` USB MSC candidate enumeration
  - early VID filtering
  - visible-selector correlation attempt
- protocol support:
  - `LoaderV2`
  - `LoaderV1`
  - `UBOOT1`
- CLI support for:
  - discovery
  - probing
  - query commands
  - flash read/write/erase
  - memory read/write/jump
  - chip-key commands
- no-hardware testing:
  - protocol mock tests
  - CLI error-path tests
  - fake-device success-path tests

### Behavior notes

- `--device` is treated as a user-facing selector
- without `--device`, the tool attempts automatic detection
- Linux block selectors such as `/dev/sdX` can be resolved internally to the
  matching `/dev/sgX`
- Windows intends to use visible selectors such as `E:` or `X:`

### Not included

- keyfile generation/parsing
- `bfu` or `JLFS` helpers
- official tooling compatibility guarantees
- real-hardware validation logs

### Major risk still open

- Windows behavior is implemented structurally, but not yet validated on real
  JieLi MSC download-mode hardware

### Validation status

- `cargo fmt --check`
- `cargo check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

All passing in the current workspace state.
