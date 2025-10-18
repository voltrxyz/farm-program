use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};
use crate::{ constants::{ MAX_REWARDS_TOKENS, REWARD_PRECISION }, error::FarmError };

pub struct Context<'info> {
    pub accounts: &'info [AccountInfo],
    pub instruction_data: &'info [u8],
}

impl<'info> From<(&'info [AccountInfo], &'info [u8])> for Context<'info> {
    fn from(value: (&'info [AccountInfo], &'info [u8])) -> Self {
        Context {
            accounts: value.0,
            instruction_data: value.1,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct StakeInfo {
    pub mint: Pubkey,
    pub total_amount: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RewardInfo {
    pub mint: Pubkey,
    pub available_amount: u64,
    pub issued_unclaimed_amount: u64,
    pub issued_cumulative_amount: u64,
    pub end_ts: u64,
    pub last_updated_ts: u64,
    pub rewards_per_share_scaled: u128,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Farm {
    pub stake_info: StakeInfo,
    pub reward_infos: [RewardInfo; MAX_REWARDS_TOKENS],
    pub bump: u8,
    pub authority_bump: u8,
}

impl<'info> TryFrom<&'info [u8]> for &'info Farm {
    type Error = ProgramError;
    fn try_from(data: &'info [u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<Farm>() {
            return Err(ProgramError::InvalidAccountData);
        }

        if (data.as_ptr() as usize) % core::mem::align_of::<Farm>() != 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &*(data.as_ptr() as *const Farm) })
    }
}

impl<'info> TryFrom<&'info mut [u8]> for &'info mut Farm {
    type Error = ProgramError;
    fn try_from(data: &'info mut [u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<Farm>() {
            return Err(ProgramError::InvalidAccountData);
        }
        if (data.as_ptr() as usize) % core::mem::align_of::<Farm>() != 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Farm) })
    }
}
impl Farm {
    pub fn refresh_rewards(&mut self, current_ts: u64) -> ProgramResult {
        for reward_info in self.reward_infos.iter_mut() {
            if reward_info.mint == Pubkey::default() {
                continue;
            }

            let effective_ts = if current_ts < reward_info.end_ts {
                current_ts
            } else {
                reward_info.end_ts
            };

            if self.stake_info.total_amount > 0 && effective_ts > reward_info.last_updated_ts {
                let time_passed = effective_ts
                    .checked_sub(reward_info.last_updated_ts)
                    .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;
                let reward_period = reward_info.end_ts
                    .checked_sub(reward_info.last_updated_ts)
                    .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;

                // Calculate how many rewards to issue based on a linear emission schedule.
                let amount_to_issue = if reward_period > 0 {
                    (reward_info.available_amount as u128)
                        .checked_mul(time_passed as u128)
                        .ok_or(ProgramError::from(FarmError::IntegerOverflow))?
                        .checked_div(reward_period as u128)
                        .unwrap_or(0) as u64
                } else if effective_ts >= reward_info.end_ts {
                    // If duration was 0 or the period is over, issue all remaining rewards.
                    reward_info.available_amount
                } else {
                    0
                };

                if amount_to_issue > 0 {
                    let added_reward_per_share = (amount_to_issue as u128)
                        .checked_mul(REWARD_PRECISION)
                        .ok_or(ProgramError::from(FarmError::IntegerOverflow))?
                        .checked_div(self.stake_info.total_amount as u128)
                        .unwrap_or(0);

                    reward_info.rewards_per_share_scaled = reward_info.rewards_per_share_scaled
                        .checked_add(added_reward_per_share)
                        .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;

                    // Move the issued amount from 'available' to 'issued_unclaimed'.
                    reward_info.available_amount = reward_info.available_amount
                        .checked_sub(amount_to_issue)
                        .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;
                    reward_info.issued_unclaimed_amount = reward_info.issued_unclaimed_amount
                        .checked_add(amount_to_issue)
                        .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;
                    reward_info.issued_cumulative_amount = reward_info.issued_cumulative_amount
                        .checked_add(amount_to_issue)
                        .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;
                }
            }

            reward_info.last_updated_ts = effective_ts;
        }
        Ok(())
    }
}

// Converted from Anchor's Staker account
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Staker {
    pub user: Pubkey,
    pub farm: Pubkey,
    pub stake_amount: u64,
    pub rewards_tally_scaled: [u128; MAX_REWARDS_TOKENS],
    pub rewards_issued_unclaimed: [u64; MAX_REWARDS_TOKENS],
    pub last_claim_ts: [u64; MAX_REWARDS_TOKENS],
    pub last_stake_ts: u64,
    pub bump: u8,
    pub authority_bump: u8,
}

impl<'info> TryFrom<&'info [u8]> for &'info Staker {
    type Error = ProgramError;
    fn try_from(data: &'info [u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<Staker>() {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if (data.as_ptr() as usize) % core::mem::align_of::<Staker>() != 0 {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(unsafe { &*(data.as_ptr() as *const Staker) })
    }
}

impl<'info> TryFrom<&'info mut [u8]> for &'info mut Staker {
    type Error = ProgramError;
    fn try_from(data: &'info mut [u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<Staker>() {
            return Err(ProgramError::InvalidAccountData);
        }

        if (data.as_ptr() as usize) % core::mem::align_of::<Staker>() != 0 {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Staker) })
    }
}

impl Staker {
    pub fn refresh_rewards(&mut self, farm: &Farm) -> ProgramResult {
        for i in 0..MAX_REWARDS_TOKENS {
            let farm_reward_info = &farm.reward_infos[i];

            if farm_reward_info.mint == Pubkey::default() {
                continue;
            }

            let last_tally = self.rewards_tally_scaled[i];
            let current_rps_scaled = farm_reward_info.rewards_per_share_scaled;

            if current_rps_scaled > last_tally {
                let diff = current_rps_scaled
                    .checked_sub(last_tally)
                    .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;

                let pending_reward = diff
                    .checked_mul(self.stake_amount as u128)
                    .ok_or(ProgramError::from(FarmError::IntegerOverflow))?
                    .checked_div(REWARD_PRECISION)
                    .unwrap_or(0) as u64;

                self.rewards_issued_unclaimed[i] = self.rewards_issued_unclaimed[i]
                    .checked_add(pending_reward)
                    .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;
            }
            self.rewards_tally_scaled[i] = current_rps_scaled;
        }
        Ok(())
    }
}
