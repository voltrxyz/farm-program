use pinocchio::{
    account_info::AccountInfo,
    instruction::{ Seed, Signer },
    log::sol_log,
    program_error::ProgramError,
    pubkey::find_program_address,
    sysvars::{ clock::Clock, rent::Rent, Sysvar },
    ProgramResult,
};
use pinocchio_associated_token_account;
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_system::instructions::CreateAccount;

use crate::{
    constants::{ MAX_REWARDS_TOKENS, STAKER_AUTHORITY_SEED, STAKER_SEED },
    state::{ Context, Farm, Staker },
};

#[derive(Debug)]
pub struct InitStakerAccounts<'info> {
    pub payer: &'info AccountInfo,
    pub user: &'info AccountInfo,
    pub farm: &'info AccountInfo,
    pub staker: &'info AccountInfo,
    pub staker_authority: &'info AccountInfo,
    pub stake_mint: &'info AccountInfo,
    pub staker_stake_token_account: &'info AccountInfo,
    pub stake_token_program: &'info AccountInfo,
    pub associated_token_program: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> TryFrom<&'info [AccountInfo]> for InitStakerAccounts<'info> {
    type Error = ProgramError;

    fn try_from(accounts: &'info [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            payer,
            user,
            farm,
            staker,
            staker_authority,
            stake_mint,
            staker_stake_token_account,
            stake_token_program,
            associated_token_program,
            system_program,
        ] = accounts else {
            sol_log("Not enough account keys");
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        if !user.is_signer() {
            sol_log("User is not signer");
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
            return Err(ProgramError::InvalidAccountData);
        }

        let (expected_authority_pda, _bump) = find_program_address(
            &[STAKER_AUTHORITY_SEED, staker.key().as_ref()],
            &crate::ID
        );
        if staker_authority.key().ne(&expected_authority_pda) {
            sol_log("Invalid staker authority PDA");
            return Err(ProgramError::InvalidArgument);
        }

        Ok(Self {
            payer,
            user,
            farm,
            staker,
            staker_authority,
            stake_mint,
            staker_stake_token_account,
            stake_token_program,
            associated_token_program,
            system_program,
        })
    }
}

pub struct InitStaker<'info> {
    accounts: InitStakerAccounts<'info>,
}

impl<'info> TryFrom<Context<'info>> for InitStaker<'info> {
    type Error = ProgramError;

    fn try_from(ctx: Context<'info>) -> Result<Self, Self::Error> {
        let accounts = InitStakerAccounts::try_from(ctx.accounts)?;
        Ok(Self { accounts })
    }
}

impl<'info> InitStaker<'info> {
    pub fn execute(&self) -> ProgramResult {
        sol_log("Executing InitStaker");

        // Derive PDAs and bumps
        let (staker_pda, staker_bump) = find_program_address(
            &[STAKER_SEED, self.accounts.farm.key().as_ref(), self.accounts.user.key().as_ref()],
            &crate::ID
        );
        let (_authority_pda, authority_bump) = find_program_address(
            &[STAKER_AUTHORITY_SEED, &staker_pda.as_ref()],
            &crate::ID
        );

        // Create the Staker account
        let space = core::mem::size_of::<Staker>();
        let lamports = Rent::get()?.minimum_balance(space);
        let staker_bump_bytes = [staker_bump];

        let seeds = [
            Seed::from(STAKER_SEED),
            Seed::from(self.accounts.farm.key().as_ref()),
            Seed::from(self.accounts.user.key().as_ref()),
            Seed::from(&staker_bump_bytes),
        ];
        let signers = [Signer::from(&seeds)];

        (CreateAccount {
            from: self.accounts.payer,
            to: self.accounts.staker,
            lamports,
            space: space as u64,
            owner: &crate::ID,
        }).invoke_signed(&signers)?;

        // Create the Staker's stake token account (ATA)
        (CreateIdempotent {
            funding_account: self.accounts.payer,
            account: self.accounts.staker_stake_token_account,
            wallet: self.accounts.staker_authority,
            mint: self.accounts.stake_mint,
            system_program: self.accounts.system_program,
            token_program: self.accounts.stake_token_program,
        }).invoke()?;

        // Initialize the Staker account state
        let mut staker_data = self.accounts.staker.try_borrow_mut_data()?;
        let staker_state: &mut Staker = (&mut *staker_data).try_into()?;

        let clock = Clock::get()?;
        let current_ts = clock.unix_timestamp as u64;

        staker_state.user = *self.accounts.user.key();
        staker_state.farm = *self.accounts.farm.key();
        staker_state.stake_amount = 0;
        staker_state.rewards_tally_scaled = [0; MAX_REWARDS_TOKENS];
        staker_state.rewards_issued_unclaimed = [0; MAX_REWARDS_TOKENS];
        staker_state.last_claim_ts = [current_ts; MAX_REWARDS_TOKENS];
        staker_state.last_stake_ts = current_ts;
        staker_state.bump = staker_bump;
        staker_state.authority_bump = authority_bump;

        Ok(())
    }
}
