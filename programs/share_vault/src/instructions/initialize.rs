use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{constants::*, events::*, state::*};

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub underlying_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = ShareVault::SIZE,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, ShareVault>,

    /// PDA that owns every token account and mint below; holds no data of its own.
    #[account(
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), AUTHORITY_SEED],
        bump,
    )]
    pub vault_authority: SystemAccount<'info>,

    #[account(
        init,
        payer = payer,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), SHARE_MINT_SEED],
        bump,
        mint::decimals = underlying_mint.decimals,
        mint::authority = vault_authority,
    )]
    pub share_mint: Account<'info, Mint>,

    /// Mock receipt token standing in for a real external market/lending position.
    #[account(
        init,
        payer = payer,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), MARKET_MINT_SEED],
        bump,
        mint::decimals = underlying_mint.decimals,
        mint::authority = vault_authority,
    )]
    pub market_mint: Account<'info, Mint>,

    /// Holds underlying tokens not currently deployed to the mock market.
    #[account(
        init,
        payer = payer,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), IDLE_SEED],
        bump,
        token::mint = underlying_mint,
        token::authority = vault_authority,
    )]
    pub idle_underlying: Account<'info, TokenAccount>,

    /// Mock "market": underlying tokens that have been deployed sit here instead of
    /// idle_underlying, standing in for a real external protocol's custody account.
    #[account(
        init,
        payer = payer,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), MARKET_CUSTODY_SEED],
        bump,
        token::mint = underlying_mint,
        token::authority = vault_authority,
    )]
    pub market_custody: Account<'info, TokenAccount>,

    /// Holds the market_mint receipts the vault has been issued for deployed capital.
    #[account(
        init,
        payer = payer,
        associated_token::mint = market_mint,
        associated_token::authority = vault_authority,
    )]
    pub market_position: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_initialize(ctx: Context<InitializeVault>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.underlying_mint = ctx.accounts.underlying_mint.key();
    vault.share_mint = ctx.accounts.share_mint.key();
    vault.market_mint = ctx.accounts.market_mint.key();
    vault.bump = ctx.bumps.vault;
    vault.authority_bump = ctx.bumps.vault_authority;
    vault.share_mint_bump = ctx.bumps.share_mint;
    vault.market_mint_bump = ctx.bumps.market_mint;

    emit!(VaultInitialized {
        vault: vault.key(),
        underlying_mint: vault.underlying_mint,
        share_mint: vault.share_mint,
        market_mint: vault.market_mint,
    });

    Ok(())
}
