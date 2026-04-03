---
name: Bug report
about: Report a transport, protocol, selector, or compatibility issue
title: "[bug] "
labels: bug
assignees: ""
---

## Summary

Describe the failure in one or two sentences.

## Host environment

- OS:
- OS version:
- Rust tool version / commit:

## Target

- JieLi chip / board:
- Protocol path:
  - [ ] LoaderV2
  - [ ] LoaderV1
  - [ ] UBOOT1

## Device selection

- Selector provided by user:
- Resolved path shown by tool, if any:
- On Windows, visible selector:
  - e.g. `E:`
- On Linux, selector type:
  - [ ] `/dev/sgX`
  - [ ] `/dev/sdX`
  - [ ] symlink to block device

## Command used

```text
paste exact command here
```

## Expected result

Describe what should have happened.

## Actual result

Describe what happened instead.

Include full stderr/stdout if possible.

## Real-hardware notes

- Does official JieLi tooling succeed on the same target?
- Is the target definitely in download mode?
- If applicable, what visible selector or mounted device did the OS show?

## Additional context

Add logs, compatibility notes, or reproduction details here.
