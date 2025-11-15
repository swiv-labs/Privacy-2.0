#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use arcium_anchor::prelude::*;
use arcium_client::idl::arcium::types::{CircuitSource, OffChainCircuitSource};

const COMP_DEF_OFFSET_PROCESS_BET: u32 = comp_def_offset("process_bet");
const COMP_DEF_OFFSET_CALCULATE_REWARD: u32 = comp_def_offset("calculate_reward_v2");

declare_id!("8D6DiY4fWkyJ2QicNacEJFoA4cNaCfbs9r215oGLxW73");

#[arcium_program]
pub mod swiv_privacy {
    use arcium_client::idl::arcium::types::CallbackAccount;

    use super::*;

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

    // ============ ARCIUM ENCRYPTED BET INSTRUCTIONS ============

    /// Initialize computation definition for process_bet
    pub fn init_process_bet_comp_def(ctx: Context<InitProcessBetCompDef>) -> Result<()> {
        init_comp_def(
          ctx.accounts,
          0,
          Some(CircuitSource::OffChain(OffChainCircuitSource {
            source: "https://bvcykkwsaifzcwtuhhrt.supabase.co/storage/v1/object/public/arcium/process_bet.arcis".to_string(),
            hash: [0; 32], 
          })),
          None,
        )?;
        Ok(())
    }

    /// Place an encrypted bet
    pub fn place_encrypted_bet(
        ctx: Context<PlaceEncryptedBet>,
        computation_offset: u64,
        ciphertext_price: [u8; 32],
        pub_key: [u8; 32],
        nonce: u128,
    ) -> Result<()> {
        let pool_key = ctx.accounts.pool.key();

        {
            let pool = &mut ctx.accounts.pool;

            require!(pool.status == PoolStatus::Active, ErrorCode::PoolNotActive);
            require!(
                Clock::get()?.unix_timestamp < pool.target_timestamp,
                ErrorCode::PredictionTimePassed
            );
            require!(
                pool.total_participants < pool.max_participants,
                ErrorCode::PoolFull
            );

            // Store entry fee
            let entry_fee = pool.entry_fee;
            let cpi_accounts = Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.pool_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

            token::transfer(cpi_ctx, entry_fee)?;

            msg!("Token transferred successfully");

            let bet = &mut ctx.accounts.bet;
            bet.user = ctx.accounts.user.key();
            bet.pool = pool_key;
            bet.encrypted_predicted_price = ciphertext_price;
            bet.pub_key = pub_key;
            bet.nonce = nonce;
            bet.stake_amount = entry_fee;
            bet.claimed = false;
            bet.bump = ctx.bumps.bet;

            pool.total_participants += 1;
            pool.total_pool_amount += entry_fee;
        }

        ctx.accounts.sign_pda_account.bump = ctx.bumps.sign_pda_account;

        let args = vec![
            Argument::ArcisPubkey(pub_key),
            Argument::PlaintextU128(nonce),
            Argument::EncryptedU8(ciphertext_price),
        ];

        queue_computation(
            ctx.accounts,
            computation_offset,
            args,
            None,
            vec![ProcessBetCallback::callback_ix(&[])],
            1,
        )?;

        emit!(EncryptedBetPlacedEvent {
            pool_id: ctx.accounts.pool.pool_id,
            user: ctx.accounts.user.key(),
            stake_amount: ctx.accounts.pool.entry_fee,
        });

        msg!("=== place_encrypted_bet END ===");
        Ok(())
    }

