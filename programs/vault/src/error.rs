use anchor_lang::prelude::*;

#[error_code]
pub enum VaultError {
    #[msg("Only the vault owner may perform this action.")]
    Unauthorized,
    #[msg("The vault does not hold enough funds to cover this withdrawal.")]
    InsufficientFunds,
    #[msg("The vault still holds funds; withdraw everything before closing.")]
    VaultNotEmpty,
    #[msg("Deposit/withdraw amount must be greater than zero.")]
    InvalidAmount,
}
