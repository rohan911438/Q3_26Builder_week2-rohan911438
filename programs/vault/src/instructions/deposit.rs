use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

use crate::{constants::*, error::*, events::*, state::*};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ VaultError::Unauthorized,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        mut,
        seeds = [VAULT_SEED, owner.key().as_ref(), VAULT_AUTHORITY_SEED],
        bump = vault.vault_bump,
    )]
    pub vault_authority: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Deposits are restricted to the vault owner (a simple, single-depositor custodial
/// vault). See README for the rationale and how this differs from a pooled vault.
pub fn handle_deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::InvalidAmount);

    let cpi_accounts = Transfer {
        from: ctx.accounts.owner.to_account_info(),
        to: ctx.accounts.vault_authority.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.system_program.key(), cpi_accounts);
    system_program::transfer(cpi_ctx, amount)?;

    let vault = &mut ctx.accounts.vault;
    vault.total_deposited = vault
        .total_deposited
        .checked_add(amount)
        .ok_or(VaultError::InvalidAmount)?;

    emit!(Deposited {
        owner: vault.owner,
        amount,
        new_total: vault.total_deposited,
    });

    Ok(())
}
