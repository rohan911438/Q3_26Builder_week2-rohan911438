use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

use crate::{constants::*, error::*, events::*, state::*};

#[derive(Accounts)]
pub struct Withdraw<'info> {
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

pub fn handle_withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::InvalidAmount);
    require!(
        ctx.accounts.vault.total_deposited >= amount,
        VaultError::InsufficientFunds
    );

    let owner_key = ctx.accounts.vault.owner;
    let vault_bump = ctx.accounts.vault.vault_bump;
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        owner_key.as_ref(),
        VAULT_AUTHORITY_SEED,
        &[vault_bump],
    ]];

    let cpi_accounts = Transfer {
        from: ctx.accounts.vault_authority.to_account_info(),
        to: ctx.accounts.owner.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.system_program.key(),
        cpi_accounts,
        signer_seeds,
    );
    system_program::transfer(cpi_ctx, amount)?;

    let vault = &mut ctx.accounts.vault;
    vault.total_deposited -= amount;

    emit!(Withdrawn {
        owner: vault.owner,
        amount,
        new_total: vault.total_deposited,
    });

    Ok(())
}
