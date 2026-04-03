# jl-uboot-rs

Rust workspace for an unofficial JieLi host-side force download toolset.

See also:

- [`LICENSE`](./LICENSE)
- [`ATTRIBUTION.md`](./ATTRIBUTION.md)
- [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- [`PROJECT_STATUS.md`](./PROJECT_STATUS.md)
- [`USAGE.md`](./USAGE.md)
- [`ROADMAP.md`](./ROADMAP.md)
- [`RELEASE_NOTES_DRAFT.md`](./RELEASE_NOTES_DRAFT.md)
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

This workspace is intentionally kept separate from the SDK build system and is maintained as its own standalone repository.

Repository metadata now included for standalone use:

- `LICENSE` (`MIT`)
- `ATTRIBUTION.md`
- `.gitignore`
- GitHub Actions CI

Project status and validation notes are tracked separately in:

- [`PROJECT_STATUS.md`](./PROJECT_STATUS.md)
- [`ROADMAP.md`](./ROADMAP.md)
- [`RELEASE_NOTES_DRAFT.md`](./RELEASE_NOTES_DRAFT.md)

Terminology note:

- this repository uses the practical term `force download tool` because that is
  the term commonly used by the official tooling ecosystem
- this repository is still an unofficial implementation

Upstream attribution and license note:

- see [`ATTRIBUTION.md`](./ATTRIBUTION.md)
- the two primary upstream tooling references currently tracked here,
  [kagaimiq/jl-uboot-tool](https://github.com/kagaimiq/jl-uboot-tool) and
  [kagaimiq/jl-misctools](https://github.com/kagaimiq/jl-misctools), are MIT licensed

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
