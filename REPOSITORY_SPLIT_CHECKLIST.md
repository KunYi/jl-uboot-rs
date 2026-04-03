# Repository Split Checklist

This workspace is currently developed inside the SDK tree:

- `host-tools/jl-uboot-rs`

The goal of this checklist is to define what must be true before splitting it
into a standalone repository.

## Required repository contents

- `Cargo.toml`
- `Cargo.lock`
- `LICENSE`
- `.gitignore`
- `.github/workflows/ci.yml`
- `README.md`
- `USAGE.md`
- `PROTOCOL_COVERAGE.md`
- `TESTING_WITHOUT_HARDWARE.md`
- `NO_HARDWARE_TEST_MATRIX.md`
- `MOCK_TEST_EXPANSION_PLAN.md`
- `REAL_DEVICE_TEST_PLAN.md`
- `WINDOWS_PORT_PLAN.md`
- `TODOs.md`

## Code boundaries

- `crates/jl-sg`
  - transport-only
  - Linux `SG_IO`
  - Windows transport/discovery
- `crates/jl-msc`
  - MSC/SCSI framing helpers
- `crates/jl-uboot`
  - protocol-only
  - no OS-specific discovery logic
- `crates/jl-device-db`
  - optional target metadata
- `apps/jluboot`
  - flasher/query CLI
- `apps/jlrunner`
  - RAM loader CLI

## Pre-split checks

- `cargo fmt --check`
- `cargo check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

## Known pre-release limitations

- Windows transport/discovery has code and no-hardware coverage, but still needs
  real-hardware validation.
- Real JieLi hardware validation is still required for:
  - `LoaderV2`
  - `LoaderV1`
  - `UBOOT1`
- Package helpers such as `bfu`, `JLFS`, and keyfile tooling are still outside
  this repository scope.

## Recommended first standalone-repo tasks

1. Add issue templates or a minimal bug-report guide.
2. Add release builds for:
   - Linux
   - Windows
3. Add real-device test logs and compatibility notes by chip family.
