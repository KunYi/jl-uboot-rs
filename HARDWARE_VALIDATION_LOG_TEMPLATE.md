# Hardware Validation Log Template

Use this template when validating `jl-uboot-rs` on a real target.

## Session metadata

- Date:
- Tester:
- Commit:
- Host OS:
- Host OS version:
- Target board:
- Target chip:
- Protocol path:
  - `LoaderV2`
  - `LoaderV1`
  - `UBOOT1`

## Device visibility

- User-visible selector:
  - Linux:
    - `/dev/sgX`
    - `/dev/sdX`
  - Windows:
    - `E:`
    - `X:`
- Resolved transport path:
- Auto-detect used:
  - yes / no

## Baseline comparison

- Official JieLi tool available:
  - yes / no
- Official tool succeeds on same target:
  - yes / no / unknown

## Commands executed

```text
paste exact commands here
```

## Results

| Command | Expected | Actual | Status |
| --- | --- | --- | --- |
| `probe` | | | |
| `read-id` | | | |
| `online-device` | | | |
| `flash-read` | | | |
| `mem-read` | | | |
| `chip-key` | | | |

## Raw notes

- selector behavior:
- response format quirks:
- timeouts/retries:
- sense/status observations:
- anything that differs from Linux or official tooling:
