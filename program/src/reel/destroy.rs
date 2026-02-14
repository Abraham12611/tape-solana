use spool_api::prelude::*;
use steel::*;

pub fn process_reel_destroy(accounts: &[AccountInfo<'_>], _data: &[u8]) -> ProgramResult {
    let [
        signer_info, 
        reel_info, 
        system_program_info,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    signer_info.is_signer()?;

    system_program_info
        .is_program(&system_program::ID)?;

    reel_info
        .is_writable()?
        .as_account::<Reel>(&spool_api::ID)?
        .assert_err(
            |p| p.authority == *signer_info.key,
            ProgramError::MissingRequiredSignature,
        )?;

    // Return rent to signer.
    reel_info.close(signer_info)?;

    Ok(())
}
