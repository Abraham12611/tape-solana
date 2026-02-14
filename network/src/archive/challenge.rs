use std::sync::Arc;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

use spool_api::prelude::*;
use spool_client::{
    get_block_account, get_miner_account, get_epoch_account, get_spool_account
};

use crate::store::*;
use super::queue::Tx;
use super::sync::sync_segments_from_solana;
use super::helpers;

/// Orchestrator Task B – periodic miner-challenge sync.
pub async fn run(
    rpc: Arc<RpcClient>,
    store: Arc<SpoolStore>,
    miner_address: Pubkey,
    _trusted_peer: Option<String>,
    tx: Tx,
) -> anyhow::Result<()> {
    loop {
        // Fetch miner, block, and epoch accounts
        let block_with_miner = tokio::join!(
            get_block_account(&rpc),
            get_miner_account(&rpc, &miner_address),
            get_epoch_account(&rpc)
        );

        let (block, miner, _epoch) = (
            block_with_miner.0?.0,
            block_with_miner.1?.0,
            block_with_miner.2?.0,
        );

        let miner_challenge = compute_challenge(&block.challenge, &miner.challenge);
        let spool_number = compute_recall_spool(&miner_challenge, block.challenge_set);

        log::debug!("Miner needs spool number: {}", spool_number);

        // Get spool address (assumed to be synced during initialization)
        if let Ok(spool_address) = store.get_spool_address(spool_number) {
            let spool = get_spool_account(&rpc, &spool_address).await?.0;

            if helpers::sync_needed(&store, &spool_address, spool.total_segments)? {
                log::debug!(
                    "Syncing segments for spool: {}",
                    spool_address
                );

                //if let Some(peer_url) = &trusted_peer {
                //    sync_segments_from_trusted_peer(&store, &spool_address, peer_url, &tx).await?;
                //} else {
                //    sync_segments_from_solana(&store, &rpc, &spool_address, &tx).await?;
                //}

                // TODO: For now, always sync from Solana, as trusted peer logic is not implemented
                // yet. Need to implement a way to fetch entire sectors from a trusted peer.

                sync_segments_from_solana(&store, &rpc, &spool_address, &tx).await?;
            }

        } else {
            log::error!("Spool address not found for spool number {}", spool_number);
        }

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}
