use std::sync::Arc;

use anyhow::{anyhow, Result};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    signature::{Keypair, Signature, Signer},
    pubkey::Pubkey,
};
use solana_client::nonblocking::rpc_client::RpcClient;

use spool_api::instruction::reel::build_pack_ix;
use crate::utils::*;

pub async fn pack_spool(
    client: &Arc<RpcClient>,
    signer: &Keypair,
    reel_address: Pubkey,
    spool_address: Pubkey,
    value: [u8; 32],
) -> Result<Signature> {
    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(100_000);
    let pack_ix = build_pack_ix(signer.pubkey(), reel_address, spool_address, value);

    let signature = build_send_and_confirm_tx(
        &[compute_budget_ix, pack_ix],
        client,
        signer.pubkey(),
        &[signer],
    )
    .await
    .map_err(|e| anyhow!("Failed to pack spool: {}", e))?;

    Ok(signature)
}
