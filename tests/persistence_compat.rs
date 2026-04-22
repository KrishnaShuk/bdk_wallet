//! Persistence compatibility tests for bdk_wallet.
//!
//! These tests verify that wallet data (ChangeSet) can be persisted and loaded
//! across different versions of bdk_wallet, ensuring forward and backward
//! compatibility of the persistence layer.
//!
//! - `generate_*` tests are `#[ignore]`d and only run explicitly to create fixture files for a
//!   specific release.
//! - `load_*` tests run in CI to verify that the current code can load fixture files committed from
//!   prior releases (forward compatibility).

use std::path::PathBuf;
use std::sync::Arc;

use bdk_chain::{
    keychain_txout, local_chain, tx_graph, ConfirmationBlockTime, DescriptorExt, SpkIterator,
};
use bdk_wallet::persist_test_utils::create_one_inp_one_out_tx;
use bdk_wallet::{locked_outpoints, ChangeSet, WalletPersister};
use bitcoin::{Amount, Network, OutPoint, TxOut};
use miniscript::descriptor::{Descriptor, DescriptorPublicKey};

/// The same magic bytes used by existing persistence tests in persisted_wallet.rs.
const DB_MAGIC: &[u8] = &[0x21, 0x24, 0x48];

/// The version string used to name fixture files.
/// Update this when generating fixtures for a new release.
const FIXTURE_VERSION: &str = "v3.0.0";

const DESCRIPTORS: [&str; 2] = [
    "tr([5940b9b9/86'/0'/0']tpubDDVNqmq75GNPWQ9UNKfP43UwjaHU4GYfoPavojQbfpyfZp2KetWgjGBRRAy4tYCrAA6SB11mhQAkqxjh1VtQHyKwT4oYxpwLaGHvoKmtxZf/0/*)#44aqnlam",
    "tr([5940b9b9/86'/0'/0']tpubDDVNqmq75GNPWQ9UNKfP43UwjaHU4GYfoPavojQbfpyfZp2KetWgjGBRRAy4tYCrAA6SB11mhQAkqxjh1VtQHyKwT4oYxpwLaGHvoKmtxZf/1/*)#ypcpw2dr",
];

macro_rules! hash {
    ($index:literal) => {{
        bitcoin::hashes::Hash::hash($index.as_bytes())
    }};
}

macro_rules! block_id {
    ($height:expr, $hash:literal) => {{
        bdk_chain::BlockId {
            height: $height,
            hash: bitcoin::hashes::Hash::hash($hash.as_bytes()),
        }
    }};
}

/// Returns the path to the fixtures directory.
fn fixtures_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path
}

fn spk_at_index(descriptor: &Descriptor<DescriptorPublicKey>, index: u32) -> bitcoin::ScriptBuf {
    use bitcoin::key::Secp256k1;
    descriptor
        .derived_descriptor(&Secp256k1::verification_only(), index)
        .expect("must derive")
        .script_pubkey()
}

/// Build a realistic, fully-populated ChangeSet for testing.
///
/// This mirrors the changeset construction in `persist_test_utils::persist_wallet_changeset`
/// to ensure our fixtures contain representative wallet data.
fn build_test_changeset() -> ChangeSet {
    let descriptor: Descriptor<DescriptorPublicKey> = DESCRIPTORS[0].parse().unwrap();
    let change_descriptor: Descriptor<DescriptorPublicKey> = DESCRIPTORS[1].parse().unwrap();

    let tx1 = Arc::new(create_one_inp_one_out_tx(
        hash!("We_are_all_Satoshi"),
        30_000,
    ));

    let conf_anchor = ConfirmationBlockTime {
        block_id: block_id!(910234, "B"),
        confirmation_time: 1755317160,
    };

    let outpoint = OutPoint::new(hash!("Rust"), 0);

    let tx_graph_changeset = tx_graph::ChangeSet::<ConfirmationBlockTime> {
        txs: [tx1.clone()].into(),
        txouts: [
            (
                outpoint,
                TxOut {
                    value: Amount::from_sat(1300),
                    script_pubkey: spk_at_index(&descriptor, 4),
                },
            ),
            (
                OutPoint::new(hash!("REDB"), 0),
                TxOut {
                    value: Amount::from_sat(1400),
                    script_pubkey: spk_at_index(&descriptor, 10),
                },
            ),
        ]
        .into(),
        anchors: [(conf_anchor, tx1.compute_txid())].into(),
        last_seen: [(tx1.compute_txid(), 1755317760)].into(),
        first_seen: [(tx1.compute_txid(), 1755317750)].into(),
        last_evicted: [(tx1.compute_txid(), 1755317760)].into(),
    };

    let keychain_txout_changeset = keychain_txout::ChangeSet {
        last_revealed: [
            (descriptor.descriptor_id(), 12),
            (change_descriptor.descriptor_id(), 10),
        ]
        .into(),
        spk_cache: [
            (
                descriptor.descriptor_id(),
                SpkIterator::new_with_range(&descriptor, 0..=37).collect(),
            ),
            (
                change_descriptor.descriptor_id(),
                SpkIterator::new_with_range(&change_descriptor, 0..=35).collect(),
            ),
        ]
        .into(),
    };

    let locked_outpoints_changeset = locked_outpoints::ChangeSet {
        outpoints: [(outpoint, true)].into(),
    };

    ChangeSet {
        descriptor: Some(descriptor),
        change_descriptor: Some(change_descriptor),
        network: Some(Network::Testnet),
        local_chain: local_chain::ChangeSet {
            blocks: [
                (910234, Some(hash!("B"))),
                (910233, Some(hash!("T"))),
                (910235, Some(hash!("C"))),
            ]
            .into(),
        },
        tx_graph: tx_graph_changeset,
        indexer: keychain_txout_changeset,
        locked_outpoints: locked_outpoints_changeset,
    }
}

