use pinocchio::{ account_info::AccountInfo, program_error::ProgramError };

pub fn program_account_data_check<T>(account: &AccountInfo) -> Result<(), ProgramError> {
    if account.data_len() != core::mem::size_of::<T>() {
        return Err(ProgramError::InvalidAccountData);
    }

    if (account.data_ptr() as usize) % core::mem::align_of::<T>() != 0 {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}
