use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, TransferChecked};

use crate::{constants::*, error::*, events::*, state::*};

#[derive(Accounts)]
pub struct Redeem<'info> {
    #[account(mut)]
    pub redeemer: Signer<'info>,

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

    #[account(
        mut,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), SHARE_MINT_SEED],
        bump = vault.share_mint_bump,
    )]
    pub share_mint: Box<Account<'info, Mint>>,

    pub market_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = market_mint,
        associated_token::authority = vault_authority,
    )]
    pub market_position: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), IDLE_SEED],
        bump,
    )]
    pub idle_underlying: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = share_mint,
        associated_token::authority = redeemer,
    )]
    pub redeemer_shares: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = redeemer,
        associated_token::mint = underlying_mint,
        associated_token::authority = redeemer,
    )]
    pub redeemer_underlying: Box<Account<'info, TokenAccount>>,

    /// Only funded/used if idle liquidity can't fully cover the redemption.
    #[account(
        init_if_needed,
        payer = redeemer,
        associated_token::mint = market_mint,
        associated_token::authority = redeemer,
    )]
    pub redeemer_market_receipt: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Always lets the redeemer exit, even when the vault's idle liquidity can't fully
/// cover their entitlement: any shortfall is paid out **in-kind** as market_mint
/// receipt tokens (a direct, transferable claim on the vault's deployed position)
/// instead of blocking the redemption or making them wait on a real market's
/// withdrawal window. No admin gate, no lockup — this is what "non-custodial" means
/// here: the vault can never refuse to let a holder cash out their shares.
pub fn handle_redeem(ctx: Context<Redeem>, shares: u64) -> Result<()> {
    require!(shares > 0, ShareVaultError::InvalidAmount);

    let share_supply_before = ctx.accounts.share_mint.supply;
    require!(share_supply_before > 0, ShareVaultError::EmptyVault);

    let idle = ctx.accounts.idle_underlying.amount;
    let market_value = ctx.accounts.market_position.amount;
    let total_value = idle
        .checked_add(market_value)
        .ok_or(ShareVaultError::MathOverflow)?;
    require!(total_value > 0, ShareVaultError::EmptyVault);

    let entitlement: u64 = {
        let numerator = (shares as u128)
            .checked_mul(total_value as u128)
            .ok_or(ShareVaultError::MathOverflow)?;
        (numerator / share_supply_before as u128)
            .try_into()
            .map_err(|_| error!(ShareVaultError::MathOverflow))?
    };
    require!(entitlement > 0, ShareVaultError::InvalidAmount);

    let underlying_paid = entitlement.min(idle);
    let market_receipt_paid = entitlement - underlying_paid;

    let underlying_mint_key = ctx.accounts.underlying_mint.key();
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        underlying_mint_key.as_ref(),
        AUTHORITY_SEED,
        &[ctx.accounts.vault.authority_bump],
    ]];

    // Burn first: a failure partway through the payouts below can't leave shares
    // burned with no corresponding transfer, since the whole instruction reverts.
    let burn_cpi = Burn {
        mint: ctx.accounts.share_mint.to_account_info(),
        from: ctx.accounts.redeemer_shares.to_account_info(),
        authority: ctx.accounts.redeemer.to_account_info(),
    };
    token::burn(CpiContext::new(ctx.accounts.token_program.key(), burn_cpi), shares)?;

    if underlying_paid > 0 {
        let transfer_cpi = TransferChecked {
            from: ctx.accounts.idle_underlying.to_account_info(),
            mint: ctx.accounts.underlying_mint.to_account_info(),
            to: ctx.accounts.redeemer_underlying.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        };
        token::transfer_checked(
            CpiContext::new_with_signer(ctx.accounts.token_program.key(), transfer_cpi, signer_seeds),
            underlying_paid,
            ctx.accounts.underlying_mint.decimals,
        )?;
    }

    if market_receipt_paid > 0 {
        let transfer_cpi = TransferChecked {
            from: ctx.accounts.market_position.to_account_info(),
            mint: ctx.accounts.market_mint.to_account_info(),
            to: ctx.accounts.redeemer_market_receipt.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        };
        token::transfer_checked(
            CpiContext::new_with_signer(ctx.accounts.token_program.key(), transfer_cpi, signer_seeds),
            market_receipt_paid,
            ctx.accounts.market_mint.decimals,
        )?;
    }

    emit!(Redeemed {
        redeemer: ctx.accounts.redeemer.key(),
        shares_burned: shares,
        underlying_paid,
        market_receipt_paid,
    });

    Ok(())
}
