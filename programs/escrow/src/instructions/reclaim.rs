use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, Mint, Token, TokenAccount, TransferChecked};

use crate::{constants::*, error::*, events, state::*};

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct ReclaimEscrow<'info> {
    /// Anyone may call this; they only pay the transaction fee. Funds always return to
    /// the maker regardless of who signs.
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(mut)]
    pub maker: SystemAccount<'info>,

    pub mint_a: Box<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
    )]
    pub maker_ata_a: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        close = maker,
        has_one = maker @ EscrowError::Unauthorized,
        has_one = mint_a,
        seeds = [ESCROW_SEED, maker.key().as_ref(), &seed.to_le_bytes()],
        bump = escrow.bump,
    )]
    pub escrow: Box<Account<'info, Escrow>>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Permissionless cleanup for an expired, un-taken escrow: identical effect to
/// `refund`, but callable by anyone once `escrow.deadline` has passed rather than
/// only by the maker. Lets a third party (or a keeper/crank) unwind an abandoned
/// escrow without waiting on the maker.
pub fn handle_reclaim(ctx: Context<ReclaimEscrow>, seed: u64) -> Result<()> {
    require!(
        Clock::get()?.unix_timestamp > ctx.accounts.escrow.deadline,
        EscrowError::EscrowNotExpired
    );

    let deposit_amount = ctx.accounts.vault.amount;
    let maker_key = ctx.accounts.escrow.maker;

    let signer_seeds: &[&[&[u8]]] = &[&[
        ESCROW_SEED,
        maker_key.as_ref(),
        &seed.to_le_bytes(),
        &[ctx.accounts.escrow.bump],
    ]];

    let transfer_cpi_accounts = TransferChecked {
        from: ctx.accounts.vault.to_account_info(),
        mint: ctx.accounts.mint_a.to_account_info(),
        to: ctx.accounts.maker_ata_a.to_account_info(),
        authority: ctx.accounts.escrow.to_account_info(),
    };
    let transfer_cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        transfer_cpi_accounts,
        signer_seeds,
    );
    token::transfer_checked(transfer_cpi_ctx, deposit_amount, ctx.accounts.mint_a.decimals)?;

    let close_cpi_accounts = CloseAccount {
        account: ctx.accounts.vault.to_account_info(),
        destination: ctx.accounts.maker.to_account_info(),
        authority: ctx.accounts.escrow.to_account_info(),
    };
    let close_cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        close_cpi_accounts,
        signer_seeds,
    );
    token::close_account(close_cpi_ctx)?;

    emit!(events::Reclaim {
        escrow: ctx.accounts.escrow.key(),
        maker: maker_key,
        caller: ctx.accounts.caller.key(),
        deposit_amount,
    });

    Ok(())
}
