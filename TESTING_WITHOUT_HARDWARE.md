# Testing Without Hardware

This document defines what can be verified before a real JieLi device is available.

## Goals

- Keep the Rust workspace buildable
- Keep CLI behavior stable
- Keep protocol encoding logic reviewable
- Reduce the amount of debugging that must happen only on first hardware contact

## What can be tested now

### 1. Build integrity

Run:

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

These should stay green.

Current status:

- implemented
- included in routine validation

### 2. CLI surface stability

Verify help output manually:

```bash
cargo run -p jluboot -- --help
cargo run -p jlrunner -- --help
```

Check:

- all subcommands are listed
- `--protocol` exists where expected
- destructive commands are clearly named

Current status:

- partly covered by integration tests
- still worth checking manually after major CLI refactors

### 3. Protocol constant review

Cross-check Rust command IDs against upstream Python:

- `jl-uboot-tool/jltech/uboot.py`
- local Rust:
  - `crates/jl-uboot/src/lib.rs`

Focus on:

- `LoaderV2`
  - `0xFB00/01/02/04/06/08`
  - `0xFC09/0A/0B/0C/14/15/16`
  - `0xFD05/07`
- `LoaderV1`
  - `0xFB00/01/02/03/04/09`
  - `0xFC00/0B`
  - `0xFD01`
- `UBOOT1`
  - `0xFB06/08`
  - `0xFD07`

Current status:

- now partially backed by protocol-layer mock tests in `crates/jl-uboot`
- still requires human review when upstream notes or command maps change

### 4. Data-shape review

Review assumptions that are encoded in Rust:

- response prefix handling for `cmd_exec`
- `LoaderV2 GET_ONLINE_DEVICE`
- `LoaderV2 READ_ID`
- `LoaderV2 GET_USB_BUFF_SIZE`
- `LoaderV2 GET_LOADER_VER`
- `LoaderV2 GET_MASKROM_ID`
- `chipkey` decode path

This is not proof, but it catches obvious mismatches before hardware is connected.

Current status:

- partially covered by mock tests for:
  - `probe_info`
  - `flash_crc16`
  - `flash_crc16_raw`
  - `read_status`
  - `set_flash_cmds`
  - `run_app`
  - `write_chipkey`
  - `flash_select`
  - `mem_jump`
  - transport error propagation

### 5. Read-output behavior

Review these code paths:

- `--output`
- `--stdout`
- `--hexdump`
- default hexdump fallback

Files:

- `apps/jluboot/src/main.rs`

Current status:

- basic error-path JSON/stdout separation is covered by CLI tests
- successful `stdout` and `hexdump` paths are covered by fake-device success-path tests
- `find` JSON output shape is now also covered for candidate and probe metadata

### 6. Static review of unsupported combinations

Before hardware exists, make sure unsupported paths fail clearly rather than silently:

- `LoaderV1` querying `usb-buffer-size`
- `LoaderV1` querying `maskrom-id`
- `UBOOT1` calling flash operations
- `LoaderV2`/`LoaderV1`/`UBOOT1` probe fallback behavior

Current status:

- several unsupported combinations are now unit-tested
- full unsupported matrix is still incomplete

### 7. Exit code behavior

Verify:

- confirmation-required path returns `14`
- local file errors return `10`
- device-not-found returns `11`
- transport/protocol failures are distinguishable

Current status:

- `10`, `11`, and `14` are covered in CLI integration tests
- transport-failure `12` is covered by fake-device tests
- explicit protocol-failure `13` coverage is still worth adding

### 8. Progress output behavior

Verify:

- `--progress` emits to `stderr`
- binary `--stdout` output is not polluted
- JSON output is not polluted

Current status:

- implemented in code
- covered in fake-device success-path tests for `jluboot` and `jlrunner`

## What cannot be trusted without hardware

- actual `/dev/sgX` enumeration quality
- actual Windows `SetupAPI` enumeration and USB-MSC-to-disk-path correlation on real systems
- real `SG_IO` sense/status handling
- command acceptance by specific chips/loaders
- maximum transfer size
- erase alignment rules
- whether `GET_LOADER_VER` really returns the expected byte order for every chip
- `chipkey` decode correctness on real silicon

## Recommended pre-hardware checklist

- `cargo check` passes
- `cargo fmt --check` passes
- `cargo clippy --all-targets --all-features -- -D warnings` passes
- `cargo test` passes
- README and USAGE match implemented commands
- TODOs are current
- first-hardware test plan is ready
- current no-hardware coverage is reviewed against:
  - [`NO_HARDWARE_TEST_MATRIX.md`](./NO_HARDWARE_TEST_MATRIX.md)

## Related documents

- [`NO_HARDWARE_TEST_MATRIX.md`](./NO_HARDWARE_TEST_MATRIX.md)
- [`MOCK_TEST_EXPANSION_PLAN.md`](./MOCK_TEST_EXPANSION_PLAN.md)
