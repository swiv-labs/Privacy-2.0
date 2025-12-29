use anchor_lang::prelude::*;

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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum PoolStatus {
    Active,
    Finalized,
    Closed,
}