# Attribution

This workspace was developed with direct reference to prior public work on
JieLi tooling and reverse engineering.

## Upstream references

Primary upstream references:

- `https://github.com/kagaimiq/jl-uboot-tool`
- `https://github.com/kagaimiq/jl-misctools`
- `https://github.com/kagaimiq/jielie`

Primary upstream author:

- Andrey Grigoryev
- GitHub: `https://github.com/kagaimiq`

## What this workspace reuses conceptually

This repository does not embed the original Python implementation, but it does
reuse and re-express several ideas and protocol observations from the upstream
projects:

- JieLi USB MSC / SCSI vendor-command workflow
- `UBOOT1`, `LoaderV1`, and `LoaderV2` command structure
- device discovery model based on:
  - candidate device enumeration
  - `INQUIRY`
  - JieLi-specific probing
- chip-key handling model and related operational constraints
- firmware-tooling context documented in the upstream repositories

## What this workspace implements independently

This workspace provides an independent Rust implementation of:

- transport boundaries
- Linux `SG_IO` handling
- Windows transport/discovery scaffolding
- protocol-layer Rust types and command execution
- CLI tools:
  - `jluboot`
  - `jlrunner`
- no-hardware test scaffolding and CLI/protocol regression coverage

## Explicit non-goals of attribution

This file does not claim:

- behavioral parity with every upstream tool revision
- source-level lineage for every function or implementation detail
- ownership of upstream reverse-engineering results

The purpose of this file is to make the reference chain explicit and to avoid
misrepresenting the origin of the protocol knowledge used here.

## Scope boundary

This repository currently does **not** include full equivalents for everything
covered by the upstream ecosystem. In particular, package-oriented tooling such
as:

- `bfu`
- `JLFS`
- keyfile generation/parsing helpers

remain outside this workspace scope.

