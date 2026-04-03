# Usage

This workspace is an unofficial host-side force download tool for JieLi
MSC/UBOOT-style download-mode devices.

There are only two binaries:

- `jluboot`
- `jlrunner`

`find` is a `jluboot` subcommand, not a separate tool.

Linux USB MSC targets usually show up in two forms:

- `/dev/sdX` or `/dev/sdX1`
  - block-device / mounted-storage view
- `/dev/sgX`
  - SCSI generic pass-through view

This tool ultimately talks to the SCSI-generic side. On Linux, `--device` may
therefore be given either as `/dev/sgX` directly or as a Linux block selector
such as `/dev/sdX`, which the tool can resolve internally to the corresponding
`/dev/sgX`.

On Windows, the intended `--device` input is the user-visible MSC volume
selector, such as `E:` or `X:`, and the tool should resolve that internally to
the actual storage/SCSI pass-through path.

Selection model:

- with `--device`, the user provides a visible selector
- without `--device`, the tool should use automatic detection
- logs should report the user-visible selector when available

Automatic detection requires a single unambiguous JieLi download-mode target. If
none or multiple matching targets are found, pass `--device` explicitly.

## Build

```bash
cargo build
```

## Protocol selection

`jluboot` defaults to `LoaderV2`.

To override:

```bash
cargo run -p jluboot -- --protocol loaderv1 probe --device /dev/sg3
cargo run -p jluboot -- --protocol uboot1 mem-read --device /dev/sg3 --address 0x1f0000 --length 256 --hexdump
```

`jlrunner` supports the same protocol selector:

```bash
cargo run -p jlrunner -- --protocol loaderv1 --device /dev/sg3 --address 0x1f0000 --file payload.bin
```

## Global options

`jluboot` supports these global options before the subcommand:

- `--protocol {loaderv2,loaderv1,uboot1}`
- `--timeout-ms <u32>`
- `--json`
- `--yes`
- `--progress`
- `--chunk-size <usize>`

Example:

```bash
cargo run -p jluboot -- \
  --protocol loaderv2 \
  --timeout-ms 8000 \
  --progress \
  --chunk-size 1024 \
  --json \
  read-id --device /dev/sg3
```

`jlrunner` supports:

- `--protocol {loaderv2,loaderv1,uboot1}`
- `--timeout-ms <u32>`
- `--progress`
- `--chunk-size <usize>`

## Find candidate devices

```bash
cargo run -p jluboot -- find
```

Machine-readable form:

```bash
cargo run -p jluboot -- --json find
```

This matches the upstream Python behavior at the filtering stage:

- enumerate platform candidates
- send standard `INQUIRY`
- keep candidates whose product starts with:
  - `UBOOT`
  - `UDISK`
  - `DEVICE`

When the transport layer can provide candidate metadata, `find` may also show:

- `usb_vid`
- `usb_pid`
- `note`

This is mainly intended for Windows discovery, where `SetupAPI` enumeration can provide USB-side hints before `INQUIRY` and JieLi-specific probe steps.

You can also request a stronger second-stage check:

```bash
cargo run -p jluboot -- --protocol loaderv2 find --probe
```

Optional vendor filter:

```bash
cargo run -p jluboot -- find --vendor JIELI
```

## Probe a device

```bash
cargo run -p jluboot -- probe --device /dev/sg3
```

Automatic detection:

```bash
cargo run -p jluboot -- probe
```

This prints:

- USB inquiry vendor/product/revision
- online device type and optional ID
- flash ID
- USB buffer size
- loader version
- maskrom ID

JSON form:

```bash
cargo run -p jluboot -- --json probe --device /dev/sg3
```

## Read flash

Write to file:

```bash
cargo run -p jluboot -- flash-read \
  --device /dev/sg3 \
  --address 0 \
  --length 4096 \
  --output flash.bin
```

Print hexdump:

```bash
cargo run -p jluboot -- flash-read \
  --device /dev/sg3 \
  --address 0 \
  --length 256 \
  --hexdump
```

Write raw bytes to stdout:

```bash
cargo run -p jluboot -- flash-read \
  --device /dev/sg3 \
  --address 0 \
  --length 256 \
  --stdout > out.bin
```

## Write flash

```bash
cargo run -p jluboot -- flash-write \
  --yes \
  --device /dev/sg3 \
  --address 0x1000 \
  --input patch.bin
```

