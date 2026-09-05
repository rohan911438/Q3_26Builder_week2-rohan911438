pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("8L1kYmbBfaneysWGZqDReecZiW2zF1yas2V2yfKosHo6");

#[program]
pub mod share_vault {
    use super::*;

    pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
        instructions::initialize::handle_initialize(ctx)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        instructions::deposit::handle_deposit(ctx, amount)
    }

    pub fn redeem(ctx: Context<Redeem>, shares: u64) -> Result<()> {
        instructions::redeem::handle_redeem(ctx, shares)
    }

    pub fn deploy_to_market(ctx: Context<DeployToMarket>, amount: u64) -> Result<()> {
        instructions::market::handle_deploy_to_market(ctx, amount)
    }

    pub fn withdraw_from_market(ctx: Context<WithdrawFromMarket>, amount: u64) -> Result<()> {
        instructions::market::handle_withdraw_from_market(ctx, amount)
    }
}
