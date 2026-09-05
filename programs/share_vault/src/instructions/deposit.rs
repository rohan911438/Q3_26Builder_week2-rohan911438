use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount, TransferChecked};

use crate::{constants::*, error::*, events::*, state::*};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    pub underlying_mint: Box<Account<'info, Mint>>,

    #[account(
        seeds = [VAULT_SEED, underlying_mint.key().as_ref()],
        bump = vault.bump,
        has_one = underlying_mint,
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

    #[account(
        mut,
        associated_token::mint = underlying_mint,
        associated_token::authority = depositor,
    )]
    pub depositor_underlying: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = depositor,
        associated_token::mint = share_mint,
        associated_token::authority = depositor,
    )]
    pub depositor_shares: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [VAULT_SEED, underlying_mint.key().as_ref(), IDLE_SEED],
        bump,
    )]
    pub idle_underlying: Box<Account<'info, TokenAccount>>,

    #[account(
        associated_token::mint = vault.market_mint,
        associated_token::authority = vault_authority,
    )]
    pub market_position: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// New shares are minted pro-rata to the vault's total value (idle + deployed, the
/// latter counted via its 1:1-pegged market_mint receipt balance) at deposit time —
/// the same formula an ERC-4626-style vault uses. The very first deposit sets the
/// initial 1:1 share price.
pub fn handle_deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, ShareVaultError::InvalidAmount);

    let idle_before = ctx.accounts.idle_underlying.amount;
    let market_value_before = ctx.accounts.market_position.amount;
    let total_value_before = idle_before
        .checked_add(market_value_before)
        .ok_or(ShareVaultError::MathOverflow)?;
    let share_supply_before = ctx.accounts.share_mint.supply;

    let shares_to_mint: u64 = if share_supply_before == 0 || total_value_before == 0 {
        amount
    } else {
        let numerator = (amount as u128)
            .checked_mul(share_supply_before as u128)
            .ok_or(ShareVaultError::MathOverflow)?;
        (numerator / total_value_before as u128)
            .try_into()
            .map_err(|_| error!(ShareVaultError::MathOverflow))?
    };
    require!(shares_to_mint > 0, ShareVaultError::InvalidAmount);

    let transfer_cpi = TransferChecked {
        from: ctx.accounts.depositor_underlying.to_account_info(),
        mint: ctx.accounts.underlying_mint.to_account_info(),
        to: ctx.accounts.idle_underlying.to_account_info(),
        authority: ctx.accounts.depositor.to_account_info(),
    };
    token::transfer_checked(
        CpiContext::new(ctx.accounts.token_program.key(), transfer_cpi),
        amount,
        ctx.accounts.underlying_mint.decimals,
    )?;

    let underlying_mint_key = ctx.accounts.underlying_mint.key();
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        underlying_mint_key.as_ref(),
        AUTHORITY_SEED,
        &[ctx.accounts.vault.authority_bump],
    ]];

    let mint_cpi = MintTo {
        mint: ctx.accounts.share_mint.to_account_info(),
        to: ctx.accounts.depositor_shares.to_account_info(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };
    token::mint_to(
        CpiContext::new_with_signer(ctx.accounts.token_program.key(), mint_cpi, signer_seeds),
        shares_to_mint,
    )?;

    emit!(Deposited {
        depositor: ctx.accounts.depositor.key(),
        underlying_amount: amount,
        shares_minted: shares_to_mint,
    });

    Ok(())
}
