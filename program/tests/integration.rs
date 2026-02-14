#![cfg(test)]
pub mod utils;
use utils::*;

use steel::Zeroable;
use solana_sdk::{
    signer::Signer,
    transaction::Transaction,
    pubkey::Pubkey,
    signature::Keypair,
    clock::Clock,
    instruction::Instruction,
};

use brine_tree::Leaf;
use spool::miner::get_base_rate;
use spool_api::prelude::*;
use spool_api::instruction;
use litesvm::LiteSVM;

use crankx::equix::SolverMemory;
use crankx::{
    solve_with_memory,
    Solution, 
    CrankXError
};

struct StoredReel {
    //number: u64,
    address: Pubkey,
    miner: Pubkey,
    tree: SpoolTree,
    spools: Vec<PackedSpool>,
    //account: Reel,
}

struct StoredSpool {
    number: u64,
    address: Pubkey,
    segments: Vec<Vec<u8>>,
    account: Spool,
}

struct PackedSpool {
    number: u64,
    address: Pubkey,
    tree: SegmentTree,
    data: Vec<Vec<u8>>,
}

#[test]
fn run_integration() {
    // Setup environment
    let (mut svm, payer) = setup_environment();

    // Initialize program
    initialize_program(&mut svm, &payer);

    // Register miner
    let miner_name = "miner-name";
    let miner_address = register_miner(&mut svm, &payer, miner_name);
    let ata = create_ata(&mut svm, &payer, &MINT_ADDRESS, &payer.pubkey());

    // Create a miner reel
    let reel_number = 1;
    let mut stored_reel = create_reel(&mut svm, &payer, miner_address, reel_number);

    // Fetch and store genesis spool
    let genesis_spool = get_genesis_spool(&mut svm, &payer);

    // Pack the spool into a miner specific representation
    pack_spool(&mut svm, &payer, &genesis_spool, &mut stored_reel);

    // Verify initial accounts
    verify_archive_account(&svm, 1);
    verify_epoch_account(&svm);
    verify_block_account(&svm);
    verify_treasury_account(&svm);
    verify_mint_account(&svm);
    verify_metadata_account(&svm);
    verify_treasury_ata(&svm);

    // Mine the genesis spool (to earn some tokens)
    do_mining_run(&mut svm, &payer, &stored_reel, 5);
    claim_rewards(&mut svm, &payer, miner_address, ata);

    let ata_balance = get_ata_balance(&svm, &ata);
    assert!(ata_balance > 0);

    println!("ATA balance after claiming rewards: {ata_balance}");

    // Advance clock
    let mut initial_clock = svm.get_sysvar::<Clock>();
    initial_clock.slot = 10;
    svm.set_sysvar::<Clock>(&initial_clock);

    // Create spools
    let spool_count = 5;
    for spool_index in 1..spool_count {
        let stored_spool = create_and_verify_spool(&mut svm, &payer, ata, spool_index);
        pack_spool(&mut svm, &payer, &stored_spool, &mut stored_reel);
    }

    // Verify archive account after spool creation
    verify_archive_account(&svm, spool_count);

    // Mine again with more spools this time
    do_mining_run(&mut svm, &payer, &stored_reel, 5);
}

fn setup_environment() -> (LiteSVM, Keypair) {
    let mut svm = setup_svm();
    let payer = create_payer(&mut svm);
    (svm, payer)
}

fn subsidize_spool(
    svm: &mut LiteSVM,
    payer: &Keypair,
    ata: Pubkey,
    spool_address: Pubkey,
    amount: u64,
) {
    let payer_pk = payer.pubkey();

    let blockhash = svm.latest_blockhash();
    let ix = instruction::spool::build_subsidize_ix(
        payer_pk, 
        ata, 
        spool_address, 
        amount
    );

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    let account = svm.get_account(&spool_address).unwrap();
    let spool = Spool::unpack(&account.data).unwrap();
    assert!(spool.balance >= amount);
}

