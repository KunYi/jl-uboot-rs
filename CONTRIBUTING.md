# Contributing

This repository is an unofficial JieLi host-side force download toolset.

The current priority is correctness and observability, not rapid feature growth.
If you contribute changes, keep the scope narrow and make the behavior easier to
validate.

## Before opening a change

Read these files first:

- [`README.md`](./README.md)
- [`USAGE.md`](./USAGE.md)
- [`ATTRIBUTION.md`](./ATTRIBUTION.md)
- [`REAL_DEVICE_TEST_PLAN.md`](./REAL_DEVICE_TEST_PLAN.md)
- [`WINDOWS_PORT_PLAN.md`](./WINDOWS_PORT_PLAN.md)
- [`ROADMAP.md`](./ROADMAP.md)

## Contribution priorities

Preferred work:

- protocol correctness fixes
- transport bug fixes
- selector-resolution fixes
- no-hardware regression tests
- real-hardware compatibility notes
- documentation updates that reduce ambiguity

Lower priority work:

- feature expansion outside current scope
- packaging helpers such as `bfu`, `JLFS`, or keyfile tooling
- GUI work

## Coding expectations

- keep crate boundaries intact:
  - `jl-sg`: transport and discovery
  - `jl-msc`: MSC/SCSI framing helpers
  - `jl-uboot`: protocol only
  - `jluboot` / `jlrunner`: CLI only
- do not move platform-specific discovery logic into `jl-uboot`
- keep Windows-specific unsafe/FFI code localized to `jl-sg`
- prefer small patches over broad refactors unless the boundary itself is wrong

## Required local checks

Run these before proposing changes:

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Reporting compatibility

If you test on real hardware, include:

- chip / family
- protocol path used:
  - `LoaderV2`
  - `LoaderV1`
  - `UBOOT1`
- host OS and version
- selector used
  - Linux:
    - `/dev/sgX`
    - `/dev/sdX`
  - Windows:
    - visible selector such as `E:`
- exact command line
- expected behavior
- actual behavior
- whether official tooling succeeds on the same target

## Attribution boundary

Do not remove or weaken upstream attribution. This project relies on protocol
knowledge made public by upstream work documented in [`ATTRIBUTION.md`](./ATTRIBUTION.md).
