// Copyright (c) 2018-2021 The MobileCoin Foundation

//! Configuration parameters for the MobileCoin Fog View Node

use fog_uri::FogViewUri;
use mc_attest_core::ProviderId;
use mc_common::ResponderId;
use mc_util_uri::AdminUri;
use serde::Serialize;
use std::{str::FromStr, time::Duration};
use structopt::StructOpt;

#[derive(Clone, Serialize, StructOpt)]
pub struct MobileAcctViewConfig {
    /// The ID with which to respond to client attestation requests.
    ///
    /// This ID needs to match the host:port clients use in their URI when referencing this node.
    #[structopt(long)]
    pub client_responder_id: ResponderId,

    /// PEM-formatted keypair to send with an Attestation Request.
    #[structopt(long)]
    pub ias_api_key: String,

    /// The IAS SPID to use when getting a quote
    #[structopt(long)]
    pub ias_spid: ProviderId,

    /// gRPC listening URI for client requests.
    #[structopt(long)]
    pub client_listen_uri: FogViewUri,

    /// Optional admin listening URI.
    #[structopt(long)]
    pub admin_listen_uri: Option<AdminUri>,

    /// Enables authenticating client requests using Authorization tokens using the provided
    /// hex-encoded 32 bytes shared secret.
    #[structopt(long, parse(try_from_str=hex::FromHex::from_hex))]
    pub client_auth_token_secret: Option<[u8; 32]>,

    /// Maximal client authentication token lifetime, in seconds (only relevant when
    /// --client-auth-token-secret is used. Defaults to 86400 - 24 hours).
    #[structopt(long, default_value = "86400", parse(try_from_str=parse_duration_in_seconds))]
    pub client_auth_token_max_lifetime: Duration,
}

/// Converts a string containing number of seconds to a Duration object.
fn parse_duration_in_seconds(src: &str) -> Result<Duration, std::num::ParseIntError> {
    Ok(Duration::from_secs(u64::from_str(src)?))
}