fn claim_rewards(
    svm: &mut LiteSVM,
    payer: &Keypair,
    miner_address: Pubkey,
    miner_ata: Pubkey,
) {
    let payer_pk = payer.pubkey();

    let blockhash = svm.latest_blockhash();
    let ix = instruction::miner::build_claim_ix(
        payer_pk, 
        miner_address, 
        miner_ata, 
        0 // Claim all unclaimed rewards
    );

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    // Verify miner account after claiming rewards
    let account = svm.get_account(&miner_address).unwrap();
    let miner = Miner::unpack(&account.data).unwrap();

    assert!(miner.unclaimed_rewards == 0);
}

fn do_mining_run(
    svm: &mut LiteSVM,
    payer: &Keypair,
    stored_reel: &StoredReel,
    num_iterations: u64,
) {
    for _ in 0..num_iterations {
        // We need to expire the blockhash because we're not checking if the mining commitment
        // needs to change (when it doesn't, we get a AlreadyProcessed error). Todo, check before
        // submitting the transaction if the commitment is still valid.

        let mut current_clock = svm.get_sysvar::<Clock>();
        current_clock.slot += 10;
        svm.set_sysvar::<Clock>(&current_clock);
        svm.expire_blockhash();

        let (epoch_address, _epoch_bump) = epoch_pda();
        let epoch_account = svm.get_account(&epoch_address).unwrap();
        let epoch = Epoch::unpack(&epoch_account.data).unwrap();

        let (block_address, _block_bump) = block_pda();
        let block_account = svm.get_account(&block_address).unwrap();
        let block = Block::unpack(&block_account.data).unwrap();

        let miner_account = svm.get_account(&stored_reel.miner).unwrap();
        let miner = Miner::unpack(&miner_account.data).unwrap();

        let miner_challenge = compute_challenge(
            &block.challenge,
            &miner.challenge,
        );

        let recall_spool = compute_recall_spool(
            &miner_challenge,
            block.challenge_set
        );

        // Compute challenge solution (proof of work challenge)

        let spool_index = recall_spool - 1; // index in reel (not the spool_number)
        let packed_spool = &stored_reel.spools[spool_index as usize];
        let spool_account = svm.get_account(&packed_spool.address).unwrap();
        let spool = Spool::unpack(&spool_account.data).unwrap();

        // Check if we need to provide a PoA solution based on whether the spool has minimum rent.
        // (Note: We always need to provide a PoW solution)

        if spool.has_minimum_rent() {
            // We need to provide a PoA solution

            let miner_address = stored_reel.miner;
            let segment_number = compute_recall_segment(
                &miner_challenge, 
                spool.total_segments
            );

            // Unpack the whole spool 
            // (todo: this could be up to 32Mb and not really trival with ~262k segments)

            let mut leaves = Vec::new();
            let mut packed_segment = [0; packx::SOLUTION_SIZE];
            let mut unpacked_segment = [0; SEGMENT_SIZE];

            for (segment_id, packed_data) in packed_spool.data.iter().enumerate() {
                let mut data = [0u8; packx::SOLUTION_SIZE];
                data.copy_from_slice(&packed_data[..packx::SOLUTION_SIZE]);

                let solution = packx::Solution::from_bytes(&data);
                let segement_data = solution.unpack(&miner_address.to_bytes());

                let leaf = compute_leaf(
                    segment_id as u64,
                    &segement_data,
                );

                leaves.push(leaf);

                if segment_id == segment_number as usize {
                    packed_segment.copy_from_slice(&data);
                    unpacked_segment.copy_from_slice(&segement_data);
                }
            }

            assert_eq!(leaves.len(), spool.total_segments as usize);

            println!("Recall segment: {segment_number}");

            let poa_solution = packx::Solution::from_bytes(&packed_segment);
            let pow_solution = solve_challenge(miner_challenge, &unpacked_segment, epoch.mining_difficulty).unwrap();
            assert!(pow_solution.is_valid(&miner_challenge, &unpacked_segment).is_ok());

            let merkle_tree = SegmentTree::new(&[packed_spool.address.as_ref()]);
            let proof_nodes: Vec<[u8; 32]> = merkle_tree
                .get_proof(&leaves, segment_number as usize)
                .into_iter()
                .map(|h| h.to_bytes())
                .collect();

            let proof_path = ProofPath::from_slice(&proof_nodes)
                .expect("merkle proof must be exactly SEGMENT_PROOF_LEN long");

            let pow = PoW::from_solution(&pow_solution);
            let poa = PoA::from_solution(&poa_solution, proof_path);

            // Tx1: load the packed spool leaf from the reel onto the miner commitment field
            commit_for_mining(
                svm, 
                payer, 
                stored_reel, 
                spool_index, 
                segment_number
            );

            // Tx2: perform mining with PoW and PoA
            perform_mining(
                svm,
                payer,
                stored_reel.miner,
                packed_spool.address,
                pow,
                poa
            );

        } else {

            let solution = solve_challenge(
                miner_challenge, 
                &EMPTY_SEGMENT, 
                epoch.mining_difficulty
            ).unwrap();

            let pow = PoW::from_solution(&solution);
            let poa = PoA::zeroed();

            perform_mining(
                svm,
                payer,
                stored_reel.miner,
                packed_spool.address,
                pow,
                poa
            );
        }
    }
}

