# TODOs

Related docs:

- [`TESTING_WITHOUT_HARDWARE.md`](./TESTING_WITHOUT_HARDWARE.md)
- [`REAL_DEVICE_TEST_PLAN.md`](./REAL_DEVICE_TEST_PLAN.md)

## Priority 1: real-device validation

- Validate `LoaderV2` on BR28/AC701N:
  - `probe`
  - `read-id`
  - `online-device`
  - `usb-buffer-size`
  - `version`
  - `maskrom-id`
  - `flash-read`
  - `mem-read`
  - `chip-key`
- Validate `LoaderV1` minimum path:
  - `probe`
  - `read-id`
  - `online-device`
  - `flash-read`
  - `flash-write`
- Validate `UBOOT1` RAM path:
  - `mem-read`
  - `mem-write`
  - `jump`

## Priority 2: protocol verification gaps

- `LoaderV2`
  - validate `read_status`
  - validate `set_flash_cmds`
  - validate `flash_crc16`
  - validate `flash_crc16_raw`
- `LoaderV1`
  - validate `flash_select`
  - `chipkey-ish` handling if needed
  - reset path
- `UBOOT1`
  - validate `rxgp` write path if required

## Priority 3: automated test coverage

- Add more unit tests around:
  - protocol framing
  - response parsing
  - chipkey decoding assumptions
  - CLI argument edge cases if moved into reusable helpers
- Add transport mocking path if protocol-layer tests need richer coverage

## Priority 4: CLI quality

- Refine JSON behavior for commands that currently stream binary data
- Add explicit machine-readable progress output if long flash transfers need automation feedback
- Add optional JSON error envelope for automation wrappers
- Define selector behavior clearly:
  - with `--device`, accept a user-visible selector
  - without `--device`, use automatic detection
- Add selector reporting in logs/output:
  - show the resolved user-visible selector when known
  - especially on Windows, show mounted MSC symbols such as `E:` / `X:`

## Priority 5: packaging and repo split

- Move workspace to standalone repo
- Add CI:
  - `cargo fmt --check`
  - `cargo clippy`
  - `cargo test`
- Add release profile and static Linux builds if useful

## Priority 6: optional integration with jl-misctools

- Add `keyfile` helpers
- Add `bfu` convenience path
- Add flash image / JLFS helpers only after core flasher is stable
