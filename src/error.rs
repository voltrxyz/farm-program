use pinocchio::program_error::ProgramError;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FarmError {
    #[error("Maximum number of reward tokens reached.")]
    MaxRewardsReached, // 0
    #[error("Invalid end timestamp.")]
    InvalidEndTs, // 1
    #[error("Integer overflow")]
    IntegerOverflow, // 2
    #[error("Amount must be greater than zero.")]
    InvalidAmount, // 3
    #[error("Reward mint not found in farm.")]
    RewardNotFound, // 4
    #[error("No rewards available to claim for this token.")]
    NothingToClaim, // 5
    #[error("Insufficient staked amount to unstake.")]
    InsufficientStake, // 6
}

impl From<FarmError> for ProgramError {
    fn from(e: FarmError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