    /// Callback for encrypted bet processing
    #[arcium_callback(encrypted_ix = "process_bet")]
    pub fn process_bet_callback(
        ctx: Context<ProcessBetCallback>,
        output: ComputationOutputs<ProcessBetOutput>,
    ) -> Result<()> {
        msg!("=== computation START ===");
        let o = match output {
            ComputationOutputs::Success(ProcessBetOutput { field_0 }) => field_0,
            _ => return Err(ErrorCode::AbortedComputation.into()),
        };

        emit!(BetProcessedEvent { success: o });

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

    // ============ ARCIUM REWARD CALCULATION ============

    /// Initialize computation definition for calculate_reward
    pub fn init_calculate_reward_comp_def(ctx: Context<InitCalculateRewardCompDef>) -> Result<()> {
        init_comp_def(
          ctx.accounts,
          0,
          Some(CircuitSource::OffChain(OffChainCircuitSource {
            source: "https://bvcykkwsaifzcwtuhhrt.supabase.co/storage/v1/object/public/arcium/calculate_reward_v2.arcis".to_string(),
            hash: [0; 32], 
          })),
          None,
        )?;
        Ok(())
    }

    /// Calculate reward using encrypted computation
    pub fn calculate_reward(ctx: Context<CalculateReward>, computation_offset: u64) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let bet = &ctx.accounts.bet;

    require!(
        pool.status == PoolStatus::Finalized,
        ErrorCode::PoolNotFinalized
    );
    require!(!bet.claimed, ErrorCode::AlreadyClaimed);
    require!(bet.user == ctx.accounts.user.key(), ErrorCode::Unauthorized);

    ctx.accounts.sign_pda_account.bump = ctx.bumps.sign_pda_account;

    let args = vec![
        Argument::ArcisPubkey(bet.pub_key),
        Argument::PlaintextU128(bet.nonce),
        Argument::EncryptedU8(bet.encrypted_predicted_price),  
        Argument::PlaintextU64(pool.actual_price),              
        Argument::PlaintextU64(pool.total_pool_amount),         
        Argument::PlaintextU16(ctx.accounts.protocol_state.protocol_fee_bps),
    ];

    // Pass ALL accounts needed by the callback
    queue_computation(
        ctx.accounts,
        computation_offset,
        args,
        None,
        vec![CalculateRewardV2Callback::callback_ix(&[
            CallbackAccount {
                pubkey: ctx.accounts.protocol_state.key(),
                is_writable: false,
            },
            CallbackAccount {
                pubkey: ctx.accounts.pool.key(),
                is_writable: false,
            },
            CallbackAccount {
                pubkey: ctx.accounts.pool_vault.key(),
                is_writable: true,
            },
            CallbackAccount {
                pubkey: ctx.accounts.bet.key(),
                is_writable: true,
            },
            CallbackAccount {
                pubkey: ctx.accounts.user_token_account.key(),
                is_writable: true,
            },
            CallbackAccount {
                pubkey: ctx.accounts.user.key(),
                is_writable: false,
            },
        ])],
        1,
    )?;

    Ok(())
}

    /// Callback for reward calculation - distributes the reward
    #[arcium_callback(encrypted_ix = "calculate_reward_v2")]
    pub fn calculate_reward_v2_callback(
        ctx: Context<CalculateRewardV2Callback>,
        output: ComputationOutputs<CalculateRewardV2Output>,
    ) -> Result<()> {
        msg!("=== calc reward call back start === ");
        let result = match output {
            ComputationOutputs::Success(CalculateRewardV2Output { field_0 }) => field_0,
            _ => return Err(ErrorCode::AbortedComputation.into()),
        };


        msg!("=== calc reward call back reward_amount === {}", result.field_0);
        msg!("=== calc reward call back accuracy_bps === {}", result.field_1);

        let reward_amount = result.field_0; 
        let accuracy_bps = result.field_1;   

        let pool = &ctx.accounts.pool;
        let bet = &mut ctx.accounts.bet;

        bet.claimed = true;

        msg!("=== reward_amount === {}", reward_amount);
        msg!("=== accuracy_bps === {}", accuracy_bps);

        if reward_amount > 0 {
            let pool_id = pool.pool_id;
            let pool_bump = pool.bump;

            let pool_id_bytes = pool_id.to_le_bytes();
            let bump_array = [pool_bump];
            let pool_seeds: &[&[u8]] = &[b"pool", pool_id_bytes.as_ref(), bump_array.as_ref()];
            let signer = &[pool_seeds];

            let cpi_accounts = Transfer {
                from: ctx.accounts.pool_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
            token::transfer(cpi_ctx, reward_amount)?;
        }

        emit!(RewardClaimedEvent {
            pool_id: pool.pool_id,
            user: ctx.accounts.user.key(),
            reward_amount,
            accuracy_bps,
        });

        Ok(())
    }

    /// Admin function to close a pool (emergency only)
    pub fn close_pool(ctx: Context<ClosePool>) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.status = PoolStatus::Closed;

        Ok(())
    }
}

// ============================================================================
// Account Structs
// ============================================================================

#[account]
pub struct ProtocolState {
    pub admin: Pubkey,
    pub protocol_fee_bps: u16,
    pub total_pools_created: u64,
    pub bump: u8,
}

impl ProtocolState {
    pub const LEN: usize = 8 + 32 + 2 + 8 + 1;
}

