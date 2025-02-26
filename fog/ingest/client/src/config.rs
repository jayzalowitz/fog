// Copyright (c) 2018-2021 The MobileCoin Foundation

//! Configuration parameters for the Fog ingest client

use std::{str::FromStr, time::Duration};
use structopt::StructOpt;

#[derive(Clone, StructOpt)]
pub struct IngestConfig {
    /// URI for ingest server
    #[structopt(long)]
    pub uri: String,

    /// How long to retry if unavailable, this is useful for tests
    #[structopt(long, short = "r", default_value = "10", parse(try_from_str=parse_duration_in_seconds))]
    pub retry_seconds: Duration,

    #[structopt(subcommand)]
    pub cmd: IngestConfigCommand,
}

fn parse_duration_in_seconds(src: &str) -> Result<Duration, std::num::ParseIntError> {
    Ok(Duration::from_secs(u64::from_str(src)?))
}

#[derive(Clone, StructOpt)]
pub enum IngestConfigCommand {
    /// Get a summary of the state of the ingest server.
    GetStatus,

    /// Wipe out all keys and oram state in the enclave, replacing them with new random keys.
    NewKeys,

    /// Set the list of peers of this ingest server.
    SetPeers { peer_uris: Vec<String> },

    /// Set the pubkey_expiry_window of the ingest server.
    SetPubkeyExpiryWindow {
        /// This value is a number of blocks that is added to the current block index to compute the "pubkey_expiry" value of fog reports.
        pubkey_expiry_window: u64,
    },

    /// Attempt to put an idle server in the active mode.
    Activate,

    /// Attempt to put an active server in the retiring mode, after which it will eventually become idle.
    Retire,

    /// Attempt to take a retired server out of retirement.
    Unretire,

    /// Report a range of missed blocks [start, end).
    ReportMissedBlockRange {
        /// The block index of the first missed block.
        #[structopt(long)]
        start: u64,

        /// The block index of the last missed block + 1.
        #[structopt(long)]
        end: u64,
    },

    /// Gets the list of reported missed block ranges.
    GetMissedBlockRanges,
}
