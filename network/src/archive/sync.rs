use anyhow::{anyhow, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_transaction_status_client_types::TransactionDetails;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::collections::HashSet;
use tokio::task::JoinSet;

use spool_api::{SEGMENT_SIZE, state::SpoolState};
use spool_client::{
    get_block_by_number, get_archive_account, get_spool_account, find_spool_account, init_read,
    process_next_block, get_epoch_account
};
use spool_client::utils::{process_block, ProcessedBlock};

use crate::store::*;
use crate::utils::peer;
use super::pack::pack_segment;
use super::queue::{Tx, SegmentJob};

/// Syncs missing spool addresses from either a trusted peer or Solana RPC.
pub async fn get_spool_addresses(
    store: &Arc<SpoolStore>,
    client: &Arc<RpcClient>,
    trusted_peer: Option<String>,
) -> Result<()> {
    log::debug!("Syncing missing spool addresses");
    log::debug!("This may take a while... please be patient");

    if let Some(peer_url) = trusted_peer {
        log::debug!("Using trusted peer: {}", peer_url);
        sync_addresses_from_trusted_peer(store, client, &peer_url).await?;
    } else {
        log::debug!("No trusted peer provided, syncing against Solana directly");
        sync_addresses_from_solana(store, client).await?;
    }

    Ok(())
}

/// Syncs segments from Solana RPC.
pub async fn sync_segments_from_solana(
    store: &SpoolStore,
    client: &Arc<RpcClient>,
    spool_address: &Pubkey,
    tx: &Tx,
) -> anyhow::Result<()> {
    let (spool, _) = get_spool_account(client, spool_address).await?;
    let mut state = init_read(spool.tail_slot);
    while process_next_block(client, spool_address, &mut state).await? {}

    let mut keys: Vec<u64> = state.segments.keys().cloned().collect();
    keys.sort();

    for seg_num in keys {
        if store.get_segment(spool_address, seg_num).is_ok() {
            continue;
        }

        let data = state.segments.remove(&seg_num).ok_or_else(|| anyhow::anyhow!("Segment data missing"))?;
        let job = SegmentJob {
            spool: *spool_address,
            seg_no: seg_num,
            data,
        };
        if tx.send(job).await.is_err() {
            return Err(anyhow::anyhow!("Channel closed"));
        }
    }

    Ok(())
}

///// Syncs segments from a trusted peer.
//pub async fn sync_segments_from_trusted_peer(
//    store: &SpoolStore,
//    spool_address: &Pubkey,
//    trusted_peer_url: &str,
//    tx: &Tx,
//) -> anyhow::Result<()> {
//    let http = HttpClient::new();
//    let segments = crate::utils::peer::fetch_spool_segments(&http, trusted_peer_url, spool_address).await?;
//
//    for (seg_num, data) in segments {
//        if store.get_segment(spool_address, seg_num).is_ok() {
//            continue;
//        }
//
//        let job = SegmentJob {
//            spool: *spool_address,
//            seg_no: seg_num,
//            data,
//        };
//        if tx.send(job).await.is_err() {
//            return Err(anyhow::anyhow!("Channel closed"));
//        }
//    }
//
//    Ok(())
//}

/// Syncs spool addresses from a trusted peer.
pub async fn sync_addresses_from_trusted_peer(
    store: &Arc<SpoolStore>,
    client: &Arc<RpcClient>,
    trusted_peer_url: &str,
) -> Result<()> {
    let (archive, _) = get_archive_account(client).await?;
    let total = archive.spools_stored;
    let http = reqwest::Client::new();
    let mut tasks = JoinSet::new();
    let mut spool_pubkeys_with_numbers = Vec::with_capacity(total as usize);

    for spool_number in 1..=total {

        if store.get_spool_address(spool_number).is_ok() {
            continue;
        }

        if tasks.len() >= 10 {
            if let Some(Ok(Ok((pubkey, number)))) = tasks.join_next().await {
                spool_pubkeys_with_numbers.push((pubkey, number));
            }
        }

        let trusted_peer_url = trusted_peer_url.to_string();
        let http = http.clone();
        tasks.spawn(async move {
            let pubkey = peer::fetch_spool_address(&http, &trusted_peer_url, spool_number).await?;
            Ok((pubkey, spool_number))
        });
    }

    let results: Vec<Result<(Pubkey, u64), anyhow::Error>> = tasks.join_all().await;
    let pairs: Vec<(Pubkey, u64)> = results.into_iter().filter_map(|r| r.ok()).collect();
    spool_pubkeys_with_numbers.extend(pairs.into_iter());

    for (pubkey, number) in spool_pubkeys_with_numbers {
        store.put_spool_address(number, &pubkey)?;
    }

    Ok(())
}

/// Syncs spool addresses from Solana RPC.
pub async fn sync_addresses_from_solana(
    store: &Arc<SpoolStore>,
    client: &Arc<RpcClient>
    ) -> Result<()> {
    let (archive, _) = get_archive_account(client).await?;
    let total = archive.spools_stored;
    let mut tasks = JoinSet::new();
    let mut spool_pubkeys_with_numbers = Vec::with_capacity(total as usize);

    for spool_number in 1..=total {
        if store.get_spool_address(spool_number).is_ok() {
            continue;
        }

        if tasks.len() >= 10 {
            if let Some(Ok(Ok((pubkey, number)))) = tasks.join_next().await {
                spool_pubkeys_with_numbers.push((pubkey, number));
            }
        }

        let client = client.clone();
        tasks.spawn(async move {
            let (pubkey, _) = find_spool_account(&client, spool_number)
                .await?
                .ok_or(anyhow::anyhow!("Spool account not found for number {}", spool_number))?;
            Ok((pubkey, spool_number))
        });
    }

    let results: Vec<Result<(Pubkey, u64), anyhow::Error>> = tasks.join_all().await;
    let pairs: Vec<(Pubkey, u64)> = results.into_iter().filter_map(|r| r.ok()).collect();
    spool_pubkeys_with_numbers.extend(pairs.into_iter());

    for (pubkey, number) in spool_pubkeys_with_numbers {
        store.put_spool_address(number, &pubkey)?;
    }

    Ok(())
}

/// Syncs block data for a specific spool address starting from a given slot.
pub async fn sync_from_block(
    store: &Arc<SpoolStore>,
    client: &Arc<RpcClient>,
    spool_address: &Pubkey,
    miner_address: &Pubkey,
    starting_slot: u64,
) -> Result<()> {
    let mut visited: HashSet<u64> = HashSet::new();
    let mut stack: Vec<u64> = Vec::new();

    let miner_bytes = miner_address.to_bytes();
    let mem = Arc::new(packx::build_memory(&miner_bytes));

    // Ensure the spool address is stored if finalized
    let (spool, _) = get_spool_account(client, spool_address).await?;
    if spool.state == u64::from(SpoolState::Finalized) {
        store.put_spool_address(spool.number, spool_address)?;
    }

    stack.push(starting_slot);

    while let Some(current_slot) = stack.pop() {
        if !visited.insert(current_slot) {
            continue; // Skip if already visited
        }

        log::debug!("Processing slot: {}", current_slot);

        let block = get_block_by_number(client, current_slot, TransactionDetails::Full).await?;
        let ProcessedBlock {
            segment_writes,
            slot,
            finalized_spools,
        } = process_block(block, current_slot)?;

        log::debug!(
            "Slot: {}, Finalized Spools: {}, Segment Writes: {} ({} bytes)",
            slot,
            finalized_spools.len(),
            segment_writes.len(),
            SEGMENT_SIZE * segment_writes.len()
        );

        // Note: we won't usually hit this during a manual re-sync, the tail_slot in the spool
        // account doesn't include the finalized slot. This is here to catch any other spool
        // finalizations that may have occurred (not for the current spool)

        if finalized_spools.is_empty() && segment_writes.is_empty() {
            continue; // Skip empty blocks
        }

        for (pubkey, number) in finalized_spools {
            store.put_spool_address(number, &pubkey)?;
        }

        let mut parents: HashSet<u64> = HashSet::new();

        for key in segment_writes.keys() {
            if key.address != *spool_address {
                continue;
            }

            if key.prev_slot != 0 {
                if key.prev_slot > slot {
                    return Err(anyhow!("Parent slot must be earlier than current slot"));
                }
                parents.insert(key.prev_slot);
            }
        }

        // Fetch packing difficulty for the current slot
        let epoch = get_epoch_account(client)
            .await
            .map_err(|e| anyhow!("Failed to get epoch account: {}", e))?.0;

        for (key, data) in segment_writes {
            if key.address != *spool_address {
                continue;
            }

            pack_segment(
                store,
                &mem,
                miner_address,
                &key.address, 
                data,
                key.segment_number,
                epoch.packing_difficulty,
            )?;
        }

        for parent in parents {
            stack.push(parent);
        }
    }

    Ok(())
}
