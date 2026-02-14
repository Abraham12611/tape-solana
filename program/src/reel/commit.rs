use brine_tree::{Leaf, verify};
use spool_api::prelude::*;
use spool_api::instruction::reel::Commit;
use steel::*;

pub fn process_reel_commit(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = Commit::try_from_bytes(data)?;
    let [
        signer_info, 
        miner_info,
        reel_info, 
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    signer_info.is_signer()?;

    let miner = miner_info
        .as_account_mut::<Miner>(&spool_api::ID)?
        .assert_mut_err(
            |p| p.authority == *signer_info.key,
            ProgramError::MissingRequiredSignature,
        )?;

    let reel = reel_info
        .as_account::<Reel>(&spool_api::ID)?
        .assert_err(
            |p| p.authority == *signer_info.key,
            ProgramError::MissingRequiredSignature,
        )?;

    let merkle_root = &reel.contains;
    let merkle_proof = args.proof.as_ref();
    assert!(merkle_proof.len() == SEGMENT_PROOF_LEN);

    // let segment_id = args.index;
    // let leaf = Leaf::new(&[
    //     segment_id.as_ref(), // u64 (8 bytes)
    //     &args.value,
    // ]);

    let leaf = Leaf::from(args.value);

    check_condition(
        verify(*merkle_root, merkle_proof, leaf),
        SpoolError::ReelCommitFailed,
    )?;

    miner.commitment = args.value;

    Ok(())
}
