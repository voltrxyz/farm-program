use pinocchio::{
    account_info::AccountInfo,
    instruction::{ Seed, Signer },
    log::sol_log,
    program_error::ProgramError,
    pubkey::find_program_address,
    sysvars::{ rent::Rent, Sysvar },
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::state::Mint;
use voltr_vault::state::Vault;

use crate::{
    constants::{ FARM_AUTHORITY_SEED, FARM_SEED },
    state::{ Context, Farm, RewardInfo, StakeInfo },
};

#[derive(Debug)]
pub struct InitFarmAccounts<'info> {
    pub payer: &'info AccountInfo,
    pub manager: &'info AccountInfo,
    pub voltr_vault: &'info AccountInfo,
    pub farm: &'info AccountInfo,
    pub farm_authority: &'info AccountInfo,
    pub stake_mint: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> TryFrom<&'info [AccountInfo]> for InitFarmAccounts<'info> {
    type Error = ProgramError;

    fn try_from(accounts: &'info [AccountInfo]) -> Result<Self, Self::Error> {
        let [payer, manager, voltr_vault, farm, farm_authority, stake_mint, system_program] =
            accounts else {
            sol_log("Not enough account keys");
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        if !manager.is_signer() {
            sol_log("Manager is not signer");
            return Err(ProgramError::MissingRequiredSignature);
        }

        if voltr_vault.owner().ne(&voltr_vault::ID) {
            return Err(ProgramError::IllegalOwner);
        }

        let vault_data = voltr_vault.try_borrow_data()?;
        let vault: &Vault = (&*vault_data).try_into()?;

        if vault.manager.ne(manager.key()) && vault.admin.ne(manager.key()) {
            return Err(ProgramError::MissingRequiredSignature);
        }

        if vault.lp.mint.ne(stake_mint.key()) {
            return Err(ProgramError::InvalidAccountData);
        }

        if stake_mint.owner().ne(&pinocchio_token::ID) {
            return Err(ProgramError::IllegalOwner);
        }
        if stake_mint.data_len() != Mint::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        let (expected_authority_pda, _bump) = find_program_address(
            &[FARM_AUTHORITY_SEED, farm.key().as_ref()],
            &crate::ID
        );
        if farm_authority.key().ne(&expected_authority_pda) {
            return Err(ProgramError::InvalidArgument);
        }

        Ok(Self {
            payer,
            manager,
            voltr_vault,
            farm,
            farm_authority,
            stake_mint,
            system_program,
        })
    }
}

pub struct InitFarm<'info> {
    accounts: InitFarmAccounts<'info>,
}

impl<'info> TryFrom<Context<'info>> for InitFarm<'info> {
    type Error = ProgramError;

    fn try_from(ctx: Context<'info>) -> Result<Self, Self::Error> {
        let accounts = InitFarmAccounts::try_from(ctx.accounts)?;
        Ok(Self { accounts })
    }
}

impl<'info> InitFarm<'info> {
    pub fn execute(&self) -> ProgramResult {
        let (farm_pda, farm_bump) = find_program_address(
            &[FARM_SEED, self.accounts.stake_mint.key().as_ref()],
            &crate::ID
        );
        let (_authority_pda, authority_bump) = find_program_address(
            &[FARM_AUTHORITY_SEED, &farm_pda.as_ref()],
            &crate::ID
        );

        let space = core::mem::size_of::<Farm>();
        let lamports = Rent::get()?.minimum_balance(space);

        let farm_bump_array = [farm_bump];

        let seeds = [
            Seed::from(FARM_SEED),
            Seed::from(self.accounts.stake_mint.key().as_ref()),
            Seed::from(&farm_bump_array),
        ];
        let signers = [Signer::from(&seeds)];

        (CreateAccount {
            from: self.accounts.payer,
            to: self.accounts.farm,
            lamports,
            space: space as u64,
            owner: &crate::ID,
        }).invoke_signed(&signers)?;

        let mut farm_data = self.accounts.farm.try_borrow_mut_data()?;
        let farm_state: &mut Farm = (&mut *farm_data).try_into()?;

        farm_state.stake_info = StakeInfo {
            mint: *self.accounts.stake_mint.key(),
            total_amount: 0,
        };
        farm_state.reward_infos = [RewardInfo::default(); crate::constants::MAX_REWARDS_TOKENS];
        farm_state.bump = farm_bump;
        farm_state.authority_bump = authority_bump;

        Ok(())
    }
}
