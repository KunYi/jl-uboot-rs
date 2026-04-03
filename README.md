# jl-uboot-rs

Rust workspace for an unofficial JieLi host-side force download toolset.

See also:

- [`LICENSE`](./LICENSE)
- [`ATTRIBUTION.md`](./ATTRIBUTION.md)
- [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- [`USAGE.md`](./USAGE.md)
- [`ROADMAP.md`](./ROADMAP.md)
- [`RELEASE_NOTES_DRAFT.md`](./RELEASE_NOTES_DRAFT.md)
- [`REPOSITORY_SPLIT_CHECKLIST.md`](./REPOSITORY_SPLIT_CHECKLIST.md)
- [`PROTOCOL_COVERAGE.md`](./PROTOCOL_COVERAGE.md)
- [`TESTING_WITHOUT_HARDWARE.md`](./TESTING_WITHOUT_HARDWARE.md)
- [`NO_HARDWARE_TEST_MATRIX.md`](./NO_HARDWARE_TEST_MATRIX.md)
- [`MOCK_TEST_EXPANSION_PLAN.md`](./MOCK_TEST_EXPANSION_PLAN.md)
- [`REAL_DEVICE_TEST_PLAN.md`](./REAL_DEVICE_TEST_PLAN.md)
- [`COMPATIBILITY_MATRIX.md`](./COMPATIBILITY_MATRIX.md)
- [`HARDWARE_VALIDATION_LOG_TEMPLATE.md`](./HARDWARE_VALIDATION_LOG_TEMPLATE.md)
- [`WINDOWS_PORT_PLAN.md`](./WINDOWS_PORT_PLAN.md)
- [`TODOs.md`](./TODOs.md)

Current scope:

- establish crate boundaries
- establish protocol-facing Rust types
- provide host-side transport and discovery for JieLi download-mode devices
- cover the main upstream protocol surface for `LoaderV2`, `LoaderV1`, and `UBOOT1`
- provide CLI entry points for query, flash, memory, chipkey, and app-run paths

Current non-goals:

- strict behavioral parity with every real device/ROM revision before hardware validation
- package helpers such as `bfu` / `JLFS` / keyfile handling
- replacing official tooling terminology or pretending this is an official JieLi tool

Current binaries:

- `jluboot`
  - global options:
    - `--protocol {loaderv2,loaderv1,uboot1}`
    - `--timeout-ms <u32>`
    - `--json`
    - `--yes`
    - `--progress`
    - `--chunk-size <usize>`
  - subcommands:
    - `find`
      - default behavior matches upstream Python:
        - enumerate platform candidates
        - `INQUIRY`
        - keep `UBOOT/UDISK/DEVICE`
      - candidate metadata may include:
        - `selector`
        - `usb_vid`
        - `usb_pid`
        - `note`
      - optional:
        - `find --probe`
        - `find --vendor JIELI`
    - `probe --device <device-selector>`
    - `read-id --device <device-selector>`
    - `online-device --device <device-selector>`
    - `usb-buffer-size --device <device-selector>`
    - `version --device <device-selector>`
    - `maskrom-id --device <device-selector>`
    - `read-status --device <device-selector>`
    - `flash-crc16 --device <device-selector> --address <u32> --length <usize>`
    - `flash-crc16-raw --device <device-selector> --address <u32> --length <usize>`
    - `set-flash-cmds --device <device-selector> --cmds <8 values>`
    - `chip-key --device <device-selector> [--arg <u32>] [--raw]`
    - `write-chip-key --device <device-selector> --key <u32> [--vpp <u32>]`
    - `flash-select --device <device-selector> --kind {code,data}`
    - `flash-read --device <device-selector> --address <u32> --length <usize> --output <file>`
    - `flash-write --device <device-selector> --address <u32> --input <file>`
    - `flash-erase-sector --device <device-selector> --address <u32>`
    - `flash-erase-block --device <device-selector> --address <u32>`
    - `flash-erase-chip --device <device-selector>`
    - `mem-read --device <device-selector> --address <u32> --length <usize> --output <file>`
    - `mem-write --device <device-selector> --address <u32> --input <file>`
    - `mem-write-rxgp --device <device-selector> --address <u32> --input <file>`
    - `jump --device <device-selector> --address <u32> [--arg <u32>]`
    - `run-app --device <device-selector> [--arg <u32>]`
- `jlrunner`
  - global options:
    - `--protocol {loaderv2,loaderv1,uboot1}`
    - `--timeout-ms <u32>`
    - `--progress`
    - `--chunk-size <usize>`
  - `--device <device-selector> --address <u32> --file <bin> [--arg <u32>] [--jump true|false]`

Device-selection note:

- the intended CLI model is that users provide a visible device selector
- on Linux, USB MSC targets usually appear in two layers:
  - `/dev/sdX` or `/dev/sdX1` for block-device and mount usage
  - `/dev/sgX` for SCSI generic pass-through
- Linux examples in this repository may use `/dev/sgX` directly, but the tool can also resolve Linux block selectors such as `/dev/sdX` to the corresponding `/dev/sgX` internally
- on Windows, the intended selector is the visible MSC volume, such as `E:` or `X:`
- the tool should resolve that selector internally to the actual storage/SCSI pass-through path
- when `--device` is omitted, the tool should fall back to automatic detection
- logs and probe output should report the user-visible selector when one is known

Current CLI behavior:

- explicit mode:
  - pass `--device <device-selector>`
- automatic mode:
  - omit `--device`
  - the tool will try to find exactly one matching JieLi download-mode target
  - if none or multiple matches are found, the command fails and asks for `--device`

This workspace is intentionally kept separate from the SDK build system so it can be split into its own repository later.

Repository metadata now included for standalone use:

- `LICENSE` (`MIT`)
- `ATTRIBUTION.md`
- `.gitignore`
- GitHub Actions CI
- split checklist for standalone-repo preparation

Current status:

- primary command coverage is implemented
- chunked flash/memory transfer path is implemented for loader tolerance and large payloads
- JSON output is implemented for query/action commands that are useful in automation
- destructive operations require `--yes`
- progress output is available on stderr with `--progress`
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
- static checks pass:
  - `cargo fmt --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo check`
  - `cargo test`
- remaining risk is mainly protocol validation on real hardware, not missing CLI surface
- `jl-sg` now contains Windows transport/discovery work in addition to Linux support:
  - `WindowsScsiDevice`
  - Win32 `CreateFileW` / `DeviceIoControl` / `SCSI_PASS_THROUGH_DIRECT` path is implemented
  - `SetupAPI`-based USB MSC candidate enumeration and early VID filtering are implemented
  - Windows discovery now also attempts to back-fill visible selectors such as `E:` / `X:` by correlating volume device numbers to disk-interface candidates
  - Windows discovery/transport still require real-hardware validation before they should be considered usable
  - current remaining risk is hardware validation, not missing module structure

Terminology note:

- this repository uses the practical term `force download tool` because that is
  the term commonly used by the official tooling ecosystem
- this repository is still an unofficial implementation

Exit code policy:

- `0`
  - success
- `10`
  - local I/O failure
- `11`
  - device selector not found / target not present
- `12`
  - transport-layer failure
- `13`
  - protocol/response failure
- `14`
  - confirmation required (`--yes`)
