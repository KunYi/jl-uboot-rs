use thiserror::Error;

#[derive(Debug, Error)]
pub enum MscError {
    #[error("cdb length too large: {0}")]
    CdbTooLarge(usize),
    #[error("response too short")]
    ResponseTooShort,
    #[error("command mismatch: expected 0x{expected:04x}, got 0x{actual:04x}")]
    CommandMismatch { expected: u16, actual: u16 },
}

pub fn build_cdb(cmd: u16, args: &[u8]) -> Result<[u8; 16], MscError> {
    if args.len() > 14 {
        return Err(MscError::CdbTooLarge(args.len() + 2));
    }

    let mut cdb = [0xff_u8; 16];
    cdb[0..2].copy_from_slice(&cmd.to_be_bytes());
    cdb[2..2 + args.len()].copy_from_slice(args);
    Ok(cdb)
}

pub fn parse_response_command(response: &[u8]) -> Result<u16, MscError> {
    if response.len() < 2 {
        return Err(MscError::ResponseTooShort);
    }
    Ok(u16::from_be_bytes([response[0], response[1]]))
}

pub fn validate_response_command(expected: u16, response: &[u8]) -> Result<(), MscError> {
    let actual = parse_response_command(response)?;
    if actual != expected {
        return Err(MscError::CommandMismatch { expected, actual });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_cdb_pads_with_ff() {
        let cdb = build_cdb(0xfc0b, &[0x11, 0x22, 0x33]).expect("cdb");
        assert_eq!(cdb[0], 0xfc);
        assert_eq!(cdb[1], 0x0b);
        assert_eq!(&cdb[2..5], &[0x11, 0x22, 0x33]);
        assert!(cdb[5..].iter().all(|&b| b == 0xff));
    }

    #[test]
    fn build_cdb_rejects_too_large_args() {
        let args = [0u8; 15];
        let err = build_cdb(0xfc0b, &args).expect_err("must fail");
        match err {
            MscError::CdbTooLarge(len) => assert_eq!(len, 17),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_response_command_works() {
        let cmd = parse_response_command(&[0xfc, 0x16, 0x00, 0x01]).expect("cmd");
        assert_eq!(cmd, 0xfc16);
    }

    #[test]
    fn validate_response_command_detects_mismatch() {
        let err = validate_response_command(0xfc0e, &[0xfc, 0x13]).expect_err("must fail");
        match err {
            MscError::CommandMismatch { expected, actual } => {
                assert_eq!(expected, 0xfc0e);
                assert_eq!(actual, 0xfc13);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
