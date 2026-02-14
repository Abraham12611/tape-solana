use spool_api::prelude::*;
use spool_api::instruction::reel::Pack;
use brine_tree::Leaf;
use steel::*;

pub fn process_reel_pack(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let pack_args = Pack::try_from_bytes(data)?;
    let [
        signer_info, 
        reel_info,
        spool_info,
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

    let spool = spool_info
        .as_account::<Spool>(&spool_api::ID)?
        .assert_err(
            |p| p.state  == u64::from(SpoolState::Finalized),
            SpoolError::UnexpectedState.into()
        )?
        .assert_err(
            |p| p.number > 0,
            SpoolError::UnexpectedState.into()
        )?;

    check_condition(
        reel.total_spools as usize <= MAX_SPOOLS_PER_REEL,
        SpoolError::ReelTooManySpools,
    )?;

    let spool_id = spool.number.to_le_bytes();
    let leaf = Leaf::new(&[
        spool_id.as_ref(), // u64 (8 bytes)
        &pack_args.value,
    ]);

    check_condition(
        reel.state.try_add_leaf(leaf).is_ok(),
        SpoolError::ReelPackFailed,
    )?;
    
    reel.total_spools += 1;

    Ok(())
}
