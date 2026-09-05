use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Deposit and receive amounts must be greater than zero.")]
    InvalidAmount,
    #[msg("Only the escrow maker may perform this action.")]
    Unauthorized,
}
