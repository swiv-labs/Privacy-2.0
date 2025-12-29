use anchor_lang::prelude::*;

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