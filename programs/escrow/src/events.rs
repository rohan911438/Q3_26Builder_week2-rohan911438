use anchor_lang::prelude::*;

#[event]
pub struct Make {
    pub escrow: Pubkey,
    pub maker: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub deposit_amount: u64,
    pub receive_amount: u64,
}

#[event]
pub struct Take {
    pub escrow: Pubkey,
    pub maker: Pubkey,
    pub taker: Pubkey,
    pub deposit_amount: u64,
    pub receive_amount: u64,
}

#[event]
pub struct Refund {
    pub escrow: Pubkey,
    pub maker: Pubkey,
    pub deposit_amount: u64,
}

#[event]
pub struct Update {
    pub escrow: Pubkey,
    pub maker: Pubkey,
    pub mint_b: Pubkey,
    pub receive_amount: u64,
}
