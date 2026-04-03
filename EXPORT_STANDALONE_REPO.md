# Export Standalone Repo

This file defines the practical steps to export `host-tools/jl-uboot-rs` into a
standalone Git repository.

## Source path

Current in-tree location:

- `host-tools/jl-uboot-rs`

## Standalone-repo goals

- preserve the existing Rust workspace layout
- preserve documentation and attribution
- avoid dragging unrelated SDK files into the new repository
- keep the exported repository buildable immediately

## Files and directories that must exist after export

- `Cargo.toml`
- `Cargo.lock`
- `LICENSE`
- `ATTRIBUTION.md`
- `CONTRIBUTING.md`
- `README.md`
- `USAGE.md`
- `ROADMAP.md`
- `RELEASE_NOTES_DRAFT.md`
- `PROTOCOL_COVERAGE.md`
- `TESTING_WITHOUT_HARDWARE.md`
- `NO_HARDWARE_TEST_MATRIX.md`
- `MOCK_TEST_EXPANSION_PLAN.md`
- `REAL_DEVICE_TEST_PLAN.md`
- `COMPATIBILITY_MATRIX.md`
- `HARDWARE_VALIDATION_LOG_TEMPLATE.md`
- `WINDOWS_PORT_PLAN.md`
- `TODOs.md`
- `REPOSITORY_SPLIT_CHECKLIST.md`
- `.gitignore`
- `.github/workflows/ci.yml`
- `.github/ISSUE_TEMPLATE/bug_report.md`
- `apps/`
- `crates/`

## Direct standalone initialization

If this directory is going to become the standalone repository directly, the
minimal workflow is:

```bash
cd host-tools/jl-uboot-rs
rm -rf target
git init
git add .
git commit -m "Initial import from SDK workspace"
```

### Run baseline checks

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

### Review repository metadata

Confirm these still make sense in the standalone repo:

- `README.md`
- `ATTRIBUTION.md`
- `LICENSE`
- `.github/workflows/ci.yml`

### Create remote and push

```bash
git remote add origin <new-repo-url>
git branch -M main
git push -u origin main
```

## If copying to another directory first

That is still valid. The required contents are the same. The only thing that
matters is that the final standalone repository root is exactly this workspace.

## Post-initialization follow-up

Recommended immediate follow-up work:

1. Replace draft wording in `RELEASE_NOTES_DRAFT.md` if publishing publicly
2. Add a repository description and topics
3. Add a short bug-report section to the repository front page
4. Record first real-device validation results in:
   - `COMPATIBILITY_MATRIX.md`
   - `HARDWARE_VALIDATION_LOG_TEMPLATE.md`

## Explicit non-goals of export

- rewriting history from the SDK tree
- importing unrelated SDK files
- adding package-helper features during export
