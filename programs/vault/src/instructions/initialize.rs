use anchor_lang::prelude::*;

use crate::{constants::*, events::*, state::*};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = Vault::SIZE,
        seeds = [VAULT_SEED, owner.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        seeds = [VAULT_SEED, owner.key().as_ref(), VAULT_AUTHORITY_SEED],
        bump,
    )]
    pub vault_authority: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_initialize(ctx: Context<Initialize>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.owner = ctx.accounts.owner.key();
    vault.bump = ctx.bumps.vault;
    vault.vault_bump = ctx.bumps.vault_authority;
    vault.total_deposited = 0;
    vault.locked = false;

    emit!(VaultInitialized {
        owner: vault.owner,
        vault: vault.key(),
    });

    Ok(())
}
