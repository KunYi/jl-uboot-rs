# Windows Port Plan

This file scopes a Windows-capable transport layer for `jl-uboot-rs`.

## Current state

Implemented now:

- Linux `/dev/sgX` discovery
- Linux `SG_IO` transport in `jl-sg`
- higher protocol layers in `jl-uboot`
- CLI layers in `jluboot` and `jlrunner`

Implemented now:

- `jl_sg::windows` module exists
- `WindowsScsiDevice` exists
- `CreateFileW` + `DeviceIoControl` + `SCSI_PASS_THROUGH_DIRECT` code path exists
- `SetupAPI` enumeration boundary exists in code:
  - `enumerate_usb_msc_candidates()`
  - `SetupApiUsbMscCandidate`
- `SetupAPI` disk-interface enumeration and early VID/PID filtering are implemented
- current default early VID allowlist includes `0x4A4C`
- Windows discovery now attempts to correlate visible selectors such as `E:` / `X:` to disk-interface candidates via `IOCTL_STORAGE_GET_DEVICE_NUMBER`

Not implemented now:

- real-hardware validation of the current pass-through implementation
- response-format and sense-data tuning based on real devices

The protocol and CLI layers are already mostly portable. The transport layer is still the missing piece.

## Why Windows is different

The current Linux path relies on:

- SCSI Generic character devices
- `SG_IO`
- stable `/dev/sgX` naming

Windows does not expose the same API surface. The equivalent design needs:

- device enumeration for the USB Mass Storage target
- a SCSI pass-through path
- stable device selection rules

The required Windows assumption is:

- JieLi download-mode targets enumerate as USB Mass Storage devices
- device discovery must start from USB MSC enumeration
- the implementation must then resolve the corresponding disk/storage device path for SCSI pass-through
- `INQUIRY` plus JieLi-specific probe commands are used only after that mapping step
- the user-facing device selector should be something the operator can actually see,
  such as a mounted MSC volume like `E:` or `X:`
- the tool should convert that visible selector to the actual pass-through path internally
- if the user does not pass `--device`, the tool should perform automatic detection
- when a target is selected, logs should report the visible selector back to the user

## Recommended architecture

Keep the crate split unchanged:

- `jl-sg`
  - add a Windows backend
- `jl-uboot`
  - no protocol changes expected
- `jluboot`
  - keep CLI unchanged except for device-selector conventions

That means the Windows work should stay localized to the transport crate.

## Transport strategy

Preferred direction:

- add `jl_sg::windows`
- implement a Windows `ScsiTransport`
- mirror the Linux backend shape:
  - open device
  - `exec(cdb, data_out, data_in_len)`
  - `inquiry()`

This keeps `JlDevice<T: ScsiTransport>` unchanged.

## Device discovery strategy

Required first-stage strategy:

- use `SetupAPI` to enumerate candidate USB MSC devices
- apply an early USB VID allowlist filter before doing storage-path correlation
- correlate them to the corresponding disk/storage interface path
- correlate visible selectors such as `E:` / `X:` to that same underlying target
- open that path with `CreateFileW`
- send `INQUIRY`
- keep devices whose inquiry strings look like JieLi download-mode targets
- optionally run JieLi-specific probe commands after `INQUIRY`

This is stricter than blind `HardDiskVolume` probing and is the intended design baseline.

Recommended filtering order:

1. USB-layer filter
   - start with a USB VID allowlist
   - the current best-known JieLi VID candidate is `0x4A4C`
   - this is an early filter only, not the final proof that the target is correct
2. storage-path mapping
   - resolve the corresponding disk/storage interface path for SCSI pass-through
   - resolve user-visible selectors such as `E:` / `X:` to that same path
3. `INQUIRY` filter
   - keep devices whose inquiry strings match expected JieLi download-mode behavior
4. JieLi-specific probe
   - run `read-id`, `online-device`, or other safe probe commands if needed

The reason to keep VID filtering as an early stage rather than the final decision is simple:

- VID helps remove obviously unrelated USB devices earlier
- but the actual downloader still talks to the storage/SCSI device, not to the raw USB device node
- the final target must therefore still be confirmed at the `INQUIRY` and protocol-probe levels

The code should therefore evolve in two steps:

1. `enumerate_usb_msc_candidates_with_allowlist(allowlist)`
   - enumerate Windows-side USB MSC candidates
   - apply the USB VID allowlist immediately
   - record USB metadata such as `vid/pid` when available
   - collect enough metadata to identify the correct storage path
2. `enumerate_usb_msc_candidates()`
   - use `DEFAULT_USB_VID_ALLOWLIST`
3. `candidate_windows_paths()`
   - reduce those candidates to the concrete pass-through paths that `WindowsScsiDevice` can open

The intended code-level defaults are:

- `DEFAULT_USB_VID_ALLOWLIST = [0x4A4C]`
- callers that need broader discovery can pass their own allowlist explicitly

## Likely Windows backend requirements

Expected responsibilities:

- open a Windows device handle
- send SCSI pass-through commands
- support:
  - no data phase
  - data-in
  - data-out
- collect returned data and status

The transport crate should isolate all Windows-specific unsafe/FFI code.

## Recommended implementation order

1. Replace the current `Unsupported` skeleton in `jl-sg::windows`
2. Implement `inquiry()`
3. Implement one generic `exec()` path
4. Verify `probe` against a Windows-visible MSC target discovered via `SetupAPI`
5. Verify:
   - `read-id`
   - `flash-read`
   - `mem-read`
6. Only then add automatic enumeration

## Testing before hardware

Even without Windows hardware attached, the following can still be validated:

- crate compiles behind `cfg(windows)`
- transport API shape matches Linux backend expectations
- higher-level protocol tests remain unchanged

The existing mock-based tests already reduce the Windows-specific unknowns to the transport layer.

## Practical boundary

This project should not attempt all of the following at once:

- Windows GUI
- installer packaging
- automatic flashing workflows
- full removable-device enumeration

The first Windows milestone should be:

- a command-line transport backend
- user-visible device selector resolution
- automatic detection when `--device` is omitted
- visible-selector reporting in logs/output
- successful `probe` and basic read paths

## Exit condition

The Windows port is in acceptable first-stage shape when:

- `jluboot probe` works on Windows with a user-visible selector such as `E:` / `X:`
- `jluboot probe` works on Windows without `--device` by selecting a single matching target automatically
- `read-id` works
- at least one read path works:
  - `flash-read`
  - or `mem-read`
- the protocol layer stays unchanged between Linux and Windows