The write path uses `--chunk-size` for chunked transfer. This is useful if a loader rejects large transfers.
If you want transfer progress on stderr, add `--progress`.

## Erase flash

Sector:

```bash
cargo run -p jluboot -- flash-erase-sector \
  --yes \
  --device /dev/sg3 \
  --address 0x1000
```

Block:

```bash
cargo run -p jluboot -- flash-erase-block \
  --yes \
  --device /dev/sg3 \
  --address 0x0000
```

Chip:

```bash
cargo run -p jluboot -- --yes flash-erase-chip --device /dev/sg3
```

## Query information

```bash
cargo run -p jluboot -- read-id --device /dev/sg3
cargo run -p jluboot -- online-device --device /dev/sg3
cargo run -p jluboot -- usb-buffer-size --device /dev/sg3
cargo run -p jluboot -- version --device /dev/sg3
cargo run -p jluboot -- maskrom-id --device /dev/sg3
cargo run -p jluboot -- read-status --device /dev/sg3
cargo run -p jluboot -- flash-crc16 --device /dev/sg3 --address 0 --length 4096
cargo run -p jluboot -- flash-crc16-raw --device /dev/sg3 --address 0 --length 4096
```

Set flash command bytes for `LoaderV2`:

```bash
cargo run -p jluboot -- set-flash-cmds \
  --yes \
  --device /dev/sg3 \
  --cmds 0xC7 0xD8 0x20 0x03 0x02 0x05 0x06 0x01
```

## Chip key

Decoded chip key:

```bash
cargo run -p jluboot -- chip-key --device /dev/sg3
```

Raw response payload:

```bash
cargo run -p jluboot -- chip-key --device /dev/sg3 --raw
```

Write key:

```bash
cargo run -p jluboot -- write-chip-key \
  --yes \
  --device /dev/sg3 \
  --key 0x12345678 \
  --vpp 5000
```

LoaderV1 flash select:

```bash
cargo run -p jluboot -- --protocol loaderv1 flash-select --device /dev/sg3 --kind code
cargo run -p jluboot -- --protocol loaderv1 flash-select --device /dev/sg3 --kind data
```

## Memory access

Read memory to file:

```bash
cargo run -p jluboot -- mem-read \
  --device /dev/sg3 \
  --address 0x1f0000 \
  --length 256 \
  --output mem.bin
```

Read memory as hexdump:

```bash
cargo run -p jluboot -- mem-read \
  --device /dev/sg3 \
  --address 0x1f0000 \
  --length 256 \
  --hexdump
```

Write memory:

```bash
cargo run -p jluboot -- mem-write \
  --device /dev/sg3 \
  --address 0x1f0000 \
  --input payload.bin
```

Write memory using `RxGp` path for `UBOOT1`:

```bash
cargo run -p jluboot -- --protocol uboot1 mem-write-rxgp \
  --device /dev/sg3 \
  --address 0x1f0000 \
  --input payload.bin
```

The memory write paths also use `--chunk-size`.
Add `--progress` if you want stderr progress during chunked transfer.

Jump:

```bash
cargo run -p jluboot -- jump \
  --yes \
  --device /dev/sg3 \
  --address 0x1f0000 \
  --arg 0
```

Run app:

```bash
cargo run -p jluboot -- --yes run-app --device /dev/sg3 --arg 1
```

## jlrunner

`jlrunner` is a smaller RAM-load helper.

```bash
cargo run -p jlrunner -- \
  --timeout-ms 8000 \
  --progress \
  --chunk-size 1024 \
  --device /dev/sg3 \
  --address 0x1f0000 \
  --file payload.bin
```

## Exit codes

For automation:

- `0`: success
- `10`: local I/O failure
- `11`: device path not found / target not present
- `12`: transport-layer failure
- `13`: protocol/response failure
- `14`: confirmation required (`--yes`)

## Test coverage without hardware

Current no-hardware validation covers:

- protocol-layer mock tests for key `LoaderV2`, `LoaderV1`, and `UBOOT1` command paths
- CLI integration tests for:
  - destructive-command confirmation
  - JSON error-path behavior
  - file-not-found exit code
  - device-not-found exit code

This does not replace real-device validation.
Use [`REAL_DEVICE_TEST_PLAN.md`](./REAL_DEVICE_TEST_PLAN.md) once target hardware is available.
