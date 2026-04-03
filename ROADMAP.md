# Roadmap

This file defines the practical release order for `jl-uboot-rs`.

## v0.1 target

Scope:

- Linux host-side force download CLI is usable
- Windows transport/discovery code exists
- `LoaderV2`, `LoaderV1`, and `UBOOT1` protocol surfaces are implemented
- no-hardware protocol and CLI coverage is in place
- standalone-repo metadata is present

Expected user value:

- Linux users can inspect and operate JieLi download-mode targets with:
  - `find`
  - `probe`
  - query commands
  - flash read/write/erase
  - memory read/write/jump
  - chip-key commands
- Windows users can evaluate the current implementation, but should treat it as
  pre-validation code until real-hardware tests are recorded

Release blockers:

- none for an explicitly labeled experimental `v0.1`

Known limitations:

- Windows support is not yet validated on real hardware
- selector correlation on Windows is implemented but still unverified on real targets
- package helpers (`bfu`, `JLFS`, keyfile tooling) remain out of scope

## v0.2 target

Focus:

- real-hardware validation on BR28/AC701N
- real-hardware validation on at least one Windows MSC target
- confirm:
  - `probe`
  - `read-id`
  - `online-device`
  - `flash-read`
  - `mem-read`
- publish compatibility notes by protocol family

Likely output:

- validated Linux workflow notes
- first confirmed Windows device-path/selector notes
- reduced protocol-risk list

## v0.3 target

Focus:

- automation quality
- machine-readable progress and error behavior cleanup
- stronger selector UX and compatibility notes

Possible additions:

- JSON error envelope
- release binaries
- issue templates / bug report guide

## Explicit non-goals for now

- official-tool parity claims
- full manufacturing workflow replacement
- `jl-misctools` feature merge
- GUI tooling
