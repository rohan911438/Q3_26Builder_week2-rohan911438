pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("BChAgZDNqUcunsKRU1UD2jYCvxj1PQ7Sb93bMg53ar7t");

#[program]
pub mod escrow {
    use super::*;

    pub fn make(
        ctx: Context<MakeEscrow>,
        seed: u64,
        deposit_amount: u64,
        receive_amount: u64,
    ) -> Result<()> {
        instructions::make::handle_make(ctx, seed, deposit_amount, receive_amount)
    }

    pub fn take(ctx: Context<TakeEscrow>, seed: u64) -> Result<()> {
        instructions::take::handle_take(ctx, seed)
    }

    pub fn refund(ctx: Context<RefundEscrow>, seed: u64) -> Result<()> {
        instructions::refund::handle_refund(ctx, seed)
    }

    pub fn update(
        ctx: Context<UpdateEscrow>,
        seed: u64,
        new_mint_b: Pubkey,
        new_receive_amount: u64,
    ) -> Result<()> {
        instructions::update::handle_update(ctx, seed, new_mint_b, new_receive_amount)
    }
}
