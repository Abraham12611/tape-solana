use spool_api::prelude::*;
use steel::*;

pub fn process_spool_write(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let current_slot = Clock::get()?.slot;
    let [
        signer_info, 
        spool_info,
        writer_info,
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

    let (spool_address, _spool_bump) = spool_pda(*signer_info.key, &spool.name);
    let (writer_address, _writer_bump) = writer_pda(spool_address);

    spool_info.has_address(&spool_address)?;
    writer_info.has_address(&writer_address)?;
        
    check_condition(
        spool.state.eq(&u64::from(SpoolState::Created)) ||
        spool.state.eq(&u64::from(SpoolState::Writing)),
        SpoolError::UnexpectedState,
    )?;

    // Convert the data to a canonical segments of data 
    // and write them to the Merkle tree (all segments are 
    // written as SEGMENT_SIZE bytes, no matter the size 
    // of the data)

    let segments = data.chunks(SEGMENT_SIZE);
    let segment_count = segments.len() as u64;

    check_condition(
        spool.total_segments + segment_count <= MAX_SEGMENTS_PER_SPOOL as u64,
        SpoolError::SpoolTooLong,
    )?;

    for (segment_number, segment) in segments.enumerate() {
        let canonical_segment = padded_array::<SEGMENT_SIZE>(segment);

        write_segment(
            &mut writer.state,
            spool.total_segments + segment_number as u64,
            &canonical_segment,
        )?;
    }

    let prev_slot = spool.tail_slot;

    spool.total_segments   += segment_count;
    spool.merkle_root       = writer.state.get_root().to_bytes();
    spool.state             = SpoolState::Writing.into();
    spool.tail_slot         = current_slot;

    WriteEvent {
        prev_slot,
        num_added: segment_count,
        num_total: spool.total_segments,
        address: spool_address.to_bytes(),
    }
    .log();

    Ok(())
}
