use anchor_lang::prelude::*;

use crate::{constants::*, error::*, events, state::*};

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct UpdateEscrow<'info> {
    pub maker: Signer<'info>,

    #[account(
        mut,
        has_one = maker @ EscrowError::Unauthorized,
        seeds = [ESCROW_SEED, maker.key().as_ref(), &seed.to_le_bytes()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,
}

/// Only terms of an escrow that has not yet been taken can be updated: once
/// `take` or `refund` runs, the escrow account is closed, so any later call
/// here fails with an account-not-found/owner error rather than a custom one.
pub fn handle_update(
    ctx: Context<UpdateEscrow>,
    _seed: u64,
    new_mint_b: Pubkey,
    new_receive_amount: u64,
) -> Result<()> {
    require!(new_receive_amount > 0, EscrowError::InvalidAmount);

    let escrow = &mut ctx.accounts.escrow;
    escrow.mint_b = new_mint_b;
    escrow.receive_amount = new_receive_amount;

    emit!(events::Update {
        escrow: escrow.key(),
        maker: escrow.maker,
        mint_b: new_mint_b,
        receive_amount: new_receive_amount,
    });

    Ok(())
}
