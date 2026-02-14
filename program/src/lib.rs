#![allow(unexpected_cfgs)]

pub mod spool;
pub mod miner;
pub mod reel;
pub mod program;

use spool::*;
use miner::*;
use reel::*;
use program::*;

use spool_api::instruction::{
    spool::SpoolInstruction,
    miner::MinerInstruction,
    program::ProgramInstruction,
    reel::ReelInstruction,
};
use steel::*;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let (discriminator, data) = parse_instruction(&spool_api::ID, program_id, data)?;

    let ix_type = if let Ok(instruction) = ProgramInstruction::try_from_primitive(discriminator) {
        format!("ProgramInstruction::{:?}", instruction)
    } else if let Ok(instruction) = SpoolInstruction::try_from_primitive(discriminator) {
        format!("SpoolInstruction::{:?}", instruction)
    } else if let Ok(instruction) = MinerInstruction::try_from_primitive(discriminator) {
        format!("MinerInstruction::{:?}", instruction)
    } else if let Ok(instruction) = ReelInstruction::try_from_primitive(discriminator) {
        format!("ReelInstruction::{:?}", instruction)
    } else {
        format!("Invalid (discriminator: {})", discriminator)
    };

    solana_program::msg!("Instruction: {}", ix_type);

    if let Ok(ix) = ProgramInstruction::try_from_primitive(discriminator) {
        match ix {
            ProgramInstruction::Initialize => process_initialize(accounts, data)?,
            #[cfg(feature = "airdrop")]
            ProgramInstruction::Airdrop => process_airdrop(accounts, data)?,
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    } else if let Ok(ix) = SpoolInstruction::try_from_primitive(discriminator) {
        match ix {
            SpoolInstruction::Create => process_spool_create(accounts, data)?,
            SpoolInstruction::Write => process_spool_write(accounts, data)?,
            SpoolInstruction::Update => process_spool_update(accounts, data)?,
            SpoolInstruction::Finalize => process_spool_finalize(accounts, data)?,
            SpoolInstruction::SetHeader => process_spool_set_header(accounts, data)?,
            SpoolInstruction::Subsidize => process_spool_subsidize_rent(accounts, data)?,
        }
    } else if let Ok(ix) = MinerInstruction::try_from_primitive(discriminator) {
        match ix {
            MinerInstruction::Register => process_register(accounts, data)?,
            MinerInstruction::Unregister => process_unregister(accounts, data)?,
            MinerInstruction::Mine => process_mine(accounts, data)?,
            MinerInstruction::Claim => process_claim(accounts, data)?,
        }
     } else if let Ok(ix) = ReelInstruction::try_from_primitive(discriminator) {
         match ix {
            ReelInstruction::Create => process_reel_create(accounts, data)?,
            ReelInstruction::Destroy => process_reel_destroy(accounts, data)?,
            ReelInstruction::Pack => process_reel_pack(accounts, data)?,
            ReelInstruction::Unpack => process_reel_unpack(accounts, data)?,
            ReelInstruction::Commit => process_reel_commit(accounts, data)?,
         }
    } else {
        return Err(ProgramError::InvalidInstructionData);
    }

    Ok(())
}

entrypoint!(process_instruction);
