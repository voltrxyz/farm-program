use pinocchio::{
    account_info::AccountInfo,
    instruction::{ Seed, Signer },
    log::sol_log,
    program_error::ProgramError,
    pubkey::find_program_address,
    sysvars::{ clock::Clock, Sysvar },
    ProgramResult,
};
use pinocchio_token::{ instructions::TransferChecked, state::Mint };

use crate::{ constants::FARM_AUTHORITY_SEED, error::FarmError, state::{ Context, Farm, Staker } };

#[derive(Debug)]
pub struct ClaimRewardAccounts<'info> {
    pub user: &'info AccountInfo,
    pub farm: &'info AccountInfo,
    pub farm_authority: &'info AccountInfo,
    pub staker: &'info AccountInfo,
    pub reward_mint: &'info AccountInfo,
    pub farm_reward_token_account: &'info AccountInfo,
    pub user_reward_token_account: &'info AccountInfo,
    pub reward_token_program: &'info AccountInfo,
}

impl<'info> TryFrom<&'info [AccountInfo]> for ClaimRewardAccounts<'info> {
    type Error = ProgramError;

    fn try_from(accounts: &'info [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            user,
            farm,
            farm_authority,
            staker,
            reward_mint,
            farm_reward_token_account,
            user_reward_token_account,
            reward_token_program,
        ] = accounts else {
            sol_log("Not enough account keys");
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        if !user.is_signer() {
            sol_log("User is not a signer");
            return Err(ProgramError::MissingRequiredSignature);
        }

        if farm.owner().ne(&crate::ID) {
            sol_log("Farm is not owned by this program");
            return Err(ProgramError::IllegalOwner);
        }

        if staker.owner().ne(&crate::ID) {
            sol_log("Staker is not owned by this program");
            return Err(ProgramError::IllegalOwner);
        }

        let staker_data = staker.try_borrow_data()?;
        let staker_state: &Staker = (&*staker_data).try_into()?;

        if staker_state.farm.ne(farm.key()) {
            sol_log("Staker does not belong to this farm");
            return Err(ProgramError::InvalidArgument);
        }
        if staker_state.user.ne(user.key()) {
            sol_log("Staker does not belong to this user");
            return Err(ProgramError::InvalidArgument);
        }

        let (expected_user_reward_token_account_pda, _bump) = find_program_address(
            &[user.key().as_ref(), reward_token_program.key().as_ref(), reward_mint.key().as_ref()],
            &pinocchio_associated_token_account::ID
        );

        if user_reward_token_account.key().ne(&expected_user_reward_token_account_pda) {
            sol_log("User reward token account is not valid");
            return Err(ProgramError::InvalidArgument);
        }

        Ok(Self {
            user,
            farm,
            farm_authority,
            staker,
            reward_mint,
            farm_reward_token_account,
            user_reward_token_account,
            reward_token_program,
        })
    }
}

pub struct ClaimReward<'info> {
    accounts: ClaimRewardAccounts<'info>,
}

impl<'info> TryFrom<Context<'info>> for ClaimReward<'info> {
    type Error = ProgramError;

    fn try_from(ctx: Context<'info>) -> Result<Self, Self::Error> {
        let accounts = ClaimRewardAccounts::try_from(ctx.accounts)?;
        Ok(Self { accounts })
    }
}

impl<'info> ClaimReward<'info> {
    pub fn execute(&self) -> ProgramResult {
        sol_log("Executing ClaimReward");

        let mut farm_data = self.accounts.farm.try_borrow_mut_data()?;
        let farm_state: &mut Farm = (&mut *farm_data).try_into()?;
        let mut staker_data = self.accounts.staker.try_borrow_mut_data()?;
        let staker_state: &mut Staker = (&mut *staker_data).try_into()?;

        let clock = Clock::get()?;
        let current_ts = clock.unix_timestamp as u64;

        farm_state.refresh_rewards(current_ts)?;
        staker_state.refresh_rewards(farm_state)?;

        let reward_mint_key = self.accounts.reward_mint.key();
        let reward_index = farm_state.reward_infos
            .iter()
            .position(|&info| info.mint == *reward_mint_key)
            .ok_or(ProgramError::from(FarmError::RewardNotFound))?;

        let amount_to_claim = staker_state.rewards_issued_unclaimed[reward_index];
        if amount_to_claim == 0 {
            return Err(FarmError::NothingToClaim.into());
        }

        let authority_bump_bytes = [farm_state.authority_bump];
        let seeds = [
            Seed::from(FARM_AUTHORITY_SEED),
            Seed::from(self.accounts.farm.key().as_ref()),
            Seed::from(&authority_bump_bytes),
        ];
        let signers = [Signer::from(&seeds)];

        let reward_mint_data = Mint::from_account_info(self.accounts.reward_mint)?;
        (TransferChecked {
            from: self.accounts.farm_reward_token_account,
            to: self.accounts.user_reward_token_account,
            authority: self.accounts.farm_authority,
            mint: self.accounts.reward_mint,
            amount: amount_to_claim,
            decimals: reward_mint_data.decimals(),
        }).invoke_signed(&signers)?;

        farm_state.reward_infos[reward_index].issued_unclaimed_amount = farm_state.reward_infos[
            reward_index
        ].issued_unclaimed_amount
            .checked_sub(amount_to_claim)
            .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;

        staker_state.rewards_issued_unclaimed[reward_index] = 0;
        staker_state.last_claim_ts[reward_index] = current_ts;

        Ok(())
    }
}
