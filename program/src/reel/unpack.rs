use brine_tree::Leaf;
use spool_api::prelude::*;
use spool_api::instruction::reel::Unpack;
use steel::*;

pub fn process_reel_unpack(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = Unpack::try_from_bytes(data)?;
    let [
        signer_info, 
        reel_info, 
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    signer_info.is_signer()?;

    let reel = reel_info
        .as_account_mut::<Reel>(&spool_api::ID)?
        .assert_mut_err(
            |p| p.authority == *signer_info.key,
            ProgramError::MissingRequiredSignature,
        )?;

    let merkle_proof = args.proof;
    assert!(merkle_proof.len() == SPOOL_PROOF_LEN);

    let spool_id = args.index;
    let leaf = Leaf::new(&[
        spool_id.as_ref(), // u64 (8 bytes)
        &args.value,
    ]);

    // let leaf = Leaf::from(args.value);

    check_condition(
        reel.state.contains_leaf(&merkle_proof, leaf),
        SpoolError::ReelUnpackFailed,
    )?;

    reel.contains = args.value;

    Ok(())
}
