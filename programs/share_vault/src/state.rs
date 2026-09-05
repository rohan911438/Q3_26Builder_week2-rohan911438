use anchor_lang::prelude::*;

/// One ShareVault instance per underlying mint. Holds no user funds itself — the actual
/// tokens live in ATAs owned by the `vault_authority` PDA; this account is just routing
/// metadata plus the PDA bumps needed to re-derive/sign for those ATAs and mints.
#[account]
pub struct ShareVault {
    pub underlying_mint: Pubkey,
    pub share_mint: Pubkey,
    pub market_mint: Pubkey,
    pub bump: u8,
    pub authority_bump: u8,
    pub share_mint_bump: u8,
    pub market_mint_bump: u8,
}

impl ShareVault {
    pub const SIZE: usize = 8 // discriminator
        + 32 // underlying_mint
        + 32 // share_mint
        + 32 // market_mint
        + 1 // bump
        + 1 // authority_bump
        + 1 // share_mint_bump
        + 1; // market_mint_bump
}
