use anchor_lang::prelude::*;

#[account]
pub struct Vault {
    /// The wallet that owns this vault and is the only party allowed to deposit/withdraw/close.
    pub owner: Pubkey,
    /// Bump for the Vault state PDA (seeds: ["vault", owner]).
    pub bump: u8,
    /// Bump for the vault authority PDA that actually holds the lamports
    /// (seeds: ["vault", owner, "vault_authority"]).
    pub vault_bump: u8,
    /// Internal accounting of how much the owner has deposited and not yet withdrawn.
    /// Kept separate from the authority PDA's raw lamport balance so rent-exempt
    /// minimums on the authority account never get confused with user funds.
    pub total_deposited: u64,
    /// Reserved for future use (e.g. the timed-escrow / non-custodial extensions).
    pub locked: bool,
}

impl Vault {
    pub const SIZE: usize = 8 // discriminator
        + 32 // owner
        + 1 // bump
        + 1 // vault_bump
        + 8 // total_deposited
        + 1; // locked
}
