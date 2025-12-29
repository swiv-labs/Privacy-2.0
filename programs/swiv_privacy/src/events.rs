use anchor_lang::prelude::*;
use arcium_anchor::prelude::SharedEncryptedStruct;

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