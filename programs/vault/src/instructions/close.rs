use anchor_lang::prelude::*;

use crate::{constants::*, error::*, events::*, state::*};

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ VaultError::Unauthorized,
        close = owner,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        seeds = [VAULT_SEED, owner.key().as_ref(), VAULT_AUTHORITY_SEED],
        bump = vault.vault_bump,
    )]
    pub vault_authority: SystemAccount<'info>,
}

/// Refuses to close a vault that still holds deposited funds rather than
/// silently sweeping them to the owner on close — see README for rationale.
pub fn handle_close(ctx: Context<Close>) -> Result<()> {
    require!(
        ctx.accounts.vault.total_deposited == 0,
        VaultError::VaultNotEmpty
    );

    emit!(VaultClosed {
        owner: ctx.accounts.owner.key(),
    });

    Ok(())
}
