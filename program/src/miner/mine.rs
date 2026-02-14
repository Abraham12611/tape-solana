use brine_tree::{Leaf, verify};
use steel::*;
use spool_api::prelude::*;
use spool_api::instruction::miner::Mine;
use solana_program::msg;

const EPOCHS_PER_YEAR: u64 = 365 * 24 * 60 / EPOCH_BLOCKS;

pub fn process_mine(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    msg!("Starting mine instruction processing");
    let current_time = Clock::get()?.unix_timestamp;
    msg!("data size: {}", data.len());
    let args = Mine::try_from_bytes(data)?;

    let [
        signer_info, 
        epoch_info, 
        block_info,
        miner_info, 
        spool_info,
        archive_info,
        slot_hashes_info,
    ] = accounts else { 
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    signer_info.is_signer()?;
    msg!("Verified signer");

    let archive = archive_info
        .is_archive()?
        .as_account_mut::<Archive>(&spool_api::ID)?;
    msg!("Loaded archive account");

    let epoch = epoch_info
        .is_epoch()?
        .as_account_mut::<Epoch>(&spool_api::ID)?;
    msg!("Loaded epoch account");

    let block = block_info
        .is_block()?
        .as_account_mut::<Block>(&spool_api::ID)?;
    msg!("Loaded block account");

    let spool = spool_info
        .as_account_mut::<Spool>(&spool_api::ID)?;
    msg!("Loaded spool account");

    let miner = miner_info
        .as_account_mut::<Miner>(&spool_api::ID)?;
    msg!("Loaded miner account");

    let (miner_address, _miner_bump) = miner_pda(miner.authority, miner.name);
    msg!("Computed miner PDA: {}", miner_address);

    check_condition(
        miner_info.key.eq(&miner_address),
        ProgramError::InvalidSeeds
    )?;
    msg!("Verified miner PDA");

    check_condition(
        signer_info.key.eq(&miner.authority),
        ProgramError::InvalidAccountOwner,
    )?;
    msg!("Verified signer is miner authority");

    slot_hashes_info.is_sysvar(&sysvar::slot_hashes::ID)?;
    msg!("Verified slot hashes sysvar");

    check_submission(miner, block, epoch, current_time)?;
    msg!("Submission checks passed");

    let miner_challenge = compute_challenge(
        &block.challenge,
        &miner.challenge,
    );
    msg!("Computed miner challenge");

    let spool_number = compute_recall_spool(
        &miner_challenge,
        block.challenge_set,
    );
    msg!("Computed recall spool number: {}", spool_number);

    check_condition(
        spool.number == spool_number,
        SpoolError::UnexpectedSpool,
    )?;
    msg!("Verified spool number");

    verify_solution(
        epoch,
        spool,
        &miner_address,
        &miner_challenge,
        &miner.commitment,
        args.pow,
        args.poa,
    )?;
    msg!("Verified solution");

    // Update miner
    update_multiplier(miner, block);
    msg!("Updated miner multiplier");

    let next_challenge = compute_next_challenge(
        &miner.challenge,
        slot_hashes_info
    );
    msg!("Computed next miner challenge");

    let reward = calculate_reward(
        epoch,
        spool,
        miner.multiplier
    );
    msg!("Calculated reward: {}", reward);

    update_miner_state(
        miner,
        block,
        reward,
        current_time,
        next_challenge,
    );
    msg!("Updated miner state");

    // Update spool
    update_spool_balance(spool, block.number);
    msg!("Updated spool balance for block: {}", block.number);

    // Update block
    block.progress = block.progress
        .saturating_add(1);
    msg!("Incremented block progress: {}", block.progress);

    if block.progress >= epoch.target_participation {
        advance_block(block, current_time)?;
        msg!("Advanced block due to sufficient progress");

        let next_block_challenge = compute_next_challenge(
            &block.challenge,
            slot_hashes_info
        );
        msg!("Computed next block challenge");

        block.challenge = next_block_challenge;
        block.challenge_set = archive.spools_stored;
        msg!("Updated block challenge and challenge set");
    }

    update_epoch(epoch, archive, current_time)?;
    msg!("Updated epoch state");

    msg!("Mine instruction processed successfully");
    Ok(())
}

fn check_submission(
    miner: &Miner,
    block: &Block,
    epoch: &mut Epoch,
    current_time: i64,
) -> ProgramResult {

    // Check if the proof is too early, just in case someone aquires insane hardware
    // and can solve the challenge faster than we can adjust the difficulty.

    // let min_block_time = block.last_proof_at
    //     .saturating_add(BLOCK_DURATION_SECONDS as i64 / 2);
    //
    // if current_time < min_block_time {
    //     return Err(SpoolError::SolutionTooEarly.into());
    // }

    if miner.last_proof_block == block.number {
        if has_stalled(block, current_time) {
            epoch.duplicates = epoch.duplicates.saturating_add(1);
            Ok(())
        } else {
            Err(SpoolError::SolutionInvalid.into())
        }
    } else {
        Ok(())
    }
}

fn verify_solution(
    epoch: &Epoch,
    spool: &Spool,
    miner_address: &Pubkey,
    miner_challenge: &[u8; 32],
    _miner_commitment: &[u8; 32],
    pow: PoW,
    poa: PoA,
) -> ProgramResult {

    let pow_solution = pow.as_solution();
    let poa_solution = poa.as_solution();

    let pow_difficulty = pow_solution.difficulty() as u64;
    let poa_difficulty = poa_solution.difficulty() as u64;

    check_condition(
        pow_difficulty >= epoch.mining_difficulty,
        SpoolError::SolutionTooEasy,
    )?;

    check_condition(
        poa_difficulty >= epoch.packing_difficulty,
        SpoolError::SolutionTooEasy,
    )?;

    // Check if the spool can be mined.
    if spool.has_minimum_rent() {
        solana_program::msg!("spool has minimum rent for mining");

        let segment_number = compute_recall_segment(
            miner_challenge, 
            spool.total_segments
        );

        let merkle_proof   = poa.path.as_ref();
        let merkle_root    = spool.merkle_root;
        let recall_segment = poa_solution.unpack(&miner_address.to_bytes());

        assert!(merkle_proof.len() == SEGMENT_PROOF_LEN);

        let leaf = Leaf::new(&[
            segment_number.to_le_bytes().as_ref(),
            recall_segment.as_ref(),
        ]);

        let _packed_leaf = Leaf::new(&[
            segment_number.to_le_bytes().as_ref(),
            poa_solution.to_bytes().as_ref(),
        ]);

        // TODO: enable this check once reel management is live in the protocol
        //check_condition(
        //    miner_commitment.eq(&packed_leaf.to_bytes()),
        //    SpoolError::CommitmentMismatch,
        //)?;

        check_condition(
            verify(
                merkle_root,
                merkle_proof,
                leaf
            ),
            SpoolError::SolutionInvalid,
        )?;

        // Verify PoW using the actual recalled segment
        check_condition(
            pow_solution.is_valid(miner_challenge, &recall_segment).is_ok(),
            SpoolError::SolutionInvalid,
        )?;

    // For expired spools, enforce use of the fixed segment (no storage needed)
    } else {
        solana_program::msg!("spool rent has expired, checking against fixed/empty segment");

        // Verify PoW using the fixed segment
        check_condition(
            pow_solution.is_valid(miner_challenge, &EMPTY_SEGMENT).is_ok(),
            SpoolError::SolutionInvalid,
        )?;
    }

    Ok(())
}

fn calculate_reward(epoch: &Epoch, spool: &Spool, multiplier: u64) -> u64 {
    // Divide the scaled reward by the target participation, each miner gets an equal share
    let available_reward = epoch.reward_rate
        .saturating_div(epoch.target_participation);

    // Scale the reward based on miner's consistency multiplier
    let scaled_reward = get_scaled_reward(
        available_reward,
        multiplier
    );

    // If the spool is subsidized, miners get the full reward.
    if spool.has_minimum_rent() {
        scaled_reward
    } else {
        scaled_reward
            .saturating_div(2)
    }
}

fn update_miner_state(
    miner: &mut Miner,
    block: &Block,
    final_reward: u64,
    current_time: i64,
    next_miner_challenge: [u8; 32],
) {
    miner.unclaimed_rewards   += final_reward;
    miner.total_rewards       += final_reward;
    miner.total_proofs        += 1;
    miner.last_proof_at        = current_time;
    miner.last_proof_block     = block.number;
    miner.challenge            = next_miner_challenge;
}

fn update_spool_balance(spool: &mut Spool, block_number: u64) {
    let rent = spool.rent_owed(block_number);

    spool.balance = spool.balance
        .saturating_sub(rent);
}

fn update_epoch(
    epoch: &mut Epoch,
    archive: &Archive,
    current_time: i64,
) -> ProgramResult {

    // Check if we need to advance the epoch
    if epoch.progress >= EPOCH_BLOCKS {
        advance_epoch(epoch, current_time)?;

        let base_rate     = get_base_rate(epoch.number);
        let storage_rate  = archive.block_reward();

        epoch.reward_rate = storage_rate
            .saturating_add(base_rate);

    // Epoch is still in progress, increment the progress
    } else {
        epoch.progress = epoch.progress
            .saturating_add(1);
    }

    Ok(())
}

// Helper: Check if the block has stalled, meaning no solutions have been submitted for a while.
fn has_stalled(block: &Block, current_time: i64) -> bool {
    current_time > block.last_proof_at
        .saturating_add(BLOCK_DURATION_SECONDS as i64)
}

// Helper: Update miner multiplier based on timing of this solution.
//
// Miners that consistently submit solutions on-time will have a higher multiplier number.
//
// This encourages miners to come up with strategies that allow them quick access to the spool data
// needed to solve the challenge.
fn update_multiplier(miner: &mut Miner, block: &Block) {
    if miner.last_proof_block.saturating_add(1) == block.number {
        miner.multiplier = miner.multiplier
            .saturating_add(1)
            .min(MAX_CONSISTENCY_MULTIPLIER);
    } else {
        miner.multiplier = miner.multiplier
            .saturating_sub(1)
            .max(MIN_CONSISTENCY_MULTIPLIER);
    }
}

// Helper: Get the scaled reward based on miner's consistency multiplier.
fn get_scaled_reward(reward: u64, multiplier: u64) -> u64 {
    assert!(multiplier >= MIN_CONSISTENCY_MULTIPLIER);
    assert!(multiplier <= MAX_CONSISTENCY_MULTIPLIER);
    
    reward
        .saturating_mul(multiplier)
        .saturating_div(MAX_CONSISTENCY_MULTIPLIER)
}

// Helper: Advance the block state
fn advance_block(
    block: &mut Block,
    current_time: i64,
) -> ProgramResult {

    // Reset the block state
    block.number            = block.number.saturating_add(1);
    block.progress          = 0;
    block.last_proof_at     = current_time;
    block.last_block_at     = current_time;

    Ok(())
}

// Helper: Advance the epoch state
fn advance_epoch(
    epoch: &mut Epoch,
    current_time: i64,
) -> ProgramResult {

    adjust_participation(epoch);
    adjust_difficulty(epoch, current_time);

    epoch.number                = epoch.number.saturating_add(1);
    epoch.mining_difficulty     = epoch.mining_difficulty.max(MIN_MINING_DIFFICULTY);
    epoch.target_participation  = epoch.target_participation.max(MIN_PARTICIPATION_TARGET);
    epoch.progress              = 0;
    epoch.duplicates            = 0;
    epoch.last_epoch_at         = current_time;

    Ok(())
}


// Every epoch, the protocol adjusts the minimum required difficulty for a block solution.
//
// Proof Difficulty:
// If blocks were solved faster than 1 minute on average, increase difficulty.
// If blocks were slower, decrease difficulty.
//
// This keeps block times near the 1-minute target.
fn adjust_difficulty(epoch: &mut Epoch, current_time: i64) {

    let elapsed_time = current_time.saturating_sub(epoch.last_epoch_at);
    let average_time_per_block = elapsed_time / EPOCH_BLOCKS as i64;

    // If blocks were solved faster than 1 minute, increase difficulty
    if average_time_per_block < BLOCK_DURATION_SECONDS as i64 {
        epoch.mining_difficulty = epoch.mining_difficulty
            .saturating_add(1)

    // If they were slower, decrease difficulty
    } else {
        epoch.mining_difficulty = epoch.mining_difficulty
            .saturating_sub(1)
            .max(MIN_MINING_DIFFICULTY);
    }
}

// Every epoch, the protocol adjusts the minimum required unique proofs for a single block. This
// is referred to as the participation target. We allow increasing only every ADJUSTMENT_INTERVAL
// epochs while decreasing can happen every epoch. This helps keep the blocks going in case of a
// large drop in participation.
//
// Participation Target (X):
// * If all submissions during the epoch came from unique miners, increase X by 1.
// * If any duplicates occurred (same miner submitting multiple times in a block), decrease X by 1.
//
// This helps tune how many miners can share in a block reward, balancing inclusivity and competitiveness.
fn adjust_participation(epoch: &mut Epoch) {

    // If all miner submissions were unique, increase by 1
    if epoch.duplicates == 0 {
        if (epoch.number % ADJUSTMENT_INTERVAL) == 0 {
            epoch.target_participation = epoch.target_participation
                .saturating_add(1)
                .min(MAX_PARTICIPATION_TARGET);
        }

    // If there were duplicates, decrease target by 1 (regardless of the ADJUSTMENT_INTERVAL)
    } else {
        epoch.target_participation = epoch.target_participation
            .saturating_sub(1)
            .max(MIN_PARTICIPATION_TARGET);
    }
}

/// Pre-computed base rate based on current epoch number. After which, the archive
/// storage fees would take over, with no further inflation.
///
/// The hard-coded values avoid CU overhead.
#[inline(always)]
pub fn get_base_rate(current_epoch: u64) -> u64 {
    match current_epoch {
        n if n < EPOCHS_PER_YEAR   => 10000000000, // Year ~1,  about 1.00 SPOOL/min
        n if n < 2 * EPOCHS_PER_YEAR   => 7500000000,  // Year ~2,  about 0.75 SPOOL/min
        n if n < 3 * EPOCHS_PER_YEAR   => 5625000000,  // Year ~3,  about 0.56 SPOOL/min
        n if n < 4 * EPOCHS_PER_YEAR   => 4218750000,  // Year ~4,  about 0.42 SPOOL/min
        n if n < 5 * EPOCHS_PER_YEAR   => 3164062500,  // Year ~5,  about 0.32 SPOOL/min
        n if n < 6 * EPOCHS_PER_YEAR   => 2373046875,  // Year ~6,  about 0.24 SPOOL/min
        n if n < 7 * EPOCHS_PER_YEAR   => 1779785156,  // Year ~7,  about 0.18 SPOOL/min
        n if n < 8 * EPOCHS_PER_YEAR   => 1334838867,  // Year ~8,  about 0.13 SPOOL/min
        n if n < 9 * EPOCHS_PER_YEAR   => 1001129150,  // Year ~9,  about 0.10 SPOOL/min
        n if n < 10 * EPOCHS_PER_YEAR  => 750846862,   // Year ~10, about 0.08 SPOOL/min
        n if n < 11 * EPOCHS_PER_YEAR  => 563135147,   // Year ~11, about 0.06 SPOOL/min
        n if n < 12 * EPOCHS_PER_YEAR  => 422351360,   // Year ~12, about 0.04 SPOOL/min
        n if n < 13 * EPOCHS_PER_YEAR  => 316763520,   // Year ~13, about 0.03 SPOOL/min
        n if n < 14 * EPOCHS_PER_YEAR  => 237572640,   // Year ~14, about 0.02 SPOOL/min
        n if n < 15 * EPOCHS_PER_YEAR  => 178179480,   // Year ~15, about 0.02 SPOOL/min
        n if n < 16 * EPOCHS_PER_YEAR  => 133634610,   // Year ~16, about 0.01 SPOOL/min
        n if n < 17 * EPOCHS_PER_YEAR  => 100225957,   // Year ~17, about 0.01 SPOOL/min
        n if n < 18 * EPOCHS_PER_YEAR  => 75169468,    // Year ~18, about 0.01 SPOOL/min
        n if n < 19 * EPOCHS_PER_YEAR  => 56377101,    // Year ~19, about 0.01 SPOOL/min
        n if n < 20 * EPOCHS_PER_YEAR  => 42282825,    // Year ~20, about 0.00 SPOOL/min
        n if n < 21 * EPOCHS_PER_YEAR  => 31712119,    // Year ~21, about 0.00 SPOOL/min
        n if n < 22 * EPOCHS_PER_YEAR  => 23784089,    // Year ~22, about 0.00 SPOOL/min
        n if n < 23 * EPOCHS_PER_YEAR  => 17838067,    // Year ~23, about 0.00 SPOOL/min
        n if n < 24 * EPOCHS_PER_YEAR  => 13378550,    // Year ~24, about 0.00 SPOOL/min
        n if n < 25 * EPOCHS_PER_YEAR  => 10033913,    // Year ~25, about 0.00 SPOOL/min
        _ => 0,
    }
}
