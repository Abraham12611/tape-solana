use spool_api::prelude::*;
use spool_api::instruction::reel::Create;
use steel::*;

pub fn process_reel_create(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let current_time = Clock::get()?.unix_timestamp;
    let args = Create::try_from_bytes(data)?;
    let [
        signer_info,
        miner_info,
        reel_info,
        system_program_info, 
        rent_info,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    signer_info.is_signer()?;

    system_program_info.is_program(&system_program::ID)?;
    rent_info.is_sysvar(&sysvar::rent::ID)?;

    let reel_number = u64::from_le_bytes(args.number);
    let (reel_pda, _bump) = reel_pda(*miner_info.key, reel_number);

    reel_info
        .is_empty()?
        .is_writable()?
        .has_address(&reel_pda)?;

    miner_info
        .as_account::<Miner>(&spool_api::ID)?
        .assert_err(
            |p| p.authority == *signer_info.key,
            ProgramError::MissingRequiredSignature,
        )?;

    // Create reel account.
    create_program_account::<Reel>(
        reel_info,
        system_program_info,
        signer_info,
        &spool_api::ID,
        &[REEL, miner_info.key.as_ref(), &args.number],
    )?;

    let reel = reel_info.as_account_mut::<Reel>(&spool_api::ID)?;

    reel.number            = reel_number;
    reel.authority         = *signer_info.key;
    reel.last_proof_at     = current_time;
    reel.last_proof_block  = 0;
    reel.state             = SpoolTree::new(&[reel_info.key.as_ref()]);
    reel.contains          = [0; 32];
    reel.total_spools      = 0;

    Ok(())
}
