use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Deposit and receive amounts must be greater than zero.")]
    InvalidAmount,
    #[msg("Only the escrow maker may perform this action.")]
    Unauthorized,
    #[msg("This escrow's deadline has passed; it can no longer be taken.")]
    EscrowExpired,
    #[msg("This escrow has not expired yet; only the maker can refund it before the deadline.")]
    EscrowNotExpired,
}