#[account]
pub struct Pool {
    pub pool_id: u64,
    pub admin: Pubkey,
    pub asset_symbol: String,
    pub entry_fee: u64,
    pub target_timestamp: i64,
    pub max_participants: u32,
    pub total_participants: u32,
    pub total_pool_amount: u64,
    pub status: PoolStatus,
    pub actual_price: u64,
    pub bump: u8,
    pub vault_bump: u8,
}

impl Pool {
    pub const LEN: usize = 8 + 8 + 32 + (4 + 10) + 8 + 8 + 4 + 4 + 8 + 1 + 8 + 1 + 1;
}

#[account]
pub struct EncryptedBet {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub encrypted_predicted_price: [u8; 32],
    pub pub_key: [u8; 32],
    pub nonce: u128,
    pub stake_amount: u64,
    pub claimed: bool,
    pub bump: u8,
}

impl EncryptedBet {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 32 + 16 + 8 + 1 + 1;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum PoolStatus {
    Active,
    Finalized,
    Closed,
}

// ============================================================================
// Context Structs - Regular Instructions
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

// ============================================================================
// Arcium Context Structs
// ============================================================================

#[init_computation_definition_accounts("process_bet", payer)]
#[derive(Accounts)]
pub struct InitProcessBetCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Account<'info, MXEAccount>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program
    pub comp_def_account: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

#[queue_computation_accounts("process_bet", payer)]
#[derive(Accounts)]
#[instruction(computation_offset: u64)]
pub struct PlaceEncryptedBet<'info> {
    #[account(
        mut,
        seeds = [b"pool", pool.pool_id.to_le_bytes().as_ref()],
        bump = pool.bump,
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(
        mut,
        seeds = [b"pool_vault", pool.pool_id.to_le_bytes().as_ref()],
        bump = pool.vault_bump,
    )]
    pub pool_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = user,
        space = EncryptedBet::LEN,
        seeds = [b"bet", pool.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub bet: Box<Account<'info, EncryptedBet>>,

    #[account(mut)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init_if_needed,
        space = 9,
        payer = payer,
        seeds = [&SIGN_PDA_SEED],
        bump,
        address = derive_sign_pda!(),
    )]
    pub sign_pda_account: Account<'info, SignerAccount>,
    #[account(
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Account<'info, MXEAccount>,
    #[account(
        mut,
        address = derive_mempool_pda!()
    )]
    /// CHECK: mempool_account, checked by arcium program
    pub mempool_account: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_execpool_pda!()
    )]
    /// CHECK: executing_pool, checked by arcium program
    pub executing_pool: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_comp_pda!(computation_offset)
    )]
    /// CHECK: computation_account, checked by arcium program
    pub computation_account: UncheckedAccount<'info>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_PROCESS_BET)
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(
        mut,
        address = derive_cluster_pda!(mxe_account, ErrorCode::ClusterNotSet)
    )]
    pub cluster_account: Account<'info, Cluster>,
    #[account(
        mut,
        address = ARCIUM_FEE_POOL_ACCOUNT_ADDRESS,
    )]
    pub pool_account: Account<'info, FeePool>,
    #[account(
        address = ARCIUM_CLOCK_ACCOUNT_ADDRESS
    )]
    pub clock_account: Account<'info, ClockAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub arcium_program: Program<'info, Arcium>,
}

#[callback_accounts("process_bet")]
#[derive(Accounts)]
pub struct ProcessBetCallback<'info> {
    pub arcium_program: Program<'info, Arcium>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_PROCESS_BET)
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(address = ::anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: instructions_sysvar, checked by account constraint
    pub instructions_sysvar: AccountInfo<'info>,
}

#[init_computation_definition_accounts("calculate_reward_v2", payer)]
#[derive(Accounts)]
pub struct InitCalculateRewardCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Account<'info, MXEAccount>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program
    pub comp_def_account: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

