// Copyright (c) 2018-2021 The MobileCoin Foundation

// This integration-level test checks that a backup ingest node will
// use the private key from its primary.

use fog_api::ingest_grpc;
use fog_ingest_server::{
    error::IngestServiceError,
    server::{IngestServer, IngestServerConfig},
};
use fog_recovery_db_iface::{RecoveryDb, ReportDb};
use fog_test_infra::get_enclave_path;
use fog_uri::{FogIngestUri, IngestPeerUri};
use grpcio::ChannelBuilder;
use mc_attest_net::{Client as AttestClient, RaClient};
use mc_common::{
    logger::{log, test_with_logger, Logger},
    ResponderId,
};
use mc_ledger_db::LedgerDB;
use mc_watcher::watcher_db::WatcherDB;
use std::{str::FromStr, sync::Arc, time::Duration};
use tempdir::TempDir;

const NUM_PHASES: usize = 3; // phase = until account server goes down
const OMAP_CAPACITY: u64 = 256;

// Test that ingest server key backup is working, in the sense that if we
// start an ingest server, then run another in backup mode pointing at the
// first, the backup and primary pubkeys will be identical.
fn test_ingest_pool_integration<A, DB>(db: DB, base_port: u16, logger: Logger)
where
    A: RaClient + 'static,
    DB: RecoveryDb + ReportDb + Clone + Send + Sync + 'static,
    IngestServiceError: From<<DB as RecoveryDb>::Error>,
{
    for phase_count in 0..NUM_PHASES {
        {
            log::info!(logger, "Phase {}/{}", phase_count + 1, NUM_PHASES);

            // First make grpcio env
            // Note: DONT move this outside the phase_count loop or you will have a bad time
            // It needs to be destroyed when the ingest server is destroyed.
            let grpcio_env = Arc::new(grpcio::EnvBuilder::new().build());

            // In each phase we tear down ingest
            let (_primary_ingest_server, primary_node_id) = {
                let local_node_id =
                    ResponderId::from_str(&format!("127.0.0.1:{}", base_port + 5)).unwrap();

                let config = IngestServerConfig {
                    ias_spid: Default::default(),
                    local_node_id: local_node_id.clone(),
                    client_listen_uri: FogIngestUri::from_str(&format!(
                        "insecure-fog-ingest://0.0.0.0:{}/",
                        base_port + 4
                    ))
                    .unwrap(),
                    peer_listen_uri: IngestPeerUri::from_str(&format!(
                        "insecure-igp://0.0.0.0:{}/",
                        base_port + 5
                    ))
                    .unwrap(),
                    peers: Default::default(),
                    fog_report_id: Default::default(),
                    max_transactions: 10_000,
                    pubkey_expiry_window: 100,
                    peer_checkup_period: None,
                    watcher_timeout: Duration::default(),
                    state_file: None,
                    enclave_path: get_enclave_path(fog_ingest_enclave::ENCLAVE_FILE),
                    omap_capacity: OMAP_CAPACITY,
                };

                // Set up the Watcher DB - create a new watcher DB for each phase
                let db_tmp =
                    TempDir::new("wallet_db").expect("Could not make tempdir for wallet db");
                WatcherDB::create(db_tmp.path().to_path_buf()).unwrap();
                let watcher =
                    WatcherDB::open_ro(db_tmp.path().to_path_buf(), logger.clone()).unwrap();

                // Set up an empty ledger db.
                let ledger_db_path =
                    TempDir::new("ledger_db").expect("Could not make tempdir for ledger db");
                LedgerDB::create(ledger_db_path.path().to_path_buf()).unwrap();
                let ledger_db = LedgerDB::open(ledger_db_path.path().to_path_buf()).unwrap();

                let ra_client = A::new("").expect("Could not create IAS client");
                let mut node = IngestServer::new(
                    config,
                    ra_client,
                    db.clone(),
                    watcher,
                    ledger_db,
                    logger.clone(),
                );
                node.start().expect("Could not start Ingest Service");
                node.activate().expect("Could not activate Ingest");

                (node, local_node_id)
            };

            std::thread::sleep(std::time::Duration::from_millis(1000));

            let _backup_ingest_server = {
                let local_node_id =
                    ResponderId::from_str(&format!("127.0.0.1:{}", base_port + 9)).unwrap();

                let config = IngestServerConfig {
                    ias_spid: Default::default(),
                    local_node_id,
                    client_listen_uri: FogIngestUri::from_str(&format!(
                        "insecure-fog-ingest://0.0.0.0:{}/",
                        base_port + 8
                    ))
                    .unwrap(),
                    peer_listen_uri: IngestPeerUri::from_str(&format!(
                        "insecure-igp://0.0.0.0:{}/",
                        base_port + 9
                    ))
                    .unwrap(),
                    peers: Default::default(),
                    fog_report_id: Default::default(),
                    max_transactions: 10_000,
                    pubkey_expiry_window: 100,
                    peer_checkup_period: None,
                    watcher_timeout: Duration::default(),
                    state_file: None,
                    enclave_path: get_enclave_path(fog_ingest_enclave::ENCLAVE_FILE),
                    omap_capacity: OMAP_CAPACITY,
                };

                let db_tmp =
                    TempDir::new("wallet_db").expect("Could not make tempdir for wallet db");
                WatcherDB::create(db_tmp.path().to_path_buf()).unwrap();
                let watcher =
                    WatcherDB::open_ro(db_tmp.path().to_path_buf(), logger.clone()).unwrap();

                // Set up an empty ledger db.
                let ledger_db_path =
                    TempDir::new("ledger_db").expect("Could not make tempdir for ledger db");
                LedgerDB::create(ledger_db_path.path().to_path_buf()).unwrap();
                let ledger_db = LedgerDB::open(ledger_db_path.path().to_path_buf()).unwrap();

                let ra_client = A::new("").expect("Could not create IAS client");
                let mut node = IngestServer::new(
                    config,
                    ra_client,
                    db.clone(),
                    watcher,
                    ledger_db,
                    logger.clone(),
                );

                // Sync key from primary.
                let primary_node_uri =
                    IngestPeerUri::from_str(&format!("insecure-igp://{}", &primary_node_id))
                        .expect("faled parsing uri");

                let _summary = node
                    .sync_keys_from_remote(&primary_node_uri)
                    .expect("failed syncing key from primary");

                node.start().expect("Could not start Ingest Service");
                node
            };

            // We are submitting the blocks to ingest over the grpc api
            let primary_ingest_client = {
                let ch = ChannelBuilder::new(grpcio_env.clone())
                    .connect(&format!("127.0.0.1:{}", base_port + 4));
                ingest_grpc::AccountIngestApiClient::new(ch)
            };
            let backup_ingest_client = {
                let ch = ChannelBuilder::new(grpcio_env.clone())
                    .connect(&format!("127.0.0.1:{}", base_port + 8));
                ingest_grpc::AccountIngestApiClient::new(ch)
            };

            // get the pubkey from the primary, then poll the backup and see
            // it it gets the same pubkey

            let primary_pubkey = primary_ingest_client
                .get_status(&Default::default())
                .unwrap()
                .take_ingress_pubkey();
            let mut backup_pubkey = backup_ingest_client
                .get_status(&Default::default())
                .unwrap()
                .take_ingress_pubkey();

            for _ in 0..30 {
                if primary_pubkey == backup_pubkey {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(1000));

                backup_pubkey = backup_ingest_client
                    .get_status(&Default::default())
                    .unwrap()
                    .take_ingress_pubkey();
            }

            assert_eq!(primary_pubkey, backup_pubkey);
        }

        // grpcio detaches all its threads and does not join them :(
        // we opened a PR here: https://github.com/tikv/grpc-rs/pull/455
        // in the meantime we can just sleep after grpcio env and all related
        // objects have been destroyed, and hope that those 6 threads see the
        // shutdown requests within 1 second.
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}

/// Run the ingest validation test using sql recovery db
#[test_with_logger]
fn test_ingest_pool_sql(logger: Logger) {
    use fog_sql_recovery_db::test_utils::SqlRecoveryDbTestContext;

    let db_test_context = SqlRecoveryDbTestContext::new(logger.clone());

    let mut trial_count = 0;
    mc_util_test_helper::run_with_several_seeds(|_rng| {
        trial_count += 1;
        log::info!(logger, "Trial {}", trial_count);

        let db = db_test_context.get_db_instance();
        test_ingest_pool_integration::<AttestClient, _>(db, 3220, logger.clone())
    })
}
