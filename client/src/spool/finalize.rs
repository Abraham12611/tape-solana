use std::sync::Arc;

use anyhow::Result;
use solana_sdk::{
    signature::{Keypair, Signer},
    pubkey::Pubkey,
};
use spool_api::instruction::spool::build_finalize_ix;
use solana_client::nonblocking::rpc_client::RpcClient;
use crate::utils::*;

/// Finalizes the spool with the last segment's signature.
pub async fn finalize_spool(
    client: &Arc<RpcClient>,
    signer: &Keypair,
    spool_address: Pubkey,
    writer_address: Pubkey,
) -> Result<()> {

    let finalize_ix = build_finalize_ix(
        signer.pubkey(),
        spool_address,
        writer_address,
    );

    build_send_and_confirm_tx(
        &[finalize_ix],
        client,
        signer.pubkey(),
        &[signer]
    ).await?;

    Ok(())
}

