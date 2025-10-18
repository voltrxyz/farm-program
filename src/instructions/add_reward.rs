use pinocchio::{
    account_info::AccountInfo,
    log::sol_log,
    program_error::ProgramError,
    pubkey::{ find_program_address, Pubkey },
    sysvars::{ clock::Clock, Sysvar },
    ProgramResult,
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_token::{ instructions::TransferChecked, state::Mint };
use voltr_vault::state::Vault;

use crate::{
    constants::{ FARM_AUTHORITY_SEED, MAX_REWARDS_TOKENS },
    error::FarmError,
    state::{ Context, Farm },
};

#[derive(Debug)]
pub struct AddRewardAccounts<'info> {
    pub payer: &'info AccountInfo,
    pub manager: &'info AccountInfo,
    pub voltr_vault: &'info AccountInfo,
    pub farm: &'info AccountInfo,
    pub farm_authority: &'info AccountInfo,
    pub reward_mint: &'info AccountInfo,
    pub farm_reward_token_account: &'info AccountInfo,
    pub manager_reward_token_account: &'info AccountInfo,
    pub reward_token_program: &'info AccountInfo,
    pub associated_token_program: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> TryFrom<&'info [AccountInfo]> for AddRewardAccounts<'info> {
    type Error = ProgramError;

    fn try_from(accounts: &'info [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            payer,
            manager,
            voltr_vault,
            farm,
            farm_authority,
            reward_mint,
            farm_reward_token_account,
            manager_reward_token_account,
            reward_token_program,
            associated_token_program,
            system_program,
        ] = accounts else {
            sol_log("Not enough account keys");
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        if !manager.is_signer() {
            sol_log("Manager is not signer");
            return Err(ProgramError::MissingRequiredSignature);
        }

        if voltr_vault.owner().ne(&voltr_vault::ID) {
            sol_log("Voltr vault is not owned by voltr-vault program");
            return Err(ProgramError::IllegalOwner);
        }

        let vault_data = voltr_vault.try_borrow_data()?;
        let vault: &Vault = (&*vault_data).try_into()?;

        if vault.manager.ne(manager.key()) && vault.admin.ne(manager.key()) {
            sol_log("Manager is not authorized to add reward");
            return Err(ProgramError::MissingRequiredSignature);
        }

        if farm.owner().ne(&crate::ID) {
            sol_log("Farm is not owned by this program");
            return Err(ProgramError::IllegalOwner);
        }

        let (expected_authority_pda, _bump) = find_program_address(
            &[FARM_AUTHORITY_SEED, farm.key().as_ref()],
            &crate::ID
        );
        if farm_authority.key().ne(&expected_authority_pda) {
            sol_log("Farm authority is not valid");
            return Err(ProgramError::InvalidArgument);
        }

        let (expected_reward_token_account_pda, _bump) = find_program_address(
            &[
                farm_authority.key().as_ref(),
                reward_token_program.key().as_ref(),
                reward_mint.key().as_ref(),
            ],
            &pinocchio_associated_token_account::ID
        );

        if farm_reward_token_account.key().ne(&expected_reward_token_account_pda) {
            sol_log("Farm reward token account is not valid");
            return Err(ProgramError::InvalidArgument);
        }

        Ok(Self {
            payer,
            manager,
            voltr_vault,
            farm,
            farm_authority,
            reward_mint,
            farm_reward_token_account,
            manager_reward_token_account,
            reward_token_program,
            associated_token_program,
            system_program,
        })
    }
}

pub struct AddReward<'info> {
    accounts: AddRewardAccounts<'info>,
    amount: u64,
    end_ts: u64,
}

impl<'info> TryFrom<Context<'info>> for AddReward<'info> {
    type Error = ProgramError;

    fn try_from(ctx: Context<'info>) -> Result<Self, Self::Error> {
        let accounts = AddRewardAccounts::try_from(ctx.accounts)?;

        if ctx.instruction_data.len() != core::mem::size_of::<u64>() * 2 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let amount = u64::from_le_bytes(
            ctx.instruction_data[..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
        );
        let end_ts = u64::from_le_bytes(
            ctx.instruction_data[8..16]
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?
        );

        Ok(Self { accounts, amount, end_ts })
    }
}

impl<'info> AddReward<'info> {
    pub fn execute(&self) -> ProgramResult {
        sol_log("Executing AddReward");

        (CreateIdempotent {
            funding_account: self.accounts.payer,
            account: self.accounts.farm_reward_token_account,
            wallet: self.accounts.farm_authority,
            mint: self.accounts.reward_mint,
            system_program: self.accounts.system_program,
            token_program: self.accounts.reward_token_program,
        }).invoke()?;

        let mut farm_data = self.accounts.farm.try_borrow_mut_data()?;
        let farm_state: &mut Farm = (&mut *farm_data).try_into()?;

        let clock = Clock::get()?;
        let current_ts = clock.unix_timestamp as u64;

        if self.end_ts < current_ts {
            return Err(FarmError::InvalidEndTs.into());
        }

        farm_state.refresh_rewards(current_ts)?;

        let reward_mint_key = self.accounts.reward_mint.key();

        for i in 0..MAX_REWARDS_TOKENS {
            if
                farm_state.reward_infos[i].mint == *reward_mint_key ||
                farm_state.reward_infos[i].mint == Pubkey::default()
            {
                let reward_info = &mut farm_state.reward_infos[i];

                reward_info.mint = *reward_mint_key;
                reward_info.available_amount = reward_info.available_amount
                    .checked_add(self.amount)
                    .ok_or(ProgramError::from(FarmError::IntegerOverflow))?;
                reward_info.last_updated_ts = current_ts;
                reward_info.end_ts = self.end_ts;

                break;
            }

            if i == MAX_REWARDS_TOKENS - 1 {
                return Err(FarmError::MaxRewardsReached.into());
            }
        }

        let reward_mint_data = Mint::from_account_info(self.accounts.reward_mint)?;

        (TransferChecked {
            from: self.accounts.manager_reward_token_account,
            to: self.accounts.farm_reward_token_account,
            authority: self.accounts.manager,
            mint: self.accounts.reward_mint,
            amount: self.amount,
            decimals: reward_mint_data.decimals(),
        }).invoke()?;

        Ok(())
    }
}
