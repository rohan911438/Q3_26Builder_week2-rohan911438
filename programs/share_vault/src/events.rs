use anchor_lang::prelude::*;

#[event]
pub struct VaultInitialized {
    pub vault: Pubkey,
    pub underlying_mint: Pubkey,
    pub share_mint: Pubkey,
    pub market_mint: Pubkey,
}

#[event]
pub struct Deposited {
    pub depositor: Pubkey,
    pub underlying_amount: u64,
    pub shares_minted: u64,
}

#[event]
pub struct Redeemed {
    pub redeemer: Pubkey,
    pub shares_burned: u64,
    pub underlying_paid: u64,
    pub market_receipt_paid: u64,
}

#[event]
pub struct DeployedToMarket {
    pub amount: u64,
}

#[event]
pub struct WithdrawnFromMarket {
    pub amount: u64,
}
