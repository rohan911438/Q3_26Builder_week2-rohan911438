use anchor_lang::prelude::*;

#[account]
pub struct Escrow {
    /// Client-chosen nonce so one maker can run multiple concurrent escrows.
    pub seed: u64,
    pub maker: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    /// Amount of mint_b the taker must pay to claim the vaulted mint_a.
    pub receive_amount: u64,
    /// Unix timestamp after which `take` is refused. `refund` remains available to the
    /// maker at any time (before or after); `reclaim` becomes available to anyone once
    /// this has passed. See README for the policy rationale.
    pub deadline: i64,
    pub bump: u8,
}

impl Escrow {
    pub const SIZE: usize = 8 // discriminator
        + 8 // seed
        + 32 // maker
        + 32 // mint_a
        + 32 // mint_b
        + 8 // receive_amount
        + 8 // deadline
        + 1; // bump
}
