use steel::*;
use crate::{
    consts::*,
    pda::*,
    types::*,
};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum ReelInstruction {
    Create = 0x40,   // Create a reel to store spools
    Destroy,         // Destroy a reel, returning the rent to the miner
    Pack,            // Pack a spool into the reel
    Unpack,          // Unpack a spool from the reel
    Commit,          // Commit a solution for mining
}

instruction!(ReelInstruction, Create);
instruction!(ReelInstruction, Destroy);
instruction!(ReelInstruction, Pack);
instruction!(ReelInstruction, Unpack);
instruction!(ReelInstruction, Commit);

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Create {
    pub number: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Destroy {
    pub number: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Pack {
    pub value: [u8; 32]
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Unpack {
    pub index: [u8; 8],
    pub proof: [[u8; 32]; SPOOL_PROOF_LEN],
    pub value: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Commit {
    pub index: [u8; 8],
    pub proof: ProofPath,
    pub value: [u8; 32],
}

pub fn build_create_ix(
    signer: Pubkey, 
    miner_address: Pubkey, 
    number: u64,
) -> Instruction {
    let (reel_address, _bump) = reel_pda(miner_address, number);

    Instruction {
        program_id: crate::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(miner_address, false),
            AccountMeta::new(reel_address, false),
            AccountMeta::new_readonly(solana_program::system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: Create {
            number: number.to_le_bytes(),
        }.to_bytes(),
    }
}

pub fn build_destroy_ix(
    signer: Pubkey, 
    miner_address: Pubkey, 
    number: u64,
) -> Instruction {
    let (reel_address, _bump) = reel_pda(miner_address, number);

    Instruction {
        program_id: crate::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(miner_address, false),
            AccountMeta::new(reel_address, false),
            AccountMeta::new_readonly(solana_program::system_program::ID, false),
        ],
        data: Destroy {
            number: number.to_le_bytes(),
        }.to_bytes(),
    }
}

pub fn build_pack_ix(
    signer: Pubkey, 
    reel_address: Pubkey,
    spool_address: Pubkey, 
    value: [u8; 32],
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(reel_address, false),
            AccountMeta::new_readonly(spool_address, false),
        ],
        data: Pack {
            value,
        }.to_bytes(),
    }
}

pub fn build_unpack_ix(
    signer: Pubkey, 
    reel_address: Pubkey,
    index: u64,                           // index of the value to unpack
    proof: [[u8; 32]; SPOOL_PROOF_LEN],   // proof of the value
    value: [u8; 32],                      // value to unpack
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(reel_address, false),
        ],
        data: Unpack {
            index: index.to_le_bytes(),
            proof,
            value,
        }.to_bytes(),
    }
}

pub fn build_commit_ix(
    signer: Pubkey, 
    miner_address: Pubkey, 
    reel_address: Pubkey,
    index: u64,                           // index of the value to commit
    proof: ProofPath,                     // proof of the value
    value: [u8; 32],                      // value to commit
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(miner_address, false),
            AccountMeta::new_readonly(reel_address, false),
        ],
        data: Commit {
            index: index.to_le_bytes(),
            proof,
            value,
        }.to_bytes(),
    }
}
