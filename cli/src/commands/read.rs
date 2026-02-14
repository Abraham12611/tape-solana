use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use num_enum::TryFromPrimitive;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tokio::{task, time::Duration};

use crate::cli::{Cli, Context, Commands};
use crate::log;
use crate::utils::write_output;

use spool_client::{
    decode_spool, finalize_read, get_spool_account, init_read, process_next_block, MimeType,
    SpoolHeader,
};

pub async fn handle_read_command(cli: Cli, context: Context) -> Result<()> {
    if let Commands::Read { spool, output } = cli.command {
        let spool_address = Pubkey::from_str(&spool)
            .map_err(|_| anyhow::anyhow!("Invalid spool address: {}", spool))?;

        log::print_message("Reading spool...");
        log::print_divider();

        let pb = setup_progress_bar();

        pb.set_message("Fetching spool metadata...");
        let (spool_data, _) = get_spool_account(context.rpc(), &spool_address).await?;
        let header = SpoolHeader::try_from_bytes(&spool_data.header)?;

        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.white/gray}] {pos}/{len} {wide_msg}")
                .expect("Failed to set progress style"),
        );
        pb.set_length(spool_data.total_segments);
        pb.set_position(0);
        pb.set_message("");

        let mut state = init_read(spool_data.tail_slot);

        while process_next_block(context.rpc(), &spool_address, &mut state).await? {
            pb.set_position(state.segments_len() as u64);
        }

        let data = finalize_read(state)?;
        let result = decode_spool(data, header)?;

        let mime_type_enum =
            MimeType::try_from_primitive(header.mime_type).unwrap_or(MimeType::Unknown);

        pb.finish();
        write_output(output, &result, mime_type_enum)?;

        log::print_divider();
    }
    Ok(())
}

fn setup_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {wide_msg}")
            .expect("Failed to set progress style"),
    );

    let pb_clone = pb.clone();
    task::spawn(async move {
        while !pb_clone.is_finished() {
            pb_clone.tick();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    pb
}
