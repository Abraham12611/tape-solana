use spool_api::prelude::*;
use spool_api::instruction::spool::SetHeader;
use steel::*;

pub fn process_spool_set_header(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = SetHeader::try_from_bytes(data)?;
    let [
        signer_info, 
        spool_info,
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

    let (spool_address, _spool_bump) = spool_pda(*signer_info.key, &spool.name);

    spool_info.has_address(&spool_address)?;

    check_condition(
        spool.state.eq(&u64::from(SpoolState::Writing)),
        SpoolError::UnexpectedState,
    )?;

    spool.header = args.header;

    Ok(())
}