fn get_genesis_spool(svm: &mut LiteSVM, payer: &Keypair) -> StoredSpool {
    let genesis_name = "genesis".to_string();
    let genesis_name_bytes = to_name(&genesis_name);
    let (genesis_pubkey, _) = spool_pda(payer.pubkey(), &genesis_name_bytes);

    let account = svm.get_account(&genesis_pubkey).expect("Genesis spool should exist");
    let spool = Spool::unpack(&account.data).expect("Failed to unpack genesis spool");

    assert!(spool.can_finalize());

    let genesis_data = b"hello, world";
    let genesis_segment = padded_array::<SEGMENT_SIZE>(genesis_data).to_vec();
    let segments = vec![genesis_segment];

    

    StoredSpool {
        number: spool.number,
        address: genesis_pubkey,
        segments,
        account: *spool,
    }
}


fn initialize_program(svm: &mut LiteSVM, payer: &Keypair) {
    let payer_pk = payer.pubkey();
    let ix = instruction::program::build_initialize_ix(payer_pk);
    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());
}

fn verify_archive_account(svm: &LiteSVM, expected_spools_stored: u64) {
    let (archive_address, _archive_bump) = archive_pda();
    let account = svm
        .get_account(&archive_address)
        .expect("Archive account should exist");
    let archive = Archive::unpack(&account.data).expect("Failed to unpack Archive account");
    assert_eq!(archive.spools_stored, expected_spools_stored);
}

fn verify_epoch_account(svm: &LiteSVM) {
    let (epoch_address, _epoch_bump) = epoch_pda();
    let account = svm
        .get_account(&epoch_address)
        .expect("Epoch account should exist");
    let epoch = Epoch::unpack(&account.data).expect("Failed to unpack Epoch account");
    assert_eq!(epoch.number, 1);
    assert_eq!(epoch.progress, 0);
    assert_eq!(epoch.mining_difficulty, MIN_MINING_DIFFICULTY);
    assert_eq!(epoch.packing_difficulty, MIN_PACKING_DIFFICULTY);
    assert_eq!(epoch.target_participation, MIN_PARTICIPATION_TARGET);
    assert_eq!(epoch.reward_rate, get_base_rate(1));
    assert_eq!(epoch.duplicates, 0);
    assert_eq!(epoch.last_epoch_at, 0);
}

