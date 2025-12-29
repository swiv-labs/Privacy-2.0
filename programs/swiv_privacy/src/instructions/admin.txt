use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

use crate::{
    errors::ErrorCode,
    events::{
        AdminTransferredEvent, PoolCreatedEvent, PoolFinalizedEvent, ProtocolFeeUpdatedEvent,
    },
    state::{Pool, PoolStatus, ProtocolState},
};

/// Initialize the protocol state
pub fn initialize(ctx: Context<Initialize>, protocol_fee_bps: u16) -> Result<()> {
    require!(protocol_fee_bps <= 1000, ErrorCode::InvalidFee);

    let protocol_state = &mut ctx.accounts.protocol_state;
    protocol_state.admin = ctx.accounts.admin.key();
    protocol_state.protocol_fee_bps = protocol_fee_bps;
    protocol_state.total_pools_created = 0;
    protocol_state.bump = ctx.bumps.protocol_state;

    Ok(())
}

/// Transfer admin authority to a new address
pub fn transfer_admin(ctx: Context<TransferAdmin>, new_admin: Pubkey) -> Result<()> {
    require!(new_admin != Pubkey::default(), ErrorCode::InvalidNewAdmin);

    let old_admin = ctx.accounts.protocol_state.admin;
    ctx.accounts.protocol_state.admin = new_admin;

    emit!(AdminTransferredEvent {
        old_admin,
        new_admin,
    });

    Ok(())
}

/// Update the protocol fee basis points
pub fn update_protocol_fee(ctx: Context<UpdateProtocolFee>, new_fee_bps: u16) -> Result<()> {
    require!(new_fee_bps <= 1000, ErrorCode::InvalidFee);

    let old_fee_bps = ctx.accounts.protocol_state.protocol_fee_bps;
    ctx.accounts.protocol_state.protocol_fee_bps = new_fee_bps;

    emit!(ProtocolFeeUpdatedEvent {
        old_fee_bps,
        new_fee_bps,
    });

    Ok(())
}

/// Create a new prediction pool
pub fn create_pool(
    ctx: Context<CreatePool>,
    pool_id: u64,
    asset_symbol: String,
    entry_fee: u64,
    target_timestamp: i64,
    max_participants: u32,
) -> Result<()> {
    require!(asset_symbol.len() <= 10, ErrorCode::AssetSymbolTooLong);
    require!(entry_fee > 0, ErrorCode::InvalidEntryFee);
    require!(
        target_timestamp > Clock::get()?.unix_timestamp,
        ErrorCode::InvalidTimestamp
    );
    require!(max_participants > 0, ErrorCode::InvalidMaxParticipants);

    let pool = &mut ctx.accounts.pool;
    pool.pool_id = pool_id;
    pool.admin = ctx.accounts.admin.key();
    pool.asset_symbol = asset_symbol.clone();
    pool.entry_fee = entry_fee;
    pool.target_timestamp = target_timestamp;
    pool.max_participants = max_participants;
    pool.total_participants = 0;
    pool.total_pool_amount = 0;
    pool.status = PoolStatus::Active;
    pool.actual_price = 0;
    pool.bump = ctx.bumps.pool;
    pool.vault_bump = ctx.bumps.pool_vault;

    let protocol_state = &mut ctx.accounts.protocol_state;
    protocol_state.total_pools_created += 1;

    emit!(PoolCreatedEvent {
        pool_id,
        asset_symbol,
        entry_fee,
        target_timestamp,
        max_participants,
    });

    Ok(())
}

/// Finalize pool with actual price from oracle
pub fn finalize_pool(ctx: Context<FinalizePool>, actual_price: u64) -> Result<()> {
    let pool = &mut ctx.accounts.pool;

    require!(pool.status == PoolStatus::Active, ErrorCode::PoolNotActive);
    require!(
        Clock::get()?.unix_timestamp >= pool.target_timestamp,
        ErrorCode::PredictionTimeNotReached
    );
    require!(actual_price > 0, ErrorCode::InvalidActualPrice);

    pool.status = PoolStatus::Finalized;
    pool.actual_price = actual_price;

    emit!(PoolFinalizedEvent {
        pool_id: pool.pool_id,
        actual_price,
        total_pool_amount: pool.total_pool_amount,
    });

    Ok(())
}

/// Admin function to close a pool (emergency only)
pub fn close_pool(ctx: Context<ClosePool>) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    pool.status = PoolStatus::Closed;

    Ok(())
}

// ============================================================================
// Context Structs
// ============================================================================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = ProtocolState::LEN,
        seeds = [b"protocol_state"],
        bump
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TransferAdmin<'info> {
    #[account(
        mut,
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = protocol_state.admin == admin.key() @ ErrorCode::Unauthorized
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateProtocolFee<'info> {
    #[account(
        mut,
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = protocol_state.admin == admin.key() @ ErrorCode::Unauthorized
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(pool_id: u64)]
pub struct CreatePool<'info> {
    #[account(
        mut,
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = protocol_state.admin == admin.key() @ ErrorCode::Unauthorized
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        init,
        payer = admin,
        space = Pool::LEN,
        seeds = [b"pool", pool_id.to_le_bytes().as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        init,
        payer = admin,
        seeds = [b"pool_vault", pool_id.to_le_bytes().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = pool,
    )]
    pub pool_vault: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, token::Mint>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct FinalizePool<'info> {
    #[account(
        mut,
        seeds = [b"pool", pool.pool_id.to_le_bytes().as_ref()],
        bump = pool.bump,
        constraint = pool.admin == admin.key() @ ErrorCode::Unauthorized
    )]
    pub pool: Account<'info, Pool>,

    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClosePool<'info> {
    #[account(
        mut,
        seeds = [b"pool", pool.pool_id.to_le_bytes().as_ref()],
        bump = pool.bump,
        constraint = pool.admin == admin.key() @ ErrorCode::Unauthorized
    )]
    pub pool: Account<'info, Pool>,

    pub admin: Signer<'info>,
}