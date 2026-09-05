use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, CloseAccount, Mint, Token, TokenAccount, TransferChecked};

use crate::{constants::*, error::*, events, state::*};

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct TakeEscrow<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,

    #[account(mut)]
    pub maker: SystemAccount<'info>,

    pub mint_a: Box<Account<'info, Mint>>,
    pub mint_b: Box<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint_b,
        associated_token::authority = taker,
    )]
    pub taker_ata_b: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = taker,
        associated_token::mint = mint_a,
        associated_token::authority = taker,
    )]
    pub taker_ata_a: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = taker,
        associated_token::mint = mint_b,
        associated_token::authority = maker,
    )]
    pub maker_ata_b: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        close = maker,
        has_one = maker @ EscrowError::Unauthorized,
        has_one = mint_a,
        has_one = mint_b,
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
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_take(ctx: Context<TakeEscrow>, seed: u64) -> Result<()> {
    let receive_amount = ctx.accounts.escrow.receive_amount;
    let deposit_amount = ctx.accounts.vault.amount;
    let maker_key = ctx.accounts.escrow.maker;

    // Taker pays the maker in mint_b.
    let pay_cpi_accounts = TransferChecked {
        from: ctx.accounts.taker_ata_b.to_account_info(),
        mint: ctx.accounts.mint_b.to_account_info(),
        to: ctx.accounts.maker_ata_b.to_account_info(),
        authority: ctx.accounts.taker.to_account_info(),
    };
    let pay_cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.key(),
        pay_cpi_accounts,
    );
    token::transfer_checked(pay_cpi_ctx, receive_amount, ctx.accounts.mint_b.decimals)?;

    // Escrow PDA releases the vaulted mint_a to the taker.
    let signer_seeds: &[&[&[u8]]] = &[&[
        ESCROW_SEED,
        maker_key.as_ref(),
        &seed.to_le_bytes(),
        &[ctx.accounts.escrow.bump],
    ]];

    let release_cpi_accounts = TransferChecked {
        from: ctx.accounts.vault.to_account_info(),
        mint: ctx.accounts.mint_a.to_account_info(),
        to: ctx.accounts.taker_ata_a.to_account_info(),
        authority: ctx.accounts.escrow.to_account_info(),
    };
    let release_cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        release_cpi_accounts,
        signer_seeds,
    );
    token::transfer_checked(release_cpi_ctx, deposit_amount, ctx.accounts.mint_a.decimals)?;

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

    emit!(events::Take {
        escrow: ctx.accounts.escrow.key(),
        maker: maker_key,
        taker: ctx.accounts.taker.key(),
        deposit_amount,
        receive_amount,
    });

    Ok(())
}