fn verify_block_account(svm: &LiteSVM) {
    let (block_address, _block_bump) = block_pda();
    let account = svm
        .get_account(&block_address)
        .expect("Block account should exist");
    let block = Block::unpack(&account.data).expect("Failed to unpack Block account");
    assert_eq!(block.number, 1);
    assert_eq!(block.progress, 0);
    assert_eq!(block.last_proof_at, 0);
    assert_eq!(block.last_block_at, 0);
    assert_eq!(block.challenge_set, 1);
    assert!(block.challenge.ne(&[0u8; 32]));
}

fn verify_treasury_account(svm: &LiteSVM) {
    let (treasury_address, _treasury_bump) = treasury_pda();
    let _treasury_account = svm
        .get_account(&treasury_address)
        .expect("Treasury account should exist");
}

fn verify_mint_account(svm: &LiteSVM) {
    let (mint_address, _mint_bump) = mint_pda();
    let mint = get_mint(svm, &mint_address);
    assert_eq!(mint.supply, MAX_SUPPLY);
    assert_eq!(mint.decimals, TOKEN_DECIMALS);
}

fn verify_metadata_account(svm: &LiteSVM) {
    let (mint_address, _mint_bump) = mint_pda();
    let (metadata_address, _metadata_bump) = metadata_pda(mint_address);
    let account = svm
        .get_account(&metadata_address)
        .expect("Metadata account should exist");
    assert!(!account.data.is_empty());
}

fn verify_treasury_ata(svm: &LiteSVM) {
    let (treasury_ata_address, _ata_bump) = treasury_ata();
    let account = svm
        .get_account(&treasury_ata_address)
        .expect("Treasury ATA should exist");
    assert!(!account.data.is_empty());
}

fn create_and_verify_spool(
    svm: &mut LiteSVM,
    payer: &Keypair,
    ata: Pubkey,
    spool_index: u64,
) -> StoredSpool {
    let payer_pk = payer.pubkey();
    let spool_name = format!("spool-name-{spool_index}");

    let (spool_address, _spool_bump) = spool_pda(payer_pk, &to_name(&spool_name));
    let (writer_address, _writer_bump) = writer_pda(spool_address);

    // Create spool and verify initial state
    let mut stored_spool = create_spool(
        svm, 
        payer, 
        &spool_name, 
        spool_address, 
        writer_address
    );

    let spool_seed = &[stored_spool.address.as_ref()];
    let mut writer_tree = SegmentTree::new(spool_seed);

    write_spool(
        svm,
        payer,
        spool_address,
        writer_address,
        &mut stored_spool,
        &mut writer_tree,
    );

    update_spool(
        svm,
        payer,
        spool_address,
        writer_address,
        &mut stored_spool,
        &mut writer_tree,
    );

    let min_rent = min_finalization_rent(
        stored_spool.account.total_segments,
    );

    subsidize_spool(
        svm, 
        payer, 
        ata,
        spool_address, 
        min_rent,
    );

    finalize_spool(
        svm,
        payer,
        spool_address,
        writer_address,
        &mut stored_spool,
        spool_index,
    );

    stored_spool
}

fn create_spool(
    svm: &mut LiteSVM,
    payer: &Keypair,
    spool_name: &str,
    spool_address: Pubkey,
    writer_address: Pubkey,
) -> StoredSpool {
    let payer_pk = payer.pubkey();

    // Create spool
    let blockhash = svm.latest_blockhash();
    let ix = instruction::spool::build_create_ix(payer_pk, spool_name);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    // Verify spool account
    let account = svm.get_account(&spool_address).unwrap();
    let spool = Spool::unpack(&account.data).unwrap();
    assert_eq!(spool.authority, payer_pk);
    assert_eq!(spool.name, to_name(spool_name));
    assert_eq!(spool.state, u64::from(SpoolState::Created));
    assert_eq!(spool.merkle_root, [0; 32]);
    assert_eq!(spool.header, [0; HEADER_SIZE]);
    assert_eq!(spool.number, 0);

    // Verify writer account
    let account = svm.get_account(&writer_address).unwrap();
    let writer = Writer::unpack(&account.data).unwrap();
    assert_eq!(writer.spool, spool_address);

    let writer_tree = SegmentTree::new(&[spool_address.as_ref()]);
    assert_eq!(writer.state, writer_tree);

    StoredSpool {
        number: 0,
        address: spool_address,
        segments: vec![],
        account: *spool,
    }
}

