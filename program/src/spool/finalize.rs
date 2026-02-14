use spool_api::prelude::*;
use spool_api::instruction::spool::Finalize;
use steel::*;

pub fn process_spool_finalize(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let _args = Finalize::try_from_bytes(data)?;
    let [
        signer_info, 
        spool_info,
        writer_info, 
        archive_info,
        system_program_info,
        rent_sysvar_info,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    signer_info.is_signer()?;

    let spool = spool_info
        .as_account_mut::<Spool>(&spool_api::ID)?
        .assert_mut_err(
            |p| p.authority == *signer_info.key,
            ProgramError::MissingRequiredSignature,
        )?;

    let writer = writer_info
        .as_account_mut::<Writer>(&spool_api::ID)?
        .assert_mut_err(
            |p| p.spool == *spool_info.key,
            ProgramError::InvalidAccountData,
        )?;

    let archive = archive_info
        .is_archive()?
        .as_account_mut::<Archive>(&spool_api::ID)?;

    let (spool_address, _spool_bump) = spool_pda(*signer_info.key, &spool.name);
    let (writer_address, _writer_bump) = writer_pda(spool_address);

    spool_info.has_address(&spool_address)?;
    writer_info.has_address(&writer_address)?;

    system_program_info
        .is_program(&system_program::ID)?;

    rent_sysvar_info
        .is_sysvar(&sysvar::rent::ID)?;

    // Can't finalize if the spool with no data on it.
    check_condition(
        spool.state.eq(&u64::from(SpoolState::Writing)),
        SpoolError::UnexpectedState,
    )?;

    // Can't finalize the spool if it doesn't have enough rent
    check_condition(
        spool.can_finalize(),
        SpoolError::InsufficientRent,
    )?;

    archive.spools_stored    = archive.spools_stored.saturating_add(1);
    archive.segments_stored = archive.segments_stored.saturating_add(spool.total_segments);

    spool.number            = archive.spools_stored;
    spool.state             = SpoolState::Finalized.into();
    spool.merkle_root       = writer.state.get_root().into();

    // Close the writer and return rent to signer.
    writer_info.close(signer_info)?;

    FinalizeEvent {
        spool: spool.number,
        address: spool_address.to_bytes()
    }
    .log();

    Ok(())
}

