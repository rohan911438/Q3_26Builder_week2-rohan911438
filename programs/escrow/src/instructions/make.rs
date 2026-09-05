use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::{constants::*, error::*, events, state::*};

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct MakeEscrow<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
    )]
    pub maker_ata_a: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = maker,
        space = Escrow::SIZE,
        seeds = [ESCROW_SEED, maker.key().as_ref(), &seed.to_le_bytes()],
        bump,
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(
        init,
        payer = maker,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
    )]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_make(
    ctx: Context<MakeEscrow>,
    seed: u64,
    deposit_amount: u64,
    receive_amount: u64,
    deadline_duration: i64,
) -> Result<()> {
    require!(deposit_amount > 0, EscrowError::InvalidAmount);
    require!(receive_amount > 0, EscrowError::InvalidAmount);
    require!(deadline_duration > 0, EscrowError::InvalidAmount);

    let deadline = Clock::get()?.unix_timestamp + deadline_duration;

    let escrow = &mut ctx.accounts.escrow;
    escrow.seed = seed;
    escrow.maker = ctx.accounts.maker.key();
    escrow.mint_a = ctx.accounts.mint_a.key();
    escrow.mint_b = ctx.accounts.mint_b.key();
    escrow.receive_amount = receive_amount;
    escrow.deadline = deadline;
    escrow.bump = ctx.bumps.escrow;

    let cpi_accounts = TransferChecked {
        from: ctx.accounts.maker_ata_a.to_account_info(),
        mint: ctx.accounts.mint_a.to_account_info(),
        to: ctx.accounts.vault.to_account_info(),
        authority: ctx.accounts.maker.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);
    token::transfer_checked(cpi_ctx, deposit_amount, ctx.accounts.mint_a.decimals)?;

    emit!(events::Make {
        escrow: escrow.key(),
        maker: escrow.maker,
        mint_a: escrow.mint_a,
        mint_b: escrow.mint_b,
        deposit_amount,
        receive_amount,
        deadline,
    });

    Ok(())
}
