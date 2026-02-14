use brine_tree::Leaf;
use spool_api::prelude::*;
use spool_api::instruction::spool::Update;
use steel::*;

pub fn process_spool_update(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let current_slot = Clock::get()?.slot;
    let args = Update::try_from_bytes(data)?;

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

    let segment_number = args.segment_number;
    let merkle_proof   = args.proof.as_ref();

    assert!(args.old_data.len() == SEGMENT_SIZE);
    assert!(args.new_data.len() == SEGMENT_SIZE);
    assert!(merkle_proof.len() == SEGMENT_PROOF_LEN);

    let old_leaf = Leaf::new(&[
        segment_number.as_ref(), // u64_le_bytes
        args.old_data.as_ref(),
    ]);

    let new_leaf = Leaf::new(&[
        segment_number.as_ref(), // u64_le_bytes
        args.new_data.as_ref(),
    ]);

    writer.state.try_replace_leaf(
        merkle_proof,
        old_leaf, 
        new_leaf
    )
    .map_err(|_| SpoolError::WriteFailed)?;

    let prev_slot = spool.tail_slot;

    spool.merkle_root = writer.state.get_root().to_bytes();
    spool.tail_slot   = current_slot;

    UpdateEvent {
        prev_slot,
        segment_number: u64::from_le_bytes(segment_number),
        address: spool_address.to_bytes(),
    }
    .log();

    Ok(())
}

