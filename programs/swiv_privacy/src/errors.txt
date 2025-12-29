use anchor_lang::prelude::*;

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