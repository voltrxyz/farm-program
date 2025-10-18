use pinocchio::{
    account_info::AccountInfo,
    log::sol_log,
    program_error::ProgramError,
    pubkey::find_program_address,
    sysvars::{ clock::Clock, Sysvar },
    ProgramResult,
};
use pinocchio_token::{ instructions::TransferChecked, state::Mint };

use crate::{ constants::STAKER_AUTHORITY_SEED, error::FarmError, state::{ Context, Farm, Staker } };

#[derive(Debug)]
pub struct StakeAccounts<'info> {
    pub user: &'info AccountInfo,
    pub farm: &'info AccountInfo,
    pub staker: &'info AccountInfo,
    pub staker_authority: &'info AccountInfo,
    pub stake_mint: &'info AccountInfo,
    pub user_stake_token_account: &'info AccountInfo,
    pub staker_stake_token_account: &'info AccountInfo,
    pub stake_token_program: &'info AccountInfo,
}

impl<'info> TryFrom<&'info [AccountInfo]> for StakeAccounts<'info> {
    type Error = ProgramError;

    fn try_from(accounts: &'info [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            user,
            farm,
            staker,
            staker_authority,
            stake_mint,
            user_stake_token_account,
            staker_stake_token_account,
            stake_token_program,
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

        let farm_data = farm.try_borrow_data()?;
        let farm_state: &Farm = (&*farm_data).try_into()?;

        if farm_state.stake_info.mint.ne(stake_mint.key()) {
            sol_log("Stake mint does not match farm's stake mint");
            return Err(ProgramError::InvalidArgument);
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

        let (expected_staker_authority_pda, _bump) = find_program_address(
            &[STAKER_AUTHORITY_SEED, staker.key().as_ref()],
            &crate::ID
        );
        if staker_authority.key().ne(&expected_staker_authority_pda) {
            sol_log("Invalid staker authority PDA");
            return Err(ProgramError::InvalidArgument);
        }

        let (expected_staker_stake_token_account_pda, _bump) = find_program_address(
            &[
                staker_authority.key().as_ref(),
                stake_token_program.key().as_ref(),
                stake_mint.key().as_ref(),
            ],
            &pinocchio_associated_token_account::ID
        );

        if staker_stake_token_account.key().ne(&expected_staker_stake_token_account_pda) {
            sol_log("Staker stake token account is not valid");
            return Err(ProgramError::InvalidArgument);
        }

        Ok(Self {
            user,
            farm,
            staker,
            staker_authority,
            stake_mint,
            user_stake_token_account,
            staker_stake_token_account,
            stake_token_program,
        })
    }
}

pub struct Stake<'info> {
    accounts: StakeAccounts<'info>,
    amount: u64,
}

impl<'info> TryFrom<Context<'info>> for Stake<'info> {
    type Error = ProgramError;

    fn try_from(ctx: Context<'info>) -> Result<Self, Self::Error> {
        let accounts = StakeAccounts::try_from(ctx.accounts)?;

        if ctx.instruction_data.len() != core::mem::size_of::<u64>() {
            return Err(ProgramError::InvalidInstructionData);
        }
        let amount = u64::from_le_bytes(
            ctx.instruction_data.try_into().map_err(|_| ProgramError::InvalidInstructionData)?
        );

        Ok(Self { accounts, amount })
    }
}

impl<'info> Stake<'info> {
    pub fn execute(&self) -> ProgramResult {
        sol_log("Executing Stake");

        if self.amount == 0 {
            return Err(FarmError::InvalidAmount.into());
        }

        let mut farm_data = self.accounts.farm.try_borrow_mut_data()?;
        let farm_state: &mut Farm = (&mut *farm_data).try_into()?;
        let mut staker_data = self.accounts.staker.try_borrow_mut_data()?;
        let staker_state: &mut Staker = (&mut *staker_data).try_into()?;

        let clock = Clock::get()?;
        let current_ts = clock.unix_timestamp as u64;

        farm_state.refresh_rewards(current_ts)?;
        staker_state.refresh_rewards(farm_state)?;

        let stake_mint_data = Mint::from_account_info(self.accounts.stake_mint)?;
        (TransferChecked {
            from: self.accounts.user_stake_token_account,
            to: self.accounts.staker_stake_token_account,
            authority: self.accounts.user,
            mint: self.accounts.stake_mint,
            amount: self.amount,
            decimals: stake_mint_data.decimals(),
        }).invoke()?;

        farm_state.stake_info.total_amount = farm_state.stake_info.total_amount
            .checked_add(self.amount)
            .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;

        staker_state.stake_amount = staker_state.stake_amount
            .checked_add(self.amount)
            .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;
        staker_state.last_stake_ts = current_ts;

        Ok(())
    }
}
