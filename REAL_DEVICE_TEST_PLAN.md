# Real Device Test Plan

Use this document once a real JieLi USB MSC/UBOOT device is available.

## Preconditions

- Linux machine with permission to access `/dev/sgX`
- Rust tool built successfully
- one known-good USB cable
- one sacrificial test device if possible
- record the exact chip / board / loader / SDK context

Suggested environment capture:

- chip family
- board name
- current firmware origin
- whether the device enumerates as `UBOOT`, `UDISK`, or `DEVICE`
- whether the device is expected to use `LoaderV2`, `LoaderV1`, or `UBOOT1`

## Safety rules

- First session must be read-only until probe commands succeed
- Do not run `flash-erase-chip` first
- Do not write chip key on the first hardware session
- Always dump small ranges before writing
- Prefer a non-production device

## Phase 1: device discovery

### 1. Find `/dev/sgX`

```bash
cargo run -p jluboot -- find
```

Record:

- discovered path
- whether multiple `sg` devices exist

### 2. Basic probe with assumed protocol

Start with `LoaderV2`:

```bash
cargo run -p jluboot -- probe --device /dev/sgX
```

If needed, retry:

```bash
cargo run -p jluboot -- --protocol loaderv1 probe --device /dev/sgX
cargo run -p jluboot -- --protocol uboot1 probe --device /dev/sgX
```

Record:

- inquiry vendor/product/revision
- which protocol gives the most useful output
- which fields are unavailable

## Phase 2: read-only verification

### 1. Query information

```bash
cargo run -p jluboot -- read-id --device /dev/sgX
cargo run -p jluboot -- online-device --device /dev/sgX
cargo run -p jluboot -- usb-buffer-size --device /dev/sgX
cargo run -p jluboot -- version --device /dev/sgX
cargo run -p jluboot -- maskrom-id --device /dev/sgX
```

For `LoaderV1` / `UBOOT1`, expect some commands to return unsupported.

### 2. Read small flash range

```bash
cargo run -p jluboot -- flash-read \
  --device /dev/sgX \
  --address 0 \
  --length 256 \
  --hexdump
```

Then dump a larger block:

```bash
cargo run -p jluboot -- flash-read \
  --device /dev/sgX \
  --address 0 \
  --length 4096 \
  --output flash-0000.bin
```

Check:

- command succeeds
- file length matches request
- repeated reads are identical

### 3. Read small memory range

```bash
cargo run -p jluboot -- mem-read \
  --device /dev/sgX \
  --address 0x1f0000 \
  --length 256 \
  --hexdump
```

If address is invalid on the target, adapt to a known-good RAM region.

### 4. Read chip key cautiously

```bash
cargo run -p jluboot -- chip-key --device /dev/sgX --raw
cargo run -p jluboot -- chip-key --device /dev/sgX
```

Record:

- raw payload
- decoded result
- whether repeated reads are stable

## Phase 3: RAM write path

Use a small harmless payload first.

### 1. Write RAM only

```bash
cargo run -p jluboot -- mem-write \
  --device /dev/sgX \
  --address 0x1f0000 \
  --input payload.bin
```

### 2. Read back RAM

```bash
cargo run -p jluboot -- mem-read \
  --device /dev/sgX \
  --address 0x1f0000 \
  --length <payload_size> \
  --output payload-readback.bin
```

Compare:

```bash
cmp payload.bin payload-readback.bin
```

Only after readback is correct should `jump` or `jlrunner` be tested.

### 3. Jump / run test

```bash
cargo run -p jluboot -- jump --device /dev/sgX --address 0x1f0000 --arg 0
```

or

```bash
cargo run -p jlrunner -- \
  --device /dev/sgX \
  --address 0x1f0000 \
  --file payload.bin
```

For `LoaderV2`, `run-app` can be tested separately:

```bash
cargo run -p jluboot -- run-app --device /dev/sgX --arg 1
```

## Phase 4: flash write path

Do this only after read-only and RAM tests pass.

### 1. Pick a safe area

Use:

- unused region
- scratch image area
- sacrificial board

### 2. Backup before writing

```bash
cargo run -p jluboot -- flash-read \
  --device /dev/sgX \
  --address 0x1000 \
  --length 4096 \
  --output before.bin
```

### 3. Erase sector

```bash
cargo run -p jluboot -- flash-erase-sector \
  --device /dev/sgX \
  --address 0x1000
```

### 4. Write test payload

```bash
cargo run -p jluboot -- flash-write \
  --device /dev/sgX \
  --address 0x1000 \
  --input patch.bin
```

### 5. Read back and compare

```bash
cargo run -p jluboot -- flash-read \
  --device /dev/sgX \
  --address 0x1000 \
  --length <patch_size> \
  --output after.bin
cmp patch.bin after.bin
```

## Phase 5: destructive / irreversible commands

Only after all earlier phases are stable.

### Flash erase chip

```bash
cargo run -p jluboot -- flash-erase-chip --device /dev/sgX
```

### Write chip key

```bash
cargo run -p jluboot -- write-chip-key \
  --device /dev/sgX \
  --key 0x12345678 \
  --vpp 5000
```

Do not run this unless:

- you know the target is intended for key programming
- voltage path is correct
- you accept irreversible outcomes

## What to record for each test

- command line used
- protocol used
- stdout/stderr
- sense/status failures if any
- whether result is stable on repeat
- chip/board identification

## Exit criteria

The implementation is minimally validated when:

- `probe` works on the intended protocol
- read-only commands succeed
- RAM write + readback works
- at least one safe flash write + readback works

Only after that should protocol parity work continue.
