# Protocol Coverage

This file tracks command-level coverage against the upstream Python tools.

Upstream references:

- `https://github.com/kagaimiq/jl-uboot-tool`
- `https://github.com/kagaimiq/jl-misctools`

## LoaderV2

Implemented:

- `online_device`
- `read_id`
- `usb_buffer_size`
- `version`
- `maskrom_id`
- `run_app`
- `read_status`
- `set_flash_cmds`
- `flash_crc16`
- `flash_crc16_raw`
- `chip_key`
- `write_chipkey`
- `flash_read`
- `flash_write`
- `flash_erase_sector`
- `flash_erase_block`
- `flash_erase_chip`
- `mem_read`
- `mem_write`
- `mem_jump`

Pending mainly for validation:

- confirm exact payload/endianness behavior on real BR28/AC701N hardware

## LoaderV1

Implemented:

- `online_device`
- `read_id`
- `flash_select`
- `flash_read`
- `flash_write`
- `flash_erase_sector`
- `flash_erase_block`
- `flash_erase_chip`
- `mem_write`
- `mem_jump`

Not implemented:

- reset path
- any `chipkey-ish` flow, only if later required by real hardware

## UBOOT1

Implemented:

- `mem_read`
- `mem_write`
- `mem_write_rxgp`
- `mem_jump`

Pending mainly for validation:

- confirm `RxGp` path behavior on real hardware

## CLI coverage

Implemented commands:

- `find`
- `probe`
- `read-id`
- `online-device`
- `usb-buffer-size`
- `version`
- `maskrom-id`
- `read-status`
- `flash-crc16`
- `flash-crc16-raw`
- `set-flash-cmds`
- `chip-key`
- `write-chip-key`
- `flash-select`
- `flash-read`
- `flash-write`
- `flash-erase-sector`
- `flash-erase-block`
- `flash-erase-chip`
- `mem-read`
- `mem-write`
- `mem-write-rxgp`
- `jump`
- `run-app`

## Practical summary

The remaining gap is no longer command surface. The remaining gap is protocol verification on real devices and a few optional quality-of-life features.
