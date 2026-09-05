use anchor_lang::prelude::*;

#[error_code]
pub enum ShareVaultError {
    #[msg("Amount must be greater than zero.")]
    InvalidAmount,
    #[msg("Arithmetic overflow while computing shares or entitlement.")]
    MathOverflow,
    #[msg("The vault has no value to redeem shares against.")]
    EmptyVault,
    #[msg("The vault does not hold enough deployed capital to withdraw that amount.")]
    InsufficientMarketPosition,
    #[msg("The vault does not hold enough idle liquidity to deploy that amount.")]
    InsufficientIdleLiquidity,
}
