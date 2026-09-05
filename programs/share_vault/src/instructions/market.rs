use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, TransferChecked};

use crate::{constants::*, error::*, events::*, state::*};

/// Both instructions here are permissionless: they only ever move the vault's own
/// funds between its own accounts (idle <-> mock market), 1:1, so there is nothing
/// for a caller to steal or grief by triggering them. A real integration would swap
/// these for CPIs into an actual lending/market program instead of the mock mint.
#[derive(Accounts)]
pub struct DeployToMarket<'info> {
    pub caller: Signer<'info>,

    pub underlying_mint: Box<Account<'info, Mint>>,

    #[account(
        seeds = [VAULT_SEED, underlying_mint.key().as_ref()],
        bump = vault.bump,
        has_one = underlying_mint,
        has_one = market_mint,
    )]
    pub vault: Box<Account<'info, ShareVault>>,

    #[account(
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), AUTHORITY_SEED],
        bump = vault.authority_bump,
    )]
    pub vault_authority: SystemAccount<'info>,

    #[account(mut)]
    pub market_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), IDLE_SEED],
        bump,
    )]
    pub idle_underlying: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), MARKET_CUSTODY_SEED],
        bump,
    )]
    pub market_custody: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = market_mint,
        associated_token::authority = vault_authority,
    )]
    pub market_position: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_deploy_to_market(ctx: Context<DeployToMarket>, amount: u64) -> Result<()> {
    require!(amount > 0, ShareVaultError::InvalidAmount);
    require!(
        ctx.accounts.idle_underlying.amount >= amount,
        ShareVaultError::InsufficientIdleLiquidity
    );

    let underlying_mint_key = ctx.accounts.underlying_mint.key();
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        underlying_mint_key.as_ref(),
        AUTHORITY_SEED,
        &[ctx.accounts.vault.authority_bump],
    ]];

    let transfer_cpi = TransferChecked {
        from: ctx.accounts.idle_underlying.to_account_info(),
        mint: ctx.accounts.underlying_mint.to_account_info(),
        to: ctx.accounts.market_custody.to_account_info(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };
    token::transfer_checked(
        CpiContext::new_with_signer(ctx.accounts.token_program.key(), transfer_cpi, signer_seeds),
        amount,
        ctx.accounts.underlying_mint.decimals,
    )?;

    let mint_cpi = MintTo {
        mint: ctx.accounts.market_mint.to_account_info(),
        to: ctx.accounts.market_position.to_account_info(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };
    token::mint_to(
        CpiContext::new_with_signer(ctx.accounts.token_program.key(), mint_cpi, signer_seeds),
        amount,
    )?;

    emit!(DeployedToMarket { amount });
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawFromMarket<'info> {
    pub caller: Signer<'info>,

    pub underlying_mint: Box<Account<'info, Mint>>,

    #[account(
        seeds = [VAULT_SEED, underlying_mint.key().as_ref()],
        bump = vault.bump,
        has_one = underlying_mint,
        has_one = market_mint,
    )]
    pub vault: Box<Account<'info, ShareVault>>,

    #[account(
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), AUTHORITY_SEED],
        bump = vault.authority_bump,
    )]
    pub vault_authority: SystemAccount<'info>,

    #[account(mut)]
    pub market_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = market_mint,
        associated_token::authority = vault_authority,
    )]
    pub market_position: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), MARKET_CUSTODY_SEED],
        bump,
    )]
    pub market_custody: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), IDLE_SEED],
        bump,
    )]
    pub idle_underlying: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_withdraw_from_market(ctx: Context<WithdrawFromMarket>, amount: u64) -> Result<()> {
    require!(amount > 0, ShareVaultError::InvalidAmount);
    require!(
        ctx.accounts.market_position.amount >= amount,
        ShareVaultError::InsufficientMarketPosition
    );

    let underlying_mint_key = ctx.accounts.underlying_mint.key();
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        underlying_mint_key.as_ref(),
        AUTHORITY_SEED,
        &[ctx.accounts.vault.authority_bump],
    ]];

    let burn_cpi = Burn {
        mint: ctx.accounts.market_mint.to_account_info(),
        from: ctx.accounts.market_position.to_account_info(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };
    token::burn(
        CpiContext::new_with_signer(ctx.accounts.token_program.key(), burn_cpi, signer_seeds),
        amount,
    )?;

    let transfer_cpi = TransferChecked {
        from: ctx.accounts.market_custody.to_account_info(),
        mint: ctx.accounts.underlying_mint.to_account_info(),
        to: ctx.accounts.idle_underlying.to_account_info(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };
    token::transfer_checked(
        CpiContext::new_with_signer(ctx.accounts.token_program.key(), transfer_cpi, signer_seeds),
        amount,
        ctx.accounts.underlying_mint.decimals,
    )?;

    emit!(WithdrawnFromMarket { amount });
    Ok(())
}
