use spool_api::prelude::*;
use spool_api::instruction::spool::Create;
use steel::*;

pub fn process_spool_create(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let current_slot = Clock::get()?.slot;
    let args = Create::try_from_bytes(data)?;
    let [
        signer_info, 
        spool_info,
        writer_info, 
        system_program_info,
        rent_sysvar_info,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    signer_info.is_signer()?;

    let (spool_address, _spool_bump) = spool_pda(*signer_info.key, &args.name);
    let (writer_address, _writer_bump) = writer_pda(spool_address);

    spool_info
        .is_empty()?
        .is_writable()?
        .has_address(&spool_address)?;

    writer_info
        .is_empty()?
        .is_writable()?
        .has_address(&writer_address)?;

    system_program_info
        .is_program(&system_program::ID)?;

    rent_sysvar_info
        .is_sysvar(&sysvar::rent::ID)?;

    create_program_account::<Spool>(
        spool_info,
        system_program_info,
        signer_info,
        &spool_api::ID,
        &[SPOOL, signer_info.key.as_ref(), &args.name],
    )?;

    create_program_account::<Writer>(
        writer_info,
        system_program_info,
        signer_info,
        &spool_api::ID,
        &[WRITER, spool_info.key.as_ref()],
    )?;

    let spool = spool_info.as_account_mut::<Spool>(&spool_api::ID)?;
    let writer = writer_info.as_account_mut::<Writer>(&spool_api::ID)?;

    spool.number            = 0; // (spools get a number when finalized)
    spool.authority         = *signer_info.key;
    spool.name              = args.name;
    spool.state             = SpoolState::Created.into();
    spool.total_segments    = 0;
    spool.merkle_root       = [0; 32];
    spool.header            = [0; HEADER_SIZE];
    spool.first_slot        = current_slot; 
    spool.tail_slot         = current_slot;

    writer.spool            = *spool_info.key;
    writer.state           = SegmentTree::new(&[spool_info.key.as_ref()]);

    Ok(())
}
