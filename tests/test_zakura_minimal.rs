//! Exercises the RocksDB features Zakura uses, so a build without the `full`
//! feature links and runs a binary that keeps them all live.
//!
//! The other test binaries call the C API of features that build skips, so
//! they only link with `full`. This one links either way:
//!
//! ```text
//! cargo test --release --no-default-features --features lz4,bindgen-runtime --test test_zakura_minimal
//! ```

mod util;

use rocksdb::{
    perf::{set_perf_stats, PerfContext, PerfMetric, PerfStatsLevel},
    BlockBasedOptions, ColumnFamilyDescriptor, DBCompressionType, DBWithThreadMode, Direction,
    IteratorMode, MergeOperands, MultiThreaded, Options, ReadOptions, WriteBatch, DB,
};
use util::DBPath;

type MultiThreadedDB = DBWithThreadMode<MultiThreaded>;

const PLAIN: &str = "plain";
const BALANCES: &str = "balances";

fn add_balances(_key: &[u8], existing: Option<&[u8]>, operands: &MergeOperands) -> Option<Vec<u8>> {
    let mut total = existing.map_or(0, decode);
    for operand in operands {
        total += decode(operand);
    }
    Some(total.to_le_bytes().to_vec())
}

fn decode(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("balances are 8 bytes"))
}

/// Options like Zakura's: block-based tables with Ribbon filters, compression,
/// and an associative merge operator on one column family.
fn options() -> (Options, Vec<ColumnFamilyDescriptor>) {
    let mut table = BlockBasedOptions::default();
    table.set_ribbon_filter(9.9);

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_block_based_table_factory(&table);
    opts.set_max_open_files(-1);
    opts.set_max_total_wal_size(1 << 20);
    #[cfg(feature = "lz4")]
    opts.set_compression_type(DBCompressionType::Lz4);
    #[cfg(not(feature = "lz4"))]
    opts.set_compression_type(DBCompressionType::None);

    let mut balances = opts.clone();
    balances.set_merge_operator_associative("add_balances", add_balances);
    let column_families = vec![
        ColumnFamilyDescriptor::new(PLAIN, opts.clone()),
        ColumnFamilyDescriptor::new(BALANCES, balances),
    ];
    (opts, column_families)
}

#[test]
fn zakura_workload() {
    let path = DBPath::new("_rust_rocksdb_zakura_minimal");
    let (opts, column_families) = options();
    let db = MultiThreadedDB::open_cf_descriptors(&opts, &path, column_families).unwrap();
    let plain = db.cf_handle(PLAIN).unwrap();
    let balances = db.cf_handle(BALANCES).unwrap();

    // Several flushed files per column family, so compaction has work to do.
    for round in 0u64..4 {
        let mut batch = WriteBatch::default();
        for i in 0u64..500 {
            let key = (round * 1000 + i).to_be_bytes();
            batch.put_cf(&plain, key, [round as u8; 64]);
            batch.merge_cf(&balances, (i % 50).to_be_bytes(), 1u64.to_le_bytes());
            if i % 7 == 0 {
                batch.delete_cf(&plain, ((round.saturating_sub(1)) * 1000 + i).to_be_bytes());
            }
        }
        db.write(batch).unwrap();
        db.flush_cf(&plain).unwrap();
        db.flush_cf(&balances).unwrap();
    }
    db.flush_wal(true).unwrap();
    db.flush().unwrap();

    // Compact while a snapshot is held, so compaction checks which versions
    // the snapshot can still see.
    let snapshot = db.snapshot();
    db.put_cf(&plain, 8u64.to_be_bytes(), b"after snapshot")
        .unwrap();
    db.flush_cf(&plain).unwrap();
    db.compact_range_cf(&plain, None::<&[u8]>, None::<&[u8]>);
    db.compact_range_cf(&balances, None::<&[u8]>, None::<&[u8]>);
    assert_eq!(
        snapshot.get_cf(&plain, 8u64.to_be_bytes()).unwrap(),
        Some(vec![0; 64])
    );
    drop(snapshot);
    assert_eq!(
        db.get_cf(&plain, 8u64.to_be_bytes()).unwrap().as_deref(),
        Some(&b"after snapshot"[..])
    );

    // Each of the 50 balance keys got one merge operand per round and per
    // 50 writes.
    assert_eq!(
        db.get_cf(&balances, 3u64.to_be_bytes())
            .unwrap()
            .map(|value| decode(&value)),
        Some(40)
    );

    // Bounded forward and reverse iteration.
    let mut read_opts = ReadOptions::default();
    read_opts.set_iterate_lower_bound(1000u64.to_be_bytes());
    read_opts.set_iterate_upper_bound(2000u64.to_be_bytes());
    let forward: Vec<_> = db
        .iterator_cf_opt(&plain, read_opts, IteratorMode::Start)
        .map(Result::unwrap)
        .collect();
    assert!(!forward.is_empty());
    assert!(forward.windows(2).all(|pair| pair[0].0 < pair[1].0));
    let reverse = db
        .iterator_cf(
            &plain,
            IteratorMode::From(&3500u64.to_be_bytes(), Direction::Reverse),
        )
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(*reverse.0, 3499u64.to_be_bytes());
    let last = db
        .iterator_cf(&plain, IteratorMode::End)
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(*last.0, 3499u64.to_be_bytes());

    // Properties and perf counters.
    assert!(db
        .property_int_value_cf(&plain, "rocksdb.estimate-num-keys")
        .unwrap()
        .is_some());
    assert!(db
        .property_int_value("rocksdb.num-running-compactions")
        .unwrap()
        .is_some());
    set_perf_stats(PerfStatsLevel::EnableCount);
    let mut perf = PerfContext::default();
    perf.reset();
    db.iterator_cf(&plain, IteratorMode::Start).count();
    let _ = perf.metric(PerfMetric::InternalDeleteSkippedCount);
    set_perf_stats(PerfStatsLevel::Disable);

    // A read-only secondary instance that follows the primary.
    let secondary_path = DBPath::new("_rust_rocksdb_zakura_minimal_secondary");
    let (secondary_opts, secondary_column_families) = options();
    let secondary = MultiThreadedDB::open_cf_descriptors_as_secondary(
        &secondary_opts,
        &path,
        &secondary_path,
        secondary_column_families,
    )
    .unwrap();
    db.put_cf(&plain, 9999u64.to_be_bytes(), b"new").unwrap();
    db.flush_cf(&plain).unwrap();
    secondary.try_catch_up_with_primary().unwrap();
    let secondary_plain = secondary.cf_handle(PLAIN).unwrap();
    assert_eq!(
        secondary
            .get_cf(&secondary_plain, 9999u64.to_be_bytes())
            .unwrap()
            .as_deref(),
        Some(&b"new"[..])
    );
    drop(secondary_plain);
    drop(secondary);

    db.cancel_all_background_work(true);
    drop(plain);
    drop(balances);
    drop(db);

    assert_eq!(DB::list_cf(&opts, &path).unwrap().len(), 3);
}