fn write_spool(
    svm: &mut LiteSVM,
    payer: &Keypair,
    spool_address: Pubkey,
    writer_address: Pubkey,
    stored_spool: &mut StoredSpool,
    writer_tree: &mut SegmentTree,
) {
    let payer_pk = payer.pubkey();

    for write_index in 0..5u64 {
        let data = format!("<segment_{write_index}_data>").into_bytes();

        let blockhash = svm.latest_blockhash();
        let ix = instruction::spool::build_write_ix(payer_pk, spool_address, writer_address, &data);
        let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
        let res = send_tx(svm, tx);
        assert!(res.is_ok());

        // Update local state
        let segments = data.chunks(SEGMENT_SIZE);
        for (segment_number, segment) in segments.enumerate() {
            let canonical_segment = padded_array::<SEGMENT_SIZE>(segment);

            assert!(write_segment(
                writer_tree,
                (stored_spool.segments.len() + segment_number) as u64,
                &canonical_segment,
            )
            .is_ok());

            stored_spool.segments.push(canonical_segment.to_vec());
        }

        // Verify writer state
        let account = svm.get_account(&writer_address).unwrap();
        let writer = Writer::unpack(&account.data).unwrap();
        assert_eq!(writer.state.get_root(), writer_tree.get_root());

        // Verify and update spool state
        let account = svm.get_account(&spool_address).unwrap();
        let spool = Spool::unpack(&account.data).unwrap();
        assert_eq!(spool.total_segments, stored_spool.segments.len() as u64);
        assert_eq!(spool.state, u64::from(SpoolState::Writing));
        assert_eq!(spool.merkle_root, writer_tree.get_root().to_bytes());
        assert_eq!(spool.header, stored_spool.account.header);

        // Update stored_spool.account
        stored_spool.account = *spool;
    }
}

