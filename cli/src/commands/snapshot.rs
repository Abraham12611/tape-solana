use std::env;
use std::io::{self, Write};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use num_enum::TryFromPrimitive;
use solana_sdk::pubkey::Pubkey;

use spool_client as spoolnet;
use spool_api::SEGMENT_SIZE;

use spool_network::archive::sync::sync_from_block;
use spool_network::snapshot::{create_snapshot, load_from_snapshot};
use spool_network::store::{StoreError, StatsOps, SegmentOps};
use spoolnet::{decode_spool, MimeType, SpoolHeader};

use crate::cli::{Cli, Commands, Context, SnapshotCommands};
use crate::log;
use crate::network::get_or_create_miner;
use crate::utils::write_output;
use packx;

pub async fn handle_snapshot_commands(cli: Cli, context: Context) -> Result<()> {
    if let Commands::Snapshot(snapshot) = cli.command {
        match snapshot {
            SnapshotCommands::Stats {} => {
                handle_stats(context)?
            }
            SnapshotCommands::Resync { spool_address, miner_address } => {
                handle_resync(context, spool_address, miner_address).await?
            }
            SnapshotCommands::Create { output } => {
                handle_create(context, output)?
            }
            SnapshotCommands::Load { input } => {
                handle_load(&input)?
            }
            SnapshotCommands::GetSpool { spool_address, miner_address, output, raw } => {
                handle_get_spool(context, spool_address, miner_address, output, raw).await?
            }
            SnapshotCommands::GetSegment { spool_address, miner_address, index } => {
                handle_get_segment(context, spool_address, miner_address, index).await?
            }
        }
    }

    Ok(())
}

fn handle_stats(context: Context) -> Result<()> {
    let store = context.open_read_only_store_conn()?;
    let stats = store.get_local_stats()?;
    log::print_section_header("Local Store Stats");
    log::print_message(&format!("Number of Spools: {}", stats.spools));
    log::print_message(&format!("Size: {} bytes", stats.size_bytes));
    Ok(())
}

async fn handle_resync(
    context: Context,
    spool_address: String,
    miner_address: Option<String>,
    ) -> Result<()> {
    let spool_pubkey: Pubkey = FromStr::from_str(&spool_address)?;

    let (spool_account, _) = spoolnet::get_spool_account(context.rpc(), &spool_pubkey).await?;
    let starting_slot = spool_account.tail_slot;
    let store = Arc::new(context.open_primary_store_conn()?);

    let miner_pubkey = get_or_create_miner(
        context.rpc(), 
        context.payer(), 
        miner_address, 
        None, 
        false
    ).await?;
    log::print_message(&format!("Using miner address: {miner_pubkey}"));

    log::print_message(&format!("Re-syncing spool: {spool_address}, please wait"));

    sync_from_block(
        &store,
        context.rpc(),
        &spool_pubkey, 
        &miner_pubkey,
        starting_slot
    ).await?;

    log::print_message("Done");
    Ok(())
}

fn handle_create(context: Context, output: Option<String>) -> Result<()> {
    let snapshot_path =
        output.unwrap_or_else(|| format!("snapshot_{}.tar.gz", Utc::now().timestamp()));
    let store = context.open_read_only_store_conn()?;
    create_snapshot(&store.db, &snapshot_path)?;
    log::print_message(&format!("Snapshot created at: {snapshot_path}"));
    Ok(())
}

fn handle_load(input: &str) -> Result<()> {
    let primary_path = env::current_dir()?.join("db_spoolstore");
    load_from_snapshot(input, &primary_path)?;
    log::print_message("Snapshot loaded into primary store");
    Ok(())
}

async fn handle_get_spool(
    context: Context,
    spool_address: String,
    miner_address: Option<String>,
    output: Option<String>,
    raw: bool,
) -> Result<()> {
    let spool_pubkey: Pubkey = FromStr::from_str(&spool_address)?;
    let (spool_account, _) = spoolnet::get_spool_account(context.rpc(), &spool_pubkey).await?;

    let total_segments = spool_account.total_segments;
    let store = context.open_read_only_store_conn()?;

    let miner_pubkey = get_or_create_miner(
        context.rpc(), 
        context.payer(), 
        miner_address, 
        None, 
        false
    ).await?;
    let miner_bytes = miner_pubkey.to_bytes();

    let mut data: Vec<u8> = Vec::with_capacity((total_segments as usize) * SEGMENT_SIZE);
    let mut missing: Vec<u64> = Vec::new();
    for seg_idx in 0..total_segments {
        match store.get_segment(&spool_pubkey, seg_idx) {
            Ok(segment_data) => {
                let solution = packx::Solution::from_bytes(&segment_data.try_into().unwrap());
                let segment = solution.unpack(&miner_bytes);
                data.extend_from_slice(&segment);
            }
            Err(StoreError::SegmentNotFoundForAddress(..)) => {
                data.extend_from_slice(&[0u8; SEGMENT_SIZE]);
                missing.push(seg_idx);
            }
            Err(e) => return Err(e.into()),
        }
    }

    if !missing.is_empty() {
        log::print_message(&format!("Missing segments: {missing:?}"));
    }

    let mime_type = if raw {
        MimeType::Unknown
    } else {
        let header = SpoolHeader::try_from_bytes(&spool_account.header)?;
        MimeType::try_from_primitive(header.mime_type).unwrap_or(MimeType::Unknown)
    };

    let data_to_write = if raw {
        data
    } else {
        let header = SpoolHeader::try_from_bytes(&spool_account.header)?;
        decode_spool(data, header)?
    };

    write_output(output, &data_to_write, mime_type)?;

    Ok(())
}

async fn handle_get_segment(
    context: Context,
    spool_address: String,
    miner_address: Option<String>,
    index: u32
) -> Result<()> {

    let spool_pubkey: Pubkey = FromStr::from_str(&spool_address)?;
    let (spool_account, _) = spoolnet::get_spool_account(context.rpc(), &spool_pubkey).await?;
    if (index as u64) >= spool_account.total_segments {
        anyhow::bail!(
            "Invalid segment index: {} (spool has {} segments)",
            index,
            spool_account.total_segments
        );
    }

    let store = context.open_read_only_store_conn()?;

    let miner_pubkey = get_or_create_miner(
        context.rpc(), 
        context.payer(), 
        miner_address, 
        None, 
        false
    ).await?;
    let miner_bytes = miner_pubkey.to_bytes();

    match store.get_segment(&spool_pubkey, index as u64) {
        Ok(segment_data) => {
            let solution = packx::Solution::from_bytes(&segment_data.try_into().unwrap());
            let segment = solution.unpack(&miner_bytes);

            let mut stdout = io::stdout();
            stdout.write_all(&segment)?;
            stdout.flush()?;
        }
        Err(StoreError::SegmentNotFoundForAddress(..)) => {
            log::print_message("Segment not found in local store");
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
