# Mock Test Expansion Plan

This file defines how to keep improving no-hardware coverage before real devices arrive.

## Goal

Move more verification from:

- manual review
- first-device debugging

into:

- deterministic unit tests
- deterministic integration tests

## Current baseline

Already present:

- protocol mock transport in `crates/jl-uboot`
- CLI integration tests for several error paths
- `jluboot` command execution moved into `lib.rs`
- `jlrunner` command execution moved into `lib.rs`
- fake-device success-path tests for key output modes

What is still missing:

- explicit transport/protocol exit-code path tests
- a protocol-failure CLI-path test

## Priority 1: protocol command completion

Add or refine unit tests for:

- `flash_erase_sector/block/chip` command selection
- `mem_write` command selection for each protocol

Reason:

- these are still protocol-shape assumptions
- they do not require hardware to verify framing

## Priority 2: injectable CLI transport

Implemented now:

- `jluboot` command execution is behind a `DeviceFactory`
- production uses the Linux SG opener
- tests can inject fake devices for success-path validation

This now allows:

- successful `--json`
- successful `--stdout`
- successful `--hexdump`
- `--progress` emitting only to `stderr`
- transport failure exit code `12`
- fake-device success-path tests for `jlrunner`
- stable `find` JSON entry shape tests including candidate metadata
- stable `find --probe` JSON entry shape tests including probe metadata

## Priority 3: golden output tests

Once CLI transport is injectable, add golden-output tests for:

- `probe --json`
- `read-id --json`
- `flash-read --hexdump`
- `mem-read --stdout`

These tests should verify:

- exact stdout format
- exact stderr format
- no cross-contamination between stdout/stderr

## Priority 4: error-shape normalization

Consider normalizing machine-oriented failure output for:

- `--json` mode

Example direction:

- structured JSON on stdout for success
- structured JSON on stderr or stdout for failure, if mode is explicitly machine-oriented

This should be decided before large automation users depend on current behavior.

## Suggested technical approach

### Option A: trait-based opener

Create a small trait such as:

- `DeviceOpener`

Production:

- opens `LinuxSgDevice`

Tests:

- returns fake transport/device

### Option B: refactor CLI into reusable library functions

Move command execution from `main.rs` into:

- `lib.rs`

Then test:

- command handlers directly
- output sinks separately

This is probably the cleaner long-term direction.

## Recommendation

Preferred next step:

- extend fake-device and protocol-path coverage for remaining protocol edge cases and protocol-failure CLI behavior

## Exit condition for this plan

This mock-expansion plan can be considered sufficient when:

- protocol framing coverage is near-complete
- CLI success and error modes are both tested without hardware
- only silicon-specific behavior remains for real-device testing
