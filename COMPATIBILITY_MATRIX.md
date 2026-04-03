# Compatibility Matrix

This file records observed behavior on real JieLi targets.

Status values:

- `yes`
- `no`
- `partial`
- `untested`

## Host environments

| Host OS | Version | Transport/discovery | Notes |
| --- | --- | --- | --- |
| Linux | untested | untested | |
| Windows | untested | untested | |

## Protocol coverage by target

| Target | Chip family | Protocol | Probe | Read ID | Online Device | Flash Read | Flash Write | Mem Read | Mem Write | Jump / Run | Chip Key | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Example | BR28 / AC701N | LoaderV2 | untested | untested | untested | untested | untested | untested | untested | untested | untested | |

## Selector behavior

| Host OS | Selector kind | Status | Notes |
| --- | --- | --- | --- |
| Linux | `/dev/sgX` | untested | |
| Linux | `/dev/sdX` -> `/dev/sgX` resolution | untested | |
| Windows | visible selector such as `E:` / `X:` | untested | |
| Windows | automatic detection without `--device` | untested | |

## Real-device notes

Use this section for concise observations that do not fit the tables above.

- none yet
