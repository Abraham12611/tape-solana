
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use crate::cli::{Cli, Commands, Context, InfoCommands};
use crate::log;
use spool_client as spoolnet;
use spool_api::utils::from_name;
use spool_client::SpoolHeader;

use super::network::get_or_create_miner;

pub async fn handle_info_commands(cli: Cli, context: Context) -> Result<()> {
    if let Commands::Info(info) = cli.command {
        match info {
            InfoCommands::Archive {} => {
                let (archive, _address) = spoolnet::get_archive_account(context.rpc()).await?;
                log::print_section_header("Archive Account");
                log::print_message(&format!("Spools: {}", archive.spools_stored));
                log::print_message(&format!("Segments: {}", archive.segments_stored));
                log::print_message(&format!("Bytes: {}", archive.segments_stored as usize * spool_api::SEGMENT_SIZE));
            }
            InfoCommands::Epoch {} => {
                let (epoch, _address) = spoolnet::get_epoch_account(context.rpc()).await?;
                log::print_section_header("Epoch Account");
                log::print_message(&format!("Current Epoch: {}", epoch.number));
                log::print_message(&format!("Progress: {}", epoch.progress));
                log::print_message(&format!("Mining Difficulty: {}", epoch.mining_difficulty));
                log::print_message(&format!("Packing Difficulty: {}", epoch.packing_difficulty));
                log::print_message(&format!("Target Participation: {}", epoch.target_participation));
                log::print_message(&format!("Reward Rate: {}", epoch.reward_rate));
                log::print_message(&format!("Duplicates: {}", epoch.duplicates));
                log::print_message(&format!("Last Epoch At: {}", epoch.last_epoch_at));
            }
            InfoCommands::Block {} => {
                let (block, _address) = spoolnet::get_block_account(context.rpc()).await?;
                log::print_section_header("Block Account");
                log::print_message(&format!("Current Block: {}", block.number));
                log::print_message(&format!("Progress: {}", block.progress));
                log::print_message(&format!("Challenge: {:?}", block.challenge));
                log::print_message(&format!("Challenge Set: {}", block.challenge_set));
                log::print_message(&format!("Last Proof At: {}", block.last_proof_at));
                log::print_message(&format!("Last Block At: {}", block.last_block_at));
            }
            InfoCommands::FindSpool { number } => {
                let res = spoolnet::find_spool_account(context.rpc(), number).await?;
                match res {
                    Some((spool_address, _spool_account)) => {
                        log::print_section_header("Spool Address");
                        log::print_message(&format!("Spool Number: {number}"));
                        log::print_message(&format!("Address: {spool_address}"));
                        log::print_divider();
                    }
                    None => {
                        log::print_error("Spool not found");
                        return Ok(());
                    }
                }
            }
            InfoCommands::Spool { pubkey } => {
                let spool_address: Pubkey = pubkey.parse()?;
                let (spool, _) = spoolnet::get_spool_account(context.rpc(), &spool_address).await?;

                log::print_section_header("Spool Account");
                log::print_message(&format!("Id: {}", spool.number));
                log::print_message(&format!("Name: {}", from_name(&spool.name)));
                log::print_message(&format!("Address: {spool_address}"));
                log::print_message(&format!("Authority: {}", spool.authority));
                log::print_message(&format!("Merkle Root: {:?}", spool.merkle_root));
                log::print_message(&format!("First Slot: {}", spool.first_slot));
                log::print_message(&format!("Tail Slot: {}", spool.tail_slot));
                log::print_message(&format!("Balance: {}", spool.balance));
                log::print_message(&format!("Last Rent Block: {}", spool.last_rent_block));
                log::print_message(&format!("Total Segments: {}", spool.total_segments));
                log::print_message(&format!("State: {}", spool.state));

                if let Ok(header) = SpoolHeader::try_from_bytes(&spool.header) {
                    log::print_message(&format!("Header: {header:?}"));
                }

                log::print_divider();
            }

            InfoCommands::Miner { pubkey, name } => {
                let miner_address = get_or_create_miner(context.rpc(), context.payer(), pubkey, name, false).await?;
                let (miner, _) = spoolnet::get_miner_account(context.rpc(), &miner_address).await?;
                log::print_section_header("Miner Account");
                log::print_message(&format!("Name: {}", from_name(&miner.name)));
                log::print_message(&format!("Address: {miner_address}"));
                log::print_message(&format!("Owner: {}", miner.authority));
                log::print_message(&format!("Unclaimed Rewards: {}", miner.unclaimed_rewards));
                log::print_message(&format!("Challenge: {:?}", miner.challenge));
                log::print_message(&format!("Multiplier: {}", miner.multiplier));
                log::print_message(&format!("Last Proof Block: {}", miner.last_proof_block));
                log::print_message(&format!("Last Proof At: {}", miner.last_proof_at));
                log::print_message(&format!("Total Proofs: {}", miner.total_proofs));
                log::print_message(&format!("Total Rewards: {}", miner.total_rewards));
                log::print_divider();
            }
        }
    }
    Ok(())
}
