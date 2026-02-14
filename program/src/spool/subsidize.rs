use spool_api::prelude::*;
use spool_api::instruction::spool::Subsidize;
use steel::*;

pub fn process_spool_subsidize_rent(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = Subsidize::try_from_bytes(data)?;
    let [
        signer_info, 
        ata_info,
        spool_info,
        treasury_ata_info, 
        token_program_info, 
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    signer_info.is_signer()?;

    // We don't require the owner of the spool to be the 
    // signer; anyone can subsidize any spool.
    let spool = spool_info
        .as_account_mut::<Spool>(&spool_api::ID)?;

    treasury_ata_info
        .is_writable()?;

    token_program_info
        .is_program(&spl_token::ID)?;

    let amount = u64::from_le_bytes(args.amount);

    transfer(
        signer_info,
        ata_info,
        treasury_ata_info,
        token_program_info,
        amount,
    )?;

    spool.balance = spool.balance.saturating_add(amount);

    Ok(())
}

