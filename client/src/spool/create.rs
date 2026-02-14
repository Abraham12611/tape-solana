use std::sync::Arc;

use anyhow::Result;
use solana_sdk::{
    signature::{Keypair, Signer, Signature},
    pubkey::Pubkey,
};
use spool_api::prelude::*;
use spool_api::instruction::spool::build_create_ix;
use solana_client::nonblocking::rpc_client::RpcClient;
use crate::utils::*;

/// Creates a new spool and returns the spool address, writer address, and initial signature.
pub async fn create_spool(
    client: &Arc<RpcClient>,
    signer: &Keypair,
    name: &str,
) -> Result<(Pubkey, Pubkey, Signature)> {

    let (spool_address, _spool_bump) = spool_pda(signer.pubkey(), &to_name(name));
    let (writer_address, _writer_bump) = writer_pda(spool_address);

    let create_ix = build_create_ix(
        signer.pubkey(), 
        name, 
    );

    let signature = build_send_and_confirm_tx(
        &[create_ix],
        client,
        signer.pubkey(),
        &[signer]
    ).await?;

    Ok((spool_address, writer_address, signature))
}

