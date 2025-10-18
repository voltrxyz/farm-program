#![cfg_attr(target_arch = "bpf", no_std)]

use pinocchio::{
    no_allocator,
    nostd_panic_handler,
    program_entrypoint,
    program_error::ProgramError,
    pubkey::Pubkey,
    account_info::AccountInfo,
    ProgramResult,
};
use pinocchio_pubkey::declare_id;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod validation;

use crate::state::Context;

use instructions::*;

declare_id!("6oB5DU2BTR8WR5njtbjbUso4hR3mcbU9KFyscDp7tnxD");

program_entrypoint!(process_instruction);

no_allocator!();
nostd_panic_handler!();

fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8]
) -> ProgramResult {
    let (discriminator, instruction_data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    let context = Context::from((accounts, instruction_data));

    match discriminator {
        0 => InitFarm::try_from(context)?.execute(),
        1 => InitStaker::try_from(context)?.execute(),
        2 => Stake::try_from(context)?.execute(),
        3 => Unstake::try_from(context)?.execute(),
        4 => ClaimReward::try_from(context)?.execute(),
        5 => AddReward::try_from(context)?.execute(),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
