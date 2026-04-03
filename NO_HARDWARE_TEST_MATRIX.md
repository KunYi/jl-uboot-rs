# No-Hardware Test Matrix

This file lists what is currently covered without real JieLi hardware and what still remains open.

## Build and lint

Covered:

- `cargo check`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

Status:

- automated
- expected to stay green in every change set

## Protocol-layer unit tests

Location:

- `crates/jl-msc/src/lib.rs`
- `crates/jl-uboot/src/lib.rs`

Covered now:

- CDB build shape
- response command parsing
- command mismatch detection
- CRC helpers
- chipkey cipher reversibility
- `LoaderV2 probe_info`
- `LoaderV2 flash_crc16`
- `LoaderV2 flash_crc16_raw`
- `LoaderV2 read_status`
- `LoaderV2 set_flash_cmds`
- `LoaderV2 run_app`
- `LoaderV2 write_chipkey`
- `LoaderV1 flash_read`
- `LoaderV1 flash_select`
- `LoaderV1 unsupported usb_buffer_size`
- `UBOOT1 mem_write_rxgp`
- `UBOOT1 unsupported flash_read`
- protocol-specific `mem_jump`
- transport error propagation

Not yet covered:

## CLI integration tests

Location:

- `apps/jluboot/tests/cli.rs`
- `apps/jlrunner/tests/cli.rs`

Covered now:

- destructive command requires `--yes`
- destructive command with `--yes` reaches device lookup
- JSON mode on error path does not emit stdout
- `find` returns success without hardware
- `find --json` returns success without hardware
- successful `probe --json`
- `find` JSON entry structure including:
  - `usb_vid`
  - `usb_pid`
  - `note`
- `find` probe-mode JSON entry structure including:
  - `online_device`
  - `flash_id`
  - `loader_version`
- successful `flash-read --hexdump`
- successful `mem-read --stdout`
- `--progress` writes to `stderr` on a successful path
- explicit transport-failure exit code `12`
- `jlrunner` missing input file returns I/O exit code
- `jlrunner` missing device returns device-not-found exit code
- `jlrunner` successful RAM load + jump path through fake device
- `jlrunner` transport-failure exit code `12`

Not yet covered:

- explicit protocol-failure exit code `13`
- real binary-spawn success paths through the external CLI binary
- `jlrunner` no-jump success path

## Manual no-hardware review

Still useful even with tests:

- compare command IDs against upstream Python
- compare payload endianness assumptions against notes and upstream tools
- inspect CLI `--help` output after major refactors

## Current confidence level

High confidence:

- Rust workspace builds and lints
- basic command mapping stays stable
- destructive-command guardrails
- some protocol framing and response parsing

Medium confidence:

- `LoaderV2/LoaderV1/UBOOT1` command paths tested via mock transport
- Windows discovery helper logic for:
  - USB-storage instance recognition
  - `VID/PID` parsing
  - USB VID allowlist matching

Low confidence until hardware exists:

- actual `SG_IO` device behavior
- flash erase/write acceptance
- runtime loader quirks
- real chipkey behavior on silicon
- actual maximum transfer size

## Recommended next no-hardware targets

1. Add mock-backed success-path CLI tests
2. Add a dedicated `maskrom_id` success-path protocol test
3. Extend protocol-failure CLI coverage if new protocol-shaped error paths appear
