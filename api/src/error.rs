use steel::*;

#[repr(u32)]
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
pub enum SpoolError {
    #[error("Unknown error")]
    UnknownError = 0,

    #[error("The provided spool is in an unexpected state")]
    UnexpectedState         = 0x10,
    #[error("The spool write failed")]
    WriteFailed             = 0x11,
    #[error("The spool is too long")]
    SpoolTooLong            = 0x12,
    #[error("The spool does not have enough rent")]
    InsufficientRent        = 0x13,

    #[error("The provided hash is invalid")]
    SolutionInvalid         = 0x20,
    #[error("The provided spool doesn't match the expected spool")]
    UnexpectedSpool         = 0x21,
    #[error("The provided hash did not satisfy the minimum required difficulty")]
    SolutionTooEasy         = 0x22,
    #[error("The provided solution is too early")]
    SolutionTooEarly        = 0x23,
    #[error("The provided claim is too large")]
    ClaimTooLarge           = 0x24,
    #[error("Computed commitment does not match the miner commitment")]
    CommitmentMismatch      = 0x25,

    #[error("Failed to pack the spool into the reel")]
    ReelPackFailed          = 0x30,
    #[error("Failed to unpack the spool from the reel")]
    ReelUnpackFailed        = 0x31,
    #[error("Too many spools in the reel")]
    ReelTooManySpools       = 0x32,
    #[error("Reel commit failed")]
    ReelCommitFailed        = 0x33,
}

error!(SpoolError);