#[queue_computation_accounts("calculate_reward_v2", payer)]
#[derive(Accounts)]
#[instruction(computation_offset: u64)]
pub struct CalculateReward<'info> {
    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [b"pool", pool.pool_id.to_le_bytes().as_ref()],
        bump = pool.bump,
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(
        mut,
        seeds = [b"pool_vault", pool.pool_id.to_le_bytes().as_ref()],
        bump = pool.vault_bump,
    )]
    pub pool_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [b"bet", pool.key().as_ref(), user.key().as_ref()],
        bump = bet.bump,
    )]
    pub bet: Box<Account<'info, EncryptedBet>>,

    #[account(mut)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    pub user: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init_if_needed,
        space = 9,
        payer = payer,
        seeds = [&SIGN_PDA_SEED],
        bump,
        address = derive_sign_pda!(),
    )]
    pub sign_pda_account: Account<'info, SignerAccount>,
    #[account(
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Account<'info, MXEAccount>,
    #[account(
        mut,
        address = derive_mempool_pda!()
    )]
    /// CHECK: mempool_account, checked by arcium program
    pub mempool_account: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_execpool_pda!()
    )]
    /// CHECK: executing_pool, checked by arcium program
    pub executing_pool: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_comp_pda!(computation_offset)
    )]
    /// CHECK: computation_account, checked by arcium program
    pub computation_account: UncheckedAccount<'info>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_CALCULATE_REWARD)
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(
        mut,
        address = derive_cluster_pda!(mxe_account, ErrorCode::ClusterNotSet)
    )]
    pub cluster_account: Account<'info, Cluster>,
    #[account(
        mut,
        address = ARCIUM_FEE_POOL_ACCOUNT_ADDRESS,
    )]
    pub pool_account: Account<'info, FeePool>,
    #[account(
        address = ARCIUM_CLOCK_ACCOUNT_ADDRESS
    )]
    pub clock_account: Account<'info, ClockAccount>,

    // ADD THIS ↓
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub arcium_program: Program<'info, Arcium>,
}

#[callback_accounts("calculate_reward_v2")]
#[derive(Accounts)]
pub struct CalculateRewardV2Callback<'info> {
    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        seeds = [b"pool", pool.pool_id.to_le_bytes().as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        seeds = [b"pool_vault", pool.pool_id.to_le_bytes().as_ref()],
        bump = pool.vault_bump,
    )]
    pub pool_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"bet", pool.key().as_ref(), user.key().as_ref()],
        bump = bet.bump,
    )]
    pub bet: Account<'info, EncryptedBet>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    /// CHECK: Verified through bet account
    pub user: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub arcium_program: Program<'info, Arcium>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_CALCULATE_REWARD)
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(address = ::anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: instructions_sysvar, checked by account constraint
    pub instructions_sysvar: AccountInfo<'info>,
}

// ============================================================================
// Events
// ============================================================================

#[event]
pub struct PoolCreatedEvent {
    pub pool_id: u64,
    pub asset_symbol: String,
    pub entry_fee: u64,
    pub target_timestamp: i64,
    pub max_participants: u32,
}

#[event]
pub struct EncryptedBetPlacedEvent {
    pub pool_id: u64,
    pub user: Pubkey,
    pub stake_amount: u64,
}

#[event]
pub struct BetProcessedEvent {
    pub success: SharedEncryptedStruct<1>,
}

#[event]
pub struct AdminTransferredEvent {
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
}

#[event]
pub struct ProtocolFeeUpdatedEvent {
    pub old_fee_bps: u16,
    pub new_fee_bps: u16,
}

#[event]
pub struct PoolFinalizedEvent {
    pub pool_id: u64,
    pub actual_price: u64,
    pub total_pool_amount: u64,
}

#[event]
pub struct RewardClaimedEvent {
    pub pool_id: u64,
    pub user: Pubkey,
    pub reward_amount: u64,
    pub accuracy_bps: u64,
}

// ============================================================================
// Errors
// ============================================================================

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid protocol fee")]
    InvalidFee,
    #[msg("Invalid new admin")]
    InvalidNewAdmin,
    #[msg("Asset symbol too long")]
    AssetSymbolTooLong,
    #[msg("Invalid entry fee")]
    InvalidEntryFee,
    #[msg("Invalid timestamp")]
    InvalidTimestamp,
    #[msg("Invalid max participants")]
    InvalidMaxParticipants,
    #[msg("Pool is not active")]
    PoolNotActive,
    #[msg("Prediction time has passed")]
    PredictionTimePassed,
    #[msg("Pool is full")]
    PoolFull,
    #[msg("Invalid predicted price")]
    InvalidPredictedPrice,
    #[msg("Prediction time not reached yet")]
    PredictionTimeNotReached,
    #[msg("Invalid actual price")]
    InvalidActualPrice,
    #[msg("Pool not finalized")]
    PoolNotFinalized,
    #[msg("Reward already claimed")]
    AlreadyClaimed,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("The computation was aborted")]
    AbortedComputation,
    #[msg("Cluster not set")]
    ClusterNotSet,
}