// ── Fixture generators (run manually, marked #[ignore]) ─────────────────

#[test]
#[ignore]
fn generate_file_store_fixture() {
    let dir = fixtures_dir();
    std::fs::create_dir_all(&dir).expect("must create fixtures dir");
    let file_path = dir.join(format!("{}.db", FIXTURE_VERSION));
    let tmp_path = dir.join(format!("{}.db.tmp", FIXTURE_VERSION));

    // Write to a temp file, then rename for atomicity
    let _ = std::fs::remove_file(&tmp_path);

    let mut store =
        bdk_file_store::Store::<ChangeSet>::create(DB_MAGIC, &tmp_path).expect("must create store");

    let changeset = build_test_changeset();
    WalletPersister::persist(&mut store, &changeset).expect("must persist changeset");
    drop(store);

    std::fs::rename(&tmp_path, &file_path).expect("must rename fixture");

    println!("Generated file_store fixture at: {}", file_path.display());
}

#[test]
#[ignore]
fn generate_sqlite_fixture() {
    let dir = fixtures_dir();
    std::fs::create_dir_all(&dir).expect("must create fixtures dir");
    let file_path = dir.join(format!("{}.sqlite", FIXTURE_VERSION));
    let tmp_path = dir.join(format!("{}.sqlite.tmp", FIXTURE_VERSION));

    // Write to a temp file, then rename for atomicity
    let _ = std::fs::remove_file(&tmp_path);

    let mut conn =
        bdk_chain::rusqlite::Connection::open(&tmp_path).expect("must open sqlite connection");

    // Initialize the schema (creates tables)
    let _empty = WalletPersister::initialize(&mut conn).expect("must initialize sqlite schema");

    let changeset = build_test_changeset();
    WalletPersister::persist(&mut conn, &changeset).expect("must persist changeset");
    drop(conn);

    std::fs::rename(&tmp_path, &file_path).expect("must rename fixture");

    println!("Generated sqlite fixture at: {}", file_path.display());
}

// ── Fixture loaders (run in CI, NOT #[ignore]) ──────────────────────────

#[test]
fn load_file_store_fixture() {
    let file_path = fixtures_dir().join(format!("{}.db", FIXTURE_VERSION));
    assert!(
        file_path.exists(),
        "Fixture file not found at {}. Run `cargo test -p bdk_wallet generate_file_store_fixture -- --ignored` to generate it.",
        file_path.display()
    );

    let (mut store, loaded_changeset) =
        bdk_file_store::Store::<ChangeSet>::load(DB_MAGIC, &file_path)
            .expect("must load file_store fixture");

    // The loaded changeset should be the aggregated result from Store::load
    let changeset = loaded_changeset.unwrap_or_else(|| {
        // Fallback: try dump() if load didn't return the aggregated changeset inline
        WalletPersister::initialize(&mut store).expect("must initialize from file_store")
    });

    // Verify key fields are present
    assert!(changeset.descriptor.is_some(), "descriptor must be present");
    assert!(
        changeset.change_descriptor.is_some(),
        "change_descriptor must be present"
    );
    assert_eq!(changeset.network, Some(Network::Testnet));
    assert!(
        !changeset.local_chain.blocks.is_empty(),
        "local_chain blocks must be present"
    );
    assert!(
        !changeset.tx_graph.txs.is_empty(),
        "tx_graph txs must be present"
    );
    assert!(
        !changeset.indexer.last_revealed.is_empty(),
        "indexer last_revealed must be present"
    );
    assert!(
        !changeset.locked_outpoints.outpoints.is_empty(),
        "locked_outpoints must be present"
    );
}

#[test]
fn load_sqlite_fixture() {
    let file_path = fixtures_dir().join(format!("{}.sqlite", FIXTURE_VERSION));
    assert!(
        file_path.exists(),
        "Fixture file not found at {}. Run `cargo test -p bdk_wallet generate_sqlite_fixture -- --ignored` to generate it.",
        file_path.display()
    );

    let mut conn =
        bdk_chain::rusqlite::Connection::open(&file_path).expect("must open sqlite fixture");

    let changeset: ChangeSet =
        WalletPersister::initialize(&mut conn).expect("must initialize from sqlite fixture");

    // Verify key fields are present
    assert!(changeset.descriptor.is_some(), "descriptor must be present");
    assert!(
        changeset.change_descriptor.is_some(),
        "change_descriptor must be present"
    );
    assert_eq!(changeset.network, Some(Network::Testnet));
    assert!(
        !changeset.local_chain.blocks.is_empty(),
        "local_chain blocks must be present"
    );
    assert!(
        !changeset.tx_graph.txs.is_empty(),
        "tx_graph txs must be present"
    );
    assert!(
        !changeset.indexer.last_revealed.is_empty(),
        "indexer last_revealed must be present"
    );
    assert!(
        !changeset.locked_outpoints.outpoints.is_empty(),
        "locked_outpoints must be present"
    );
}
