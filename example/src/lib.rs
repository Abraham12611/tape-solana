#![allow(unexpected_cfgs)]
#![allow(unused)]

use spool_api::instruction::spool::*;
use steel::*;

declare_id!("Gzuu6orA9tz2ifE7zyupNiuhogYkRBmbuQpWJme5dGhJ"); 

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [
        signer_info, 
        spool_info,
        writer_info, 
        spool_program_info,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    solana_program::msg!("<Your program functionality here>");

    let your_data = vec![42; 1024]; // (you can be creative here)
    let ix = &build_write_ix(
        *signer_info.key,
        *spool_info.key,
        *writer_info.key,
        &your_data
    );

    solana_program::program::invoke(ix, accounts);

    Ok(())
}

entrypoint!(process_instruction);