fn update_spool(
    svm: &mut LiteSVM,
    payer: &Keypair,
    spool_address: Pubkey,
    writer_address: Pubkey,
    stored_spool: &mut StoredSpool,
    writer_tree: &mut SegmentTree,
) {
    let payer_pk = payer.pubkey();
    let target_segment: u64 = 0;

    // Reconstruct leaves for proof
    let mut leaves = Vec::new();
    for (segment_id, segment_data) in stored_spool.segments.iter().enumerate() {
        let data_array = padded_array::<SEGMENT_SIZE>(segment_data);
        let leaf = compute_leaf(
            segment_id as u64, 
            &data_array
        );
        leaves.push(leaf);
    }

    // Compute Merkle proof
    let proof_nodes: Vec<[u8; 32]> = writer_tree
        .get_proof(&leaves, target_segment as usize)
        .into_iter()
        .map(|h| h.to_bytes())
        .collect();

    let proof_path = ProofPath::from_slice(&proof_nodes)
        .expect("merkle proof must be exactly SEGMENT_PROOF_LEN long");

    // Prepare data
    let old_data_array: [u8; SEGMENT_SIZE] = stored_spool.segments[target_segment as usize]
        .clone()
        .try_into()
        .unwrap();
    let new_raw = b"<segment_0_updated>";
    let new_data_array = padded_array::<SEGMENT_SIZE>(new_raw);

    // Send update transaction
    let blockhash = svm.latest_blockhash();
    let ix = instruction::spool::build_update_ix(
        payer_pk,
        spool_address,
        writer_address,
        target_segment,
        old_data_array,
        new_data_array,
        proof_path,
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    // Update local tree
    assert!(update_segment(
        writer_tree,
        target_segment,
        &old_data_array,
        &new_data_array,
        &proof_path,
    )
    .is_ok());

    // Update stored spool segments
    stored_spool.segments[target_segment as usize] = new_data_array.to_vec();

    // Verify writer state
    let account = svm.get_account(&writer_address).unwrap();
    let writer = Writer::unpack(&account.data).unwrap();
    assert_eq!(writer.state, *writer_tree);

    // Verify and update spool state
    let account = svm.get_account(&spool_address).unwrap();
    let spool = Spool::unpack(&account.data).unwrap();
    assert_eq!(spool.total_segments, 5);
    assert_eq!(spool.state, u64::from(SpoolState::Writing));
    assert_eq!(spool.merkle_root, writer_tree.get_root().to_bytes());
    assert_eq!(spool.header, stored_spool.account.header);

    // Update stored_spool.account
    stored_spool.account = *spool;
}

fn finalize_spool(
    svm: &mut LiteSVM,
    payer: &Keypair,
    spool_address: Pubkey,
    writer_address: Pubkey,
    stored_spool: &mut StoredSpool,
    spool_index: u64,
) {
    let payer_pk = payer.pubkey();

    // Finalize spool
    let blockhash = svm.latest_blockhash();
    let ix = instruction::spool::build_finalize_ix(payer_pk, spool_address, writer_address);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    // Verify update fails after finalization
    let target_segment: u64 = 0;

    let old_data_array: [u8; SEGMENT_SIZE] = stored_spool.segments[target_segment as usize]
        .clone()
        .try_into()
        .unwrap();

    let new_raw = b"<segment_0_updated>";
    let new_data_array = padded_array::<SEGMENT_SIZE>(new_raw);
    let proof_path = ProofPath::default(); // Empty proof path, should fail due to state

    let blockhash = svm.latest_blockhash();
    let ix = instruction::spool::build_update_ix(
        payer_pk,
        spool_address,
        writer_address,
        target_segment,
        old_data_array,
        new_data_array,
        proof_path,
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_err());

    // Verify finalized spool
    let account = svm.get_account(&spool_address).unwrap();
    let spool = Spool::unpack(&account.data).unwrap();
    assert_eq!(spool.state, u64::from(SpoolState::Finalized));
    assert_eq!(spool.number, spool_index + 1);
    assert_eq!(spool.total_segments, 5);
    assert_eq!(spool.merkle_root, stored_spool.account.merkle_root);

    // Verify writer account is closed
    let account = svm.get_account(&writer_address).unwrap();
    assert!(account.data.is_empty());

    // Update stored_spool
    stored_spool.number = spool_index + 1;
}

fn register_miner(svm: &mut LiteSVM, payer: &Keypair, miner_name: &str) -> Pubkey {
    let payer_pk = payer.pubkey();
    let (miner_address, _miner_bump) = miner_pda(payer_pk, to_name(miner_name));

    let blockhash = svm.latest_blockhash();
    let ix = instruction::miner::build_register_ix(payer_pk, miner_name);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    let account = svm.get_account(&miner_address).unwrap();
    let miner = Miner::unpack(&account.data).unwrap();

    assert_eq!(miner.authority, payer_pk);
    assert_eq!(miner.name, to_name(miner_name));
    assert_eq!(miner.unclaimed_rewards, 0);
    assert_eq!(miner.multiplier, 0);
    assert_eq!(miner.last_proof_block, 0);
    assert_eq!(miner.last_proof_at, 0);
    assert_eq!(miner.total_proofs, 0);
    assert_eq!(miner.total_rewards, 0);

    miner_address
}

fn create_reel(svm: &mut LiteSVM, payer: &Keypair, miner_address: Pubkey, number: u64) -> StoredReel {
    let payer_pk = payer.pubkey();
    let (reel_address, _bump) = reel_pda(miner_address, number);

    let blockhash = svm.latest_blockhash();
    let ix = instruction::reel::build_create_ix(payer_pk, miner_address, number);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    let account = svm.get_account(&reel_address).unwrap();
    let reel = Reel::unpack(&account.data).unwrap();

    assert_eq!(reel.authority, payer_pk);
    assert_eq!(reel.number, number);
    assert_eq!(reel.contains, [0; 32]);
    assert_eq!(reel.total_spools, 0);
    assert_eq!(reel.last_proof_block, 0);
    assert_eq!(reel.last_proof_at, 0);

    StoredReel {
        //number,
        address: reel_address,
        miner: miner_address,
        tree: SpoolTree::new(&[reel_address.as_ref()]),
        spools: vec![],
        //account: *reel,
    }
}

fn get_packed_segments(
    miner_address: Pubkey,
    stored_spool: &StoredSpool,
    difficulty: u32,
) -> Vec<Vec<u8>> {
    let miner_bytes = miner_address.to_bytes();
    let mem = packx::build_memory(&miner_bytes);

    let mut packed_segments: Vec<Vec<u8>> = vec![];
    for segment_data in &stored_spool.segments {
        let canonical_segment = padded_array::<SEGMENT_SIZE>(segment_data);
        let solution = packx::solve_with_memory(
            &canonical_segment,
            &mem,
            difficulty
        ).expect("Failed to pack segment data");

        packed_segments.push(solution.to_bytes().to_vec());
    }

    packed_segments
}

fn get_packed_spool(
    miner_address: Pubkey,
    stored_spool: &StoredSpool,
    difficulty: u32,
) -> PackedSpool {

    let packed_segments = get_packed_segments(miner_address, stored_spool, difficulty);

    let mut merkle_tree = SegmentTree::new(&[stored_spool.address.as_ref()]);
    for (segment_number, packed_data) in packed_segments.iter().enumerate() {
        let segment_id = segment_number.to_le_bytes();
        let leaf = Leaf::new(&[
            segment_id.as_ref(),
            packed_data,
        ]);
        
        merkle_tree.try_add_leaf(leaf)
            .expect("Failed to add leaf to Merkle tree");
    }

    PackedSpool {
        number: stored_spool.number,
        address: stored_spool.address,
        tree: merkle_tree,
        data: packed_segments,
    }
}

fn commit_for_mining(
    svm: &mut LiteSVM,
    payer: &Keypair,
    stored_reel: &StoredReel,
    spool_index: u64,
    segment_index: u64,
) {
    let payer_pk = payer.pubkey();
    let blockhash = svm.latest_blockhash();

    let ix = [
        unpack_spool_ix(
            payer, 
            stored_reel, 
            spool_index
        ),
        commit_data_ix(
            payer, 
            stored_reel, 
            spool_index, 
            segment_index
        ),
    ];

    let tx = Transaction::new_signed_with_payer(&ix, Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);

    assert!(res.is_ok());

    // Verify that the mining account has the leaf we need
    let account = svm.get_account(&stored_reel.miner)
        .expect("Miner account should exist");
    let miner = Miner::unpack(&account.data)
        .expect("Failed to unpack Miner account");

    let leaf = Leaf::new(&[
        segment_index.to_le_bytes().as_ref(),
        &stored_reel.spools[spool_index as usize].data[segment_index as usize],
    ]);

    assert!(miner.commitment.eq(&leaf.to_bytes()));
}

fn commit_data_ix(
    payer: &Keypair,
    stored_reel: &StoredReel,
    spool_index: u64,
    segment_index: u64,
) -> Instruction {
    let payer_pk = payer.pubkey();

    let packed_spool = stored_reel.spools
        .get(spool_index as usize)
        .expect("Spool index out of bounds");

    let leaves = packed_spool.data.iter().enumerate()
        .map(|(segment_id, packed_data)| {
            Leaf::new(&[
                segment_id.to_le_bytes().as_ref(),
                packed_data.as_ref(),
            ])
        })
        .collect::<Vec<_>>();

    //let data = packed_spool.data[segment_index as usize].clone();

    let data = leaves[segment_index as usize]
        .to_bytes();

    let proof_nodes: Vec<[u8; 32]> = packed_spool.tree
        .get_proof(&leaves, segment_index as usize)
        .into_iter()
        .map(|h| h.to_bytes())
        .collect();

    let proof_path = ProofPath::from_slice(&proof_nodes)
        .expect("merkle proof must be exactly SEGMENT_PROOF_LEN long");

    instruction::reel::build_commit_ix(
        payer_pk,
        stored_reel.miner,
        stored_reel.address,
        spool_index,
        proof_path,
        data,
    )
}

fn unpack_spool_ix(
    payer: &Keypair,
    stored_reel: &StoredReel,
    index: u64,
) -> Instruction {
    let payer_pk = payer.pubkey();

    let packed_spool = stored_reel.spools
        .get(index as usize)
        .expect("Spool index out of bounds");
    let spool_root = packed_spool.tree.get_root();

    let leaves = stored_reel.spools.iter()
        .map(|s| {
            Leaf::new(&[
                s.number.to_le_bytes().as_ref(),
                s.tree.get_root().as_ref(),
            ])
        })
        .collect::<Vec<_>>();

    let merkle_proof = stored_reel.tree
        .get_proof(&leaves, index as usize);

    let merkle_proof = merkle_proof
        .iter()
        .map(|v| v.to_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    instruction::reel::build_unpack_ix(
        payer_pk,
        stored_reel.address,
        packed_spool.number,
        merkle_proof,
        spool_root.to_bytes(),
    )
}

fn pack_spool(
    svm: &mut LiteSVM,
    payer: &Keypair,
    stored_spool: &StoredSpool, 
    stored_reel: &mut StoredReel,
) {
    // Get the required difficulty for packing
    let (epoch_address, _epoch_bump) = epoch_pda();
    let epoch_account = svm.get_account(&epoch_address).unwrap();
    let epoch = Epoch::unpack(&epoch_account.data).unwrap();
    let difficulty = epoch.packing_difficulty as u32;

    // Compute packed spool for this miner
    let packed_spool = get_packed_spool(stored_reel.miner, stored_spool, difficulty);

    // Publicly commit the packed spool to the provided reel address
    let payer_pk = payer.pubkey();
    let blockhash = svm.latest_blockhash();
    let ix = instruction::reel::build_pack_ix(
        payer_pk,
        stored_reel.address,
        stored_spool.address,
        packed_spool.tree.get_root().to_bytes()
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    stored_reel.tree.try_add_leaf(
        Leaf::new(&[
            stored_spool.number.to_le_bytes().as_ref(),
            packed_spool.tree.get_root().as_ref(),
        ])
    ).expect("Failed to add leaf to reel tree");

    stored_reel.spools.push(packed_spool);
}


fn perform_mining(
    svm: &mut LiteSVM,
    payer: &Keypair,
    miner_address: Pubkey,
    spool_address: Pubkey,
    pow: PoW,
    poa: PoA,
) {
    let payer_pk = payer.pubkey();

    let blockhash = svm.latest_blockhash();
    let ix = instruction::miner::build_mine_ix(
        payer_pk,
        miner_address,
        spool_address,
        pow,
        poa,
    );

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    let account = svm.get_account(&miner_address).unwrap();
    let miner = Miner::unpack(&account.data).unwrap();
    assert!(miner.unclaimed_rewards > 0);
}

fn solve_challenge<const N: usize>(
    challenge: [u8; 32],
    data: &[u8; N],
    difficulty: u64,
) -> Result<Solution, CrankXError> {
    let mut memory = SolverMemory::new();
    let mut nonce: u64 = 0;

    loop {
        if let Ok(solution) = solve_with_memory(&mut memory, &challenge, data, &nonce.to_le_bytes()) {
            if solution.difficulty() >= difficulty as u32 {
                return Ok(solution);
            }
        }
        nonce += 1;
    }
}
