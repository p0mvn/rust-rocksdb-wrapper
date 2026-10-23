//! Source edits that remove RocksDB features Zakura does not use.
//!
//! Without the `full` feature, each listed RocksDB source file is copied into
//! `OUT_DIR`, edited, and compiled in place of the original. Most edits replace
//! the body of an unused feature's entry point with an early return, so
//! nothing calls into that feature and the linker drops the feature's code.
//! They replace the whole body, rather than insert a return, because an
//! unoptimized MSVC build still compiles the code after a return. The
//! `rocksdb` submodule itself stays unmodified.
//!
//! The C++ in each edit is written exactly as it appears in RocksDB. Each
//! anchor must occur exactly once in its file. When a RocksDB upgrade changes
//! the code around an anchor, the build fails here instead of silently
//! compiling the unedited file.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

enum Edit {
    /// Replaces the body of the function whose code, up to and including the
    /// body's opening brace, ends with `signature`.
    ReplaceBody {
        file: &'static str,
        signature: &'static str,
        body: &'static str,
    },
    /// Inserts `insert` right after the code `after`.
    InsertAfter {
        file: &'static str,
        after: &'static str,
        insert: &'static str,
    },
    /// Replaces the code `find` with `with`.
    Replace {
        file: &'static str,
        find: &'static str,
        with: &'static str,
    },
}

const EDITS: &[Edit] = &[
    // SST file ingestion (`DB::IngestExternalFile*`).
    Edit::ReplaceBody {
        file: "db/db_impl/db_impl.cc",
        signature: r#"
Status DBImpl::IngestExternalFiles(
    const std::vector<IngestExternalFileArg>& args) {
"#,
        body: r#"
  return Status::NotSupported("SST file ingestion is not built in");
"#,
    },
    // Column family import (`DB::CreateColumnFamilyWithImport`).
    Edit::ReplaceBody {
        file: "db/db_impl/db_impl.cc",
        signature: r#"
    const std::vector<const ExportImportFilesMetaData*>& metadatas,
    ColumnFamilyHandle** handle) {
"#,
        body: r#"
  return Status::NotSupported("column family import is not built in");
"#,
    },
    // Query, block cache, and I/O tracing, and trace replay.
    Edit::ReplaceBody {
        file: "db/db_impl/db_impl.cc",
        signature: r#"
Status DBImpl::StartTrace(const TraceOptions& trace_options,
                          std::unique_ptr<TraceWriter>&& trace_writer) {
"#,
        body: r#"
  return Status::NotSupported("tracing is not built in");
"#,
    },
    Edit::ReplaceBody {
        file: "db/db_impl/db_impl.cc",
        signature: r#"
Status DBImpl::StartBlockCacheTrace(
    const TraceOptions& trace_options,
    std::unique_ptr<TraceWriter>&& trace_writer) {
"#,
        body: r#"
  return Status::NotSupported("tracing is not built in");
"#,
    },
    Edit::ReplaceBody {
        file: "db/db_impl/db_impl.cc",
        signature: r#"
Status DBImpl::StartBlockCacheTrace(
    const BlockCacheTraceOptions& trace_options,
    std::unique_ptr<BlockCacheTraceWriter>&& trace_writer) {
"#,
        body: r#"
  return Status::NotSupported("tracing is not built in");
"#,
    },
    Edit::ReplaceBody {
        file: "db/db_impl/db_impl.cc",
        signature: r#"
Status DBImpl::StartIOTrace(const TraceOptions& trace_options,
                            std::unique_ptr<TraceWriter>&& trace_writer) {
"#,
        body: r#"
  return Status::NotSupported("tracing is not built in");
"#,
    },
    Edit::ReplaceBody {
        file: "db/db_impl/db_impl.cc",
        signature: r#"
    std::unique_ptr<Replayer>* replayer) {
"#,
        body: r#"
  return Status::NotSupported("trace replay is not built in");
"#,
    },
    // Remote compaction (`CompactionService`, `DB::OpenAndCompact`).
    Edit::ReplaceBody {
        file: "db/db_impl/db_impl_secondary.cc",
        signature: r#"
    std::string* output,
    const CompactionServiceOptionsOverride& override_options) {
"#,
        body: r#"
  return Status::NotSupported("remote compaction is not built in");
"#,
    },
    Edit::Replace {
        file: "db/compaction/compaction_job.cc",
        find: r#"
  if (db_options_.compaction_service) {
    CompactionServiceJobStatus comp_status =
        ProcessKeyValueCompactionWithCompactionService(sub_compact);
    if (comp_status != CompactionServiceJobStatus::kUseLocal) {
      return;
    }
    // fallback to local compaction
    assert(comp_status == CompactionServiceJobStatus::kUseLocal);
    sub_compact->compaction_job_stats.is_remote_compaction = false;
  }
"#,
        with: "",
    },
    // Encrypted file systems.
    Edit::Replace {
        file: "env/file_system.cc",
        find: r#"
        Status s = NewEncryptedFileSystemImpl(nullptr, nullptr, guard);
"#,
        with: r#"
        Status s = Status::NotSupported("encryption is not built in");
"#,
    },
    // Plain and cuckoo table formats.
    Edit::Replace {
        file: "table/table_factory.cc",
        find: r#"
          guard->reset(new PlainTableFactory());
"#,
        with: r#"
          guard->reset();
"#,
    },
    Edit::Replace {
        file: "table/table_factory.cc",
        find: r#"
          guard->reset(new CuckooTableFactory());
"#,
        with: r#"
          guard->reset();
"#,
    },
    Edit::Replace {
        file: "options/cf_options.cc",
        find: r#"
      new_factory.reset(NewPlainTableFactory());
"#,
        with: r#"
      return Status::NotSupported("plain tables are not built in");
"#,
    },
    // The edits above leave no way to create a `PlainTableFactory`, but its
    // virtual methods are still compiled. These remove their calls into the
    // skipped plain table sources.
    Edit::Replace {
        file: "table/plain/plain_table_factory.cc",
        find: r#"
  return PlainTableReader::Open(
      table_reader_options.ioptions, table_reader_options.env_options,
      table_reader_options.internal_comparator, std::move(file), file_size,
      table, table_options_.bloom_bits_per_key, table_options_.hash_table_ratio,
      table_options_.index_sparseness, table_options_.huge_page_tlb_size,
      table_options_.full_scan_mode, table_reader_options.immortal,
      table_reader_options.prefix_extractor.get());
"#,
        with: r#"
  (void)table_reader_options;
  (void)file;
  (void)file_size;
  (void)table;
  return Status::NotSupported("plain tables are not built in");
"#,
    },
    Edit::Replace {
        file: "table/plain/plain_table_factory.cc",
        find: r#"
  return new PlainTableBuilder(
      table_builder_options.ioptions, table_builder_options.moptions,
      table_builder_options.internal_tbl_prop_coll_factories,
      table_builder_options.column_family_id,
      table_builder_options.level_at_creation, file,
      table_options_.user_key_len, table_options_.encoding_type,
      table_options_.index_sparseness, table_options_.bloom_bits_per_key,
      table_builder_options.column_family_name, 6,
      table_options_.huge_page_tlb_size, table_options_.hash_table_ratio,
      table_options_.store_index_in_file, table_builder_options.db_id,
      table_builder_options.db_session_id, table_builder_options.cur_file_num);
"#,
        with: r#"
  (void)table_builder_options;
  (void)file;
  return nullptr;
"#,
    },
    // Write-prepared and write-unprepared transactions. Nothing creates a
    // `WritePreparedSnapshotChecker` without the skipped transaction sources,
    // but this file is still compiled for `DisableGCSnapshotChecker`. The
    // checker's call into `WritePreparedTxnDB` would otherwise leave a
    // reference to the skipped code, which compilers may keep live: GCC 16
    // speculatively devirtualizes the `CheckInSnapshot` calls in
    // `DataIsDefinitelyInSnapshot` and `DataIsDefinitelyNotInSnapshot`, which
    // compaction uses, and the link fails.
    Edit::Replace {
        file: "utilities/transactions/snapshot_checker.cc",
        find: r#"
  bool snapshot_released = false;
  // TODO(myabandeh): set min_uncommitted
  bool in_snapshot = txn_db_->IsInSnapshot(
      sequence, snapshot_sequence, kMinUnCommittedSeq, &snapshot_released);
  if (snapshot_released) {
    return SnapshotCheckerResult::kSnapshotReleased;
  }
  return in_snapshot ? SnapshotCheckerResult::kInSnapshot
                     : SnapshotCheckerResult::kNotInSnapshot;
"#,
        with: r#"
  (void)sequence;
  (void)snapshot_sequence;
  std::abort();
"#,
    },
    Edit::InsertAfter {
        file: "utilities/transactions/snapshot_checker.cc",
        after: r#"
#include "db/snapshot_checker.h"
"#,
        insert: r#"
#include <cstdlib>
"#,
    },
    // Remote compaction: `DB::OpenAndCompact` returns early above, but the
    // secondary instance's compaction still calls the skipped
    // `CompactionServiceCompactionJob`.
    Edit::ReplaceBody {
        file: "db/db_impl/db_impl_secondary.cc",
        signature: r#"
    const CompactionServiceInput& input, CompactionServiceResult* result) {
"#,
        body: r#"
  return Status::NotSupported("remote compaction is not built in");
"#,
    },
];

/// The C++ in a raw string, without the newline after its opening quote.
fn code(text: &'static str) -> &'static str {
    text.strip_prefix('\n').unwrap_or(text)
}

impl Edit {
    fn file(&self) -> &'static str {
        match self {
            Edit::ReplaceBody { file, .. }
            | Edit::InsertAfter { file, .. }
            | Edit::Replace { file, .. } => file,
        }
    }

    /// The code this edit matches.
    fn anchor(&self) -> &'static str {
        match self {
            Edit::ReplaceBody { signature, .. } => code(signature),
            Edit::InsertAfter { after, .. } => code(after),
            Edit::Replace { find, .. } => code(find),
        }
    }

    /// Applies this edit to `source`, whose only match of the anchor starts at
    /// `start`.
    fn apply(&self, source: &str, start: usize) -> String {
        let end = start + self.anchor().len();
        match self {
            Edit::ReplaceBody { body, .. } => {
                let open = source[..end]
                    .trim_end()
                    .strip_suffix('{')
                    .map(str::len)
                    .expect("a ReplaceBody signature ends with the body's opening brace");
                let close = matching_brace(source, open);
                format!("{}\n{}{}", &source[..=open], code(body), &source[close..])
            }
            Edit::InsertAfter { insert, .. } => {
                format!("{}{}{}", &source[..end], code(insert), &source[end..])
            }
            Edit::Replace { with, .. } => {
                format!("{}{}{}", &source[..start], code(with), &source[end..])
            }
        }
    }
}

/// Returns the index of the `}` that closes the `{` at `open` in C++ `source`,
/// skipping braces in comments and string and character literals.
fn matching_brace(source: &str, open: usize) -> usize {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i += source[i..].find('\n').unwrap_or(source.len() - i);
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += source[i..].find("*/").expect("unterminated comment") + 1;
            }
            // Raw strings are not supported: none of the edited bodies has one.
            b'R' if bytes.get(i + 1) == Some(&b'"') => {
                panic!("zakura_minimal: raw string literal in a replaced body")
            }
            // A quote after a digit separates digits, as in `1'000`.
            quote @ (b'"' | b'\'') if quote == b'"' || !bytes[i - 1].is_ascii_alphanumeric() => {
                i += 1;
                while bytes[i] != quote {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("zakura_minimal: unbalanced braces after byte {open}")
}

/// Writes the edited copies under `out_dir` and returns, for each edited
/// source path (relative to the `rocksdb` directory), the path to compile.
pub fn write_edited_sources(rocksdb_dir: &Path, out_dir: &Path) -> HashMap<&'static str, PathBuf> {
    let mut by_file: HashMap<&'static str, Vec<&Edit>> = HashMap::new();
    for edit in EDITS {
        by_file.entry(edit.file()).or_default().push(edit);
    }
    let mut edited = HashMap::new();
    for (file, edits) in by_file {
        let original = rocksdb_dir.join(file);
        println!("cargo:rerun-if-changed={}", original.display());
        // A Windows checkout may have CRLF line endings; the anchors use LF.
        let mut source = fs::read_to_string(&original)
            .unwrap_or_else(|e| panic!("unable to read {}: {e}", original.display()))
            .replace("\r\n", "\n");
        for edit in edits {
            let anchor = edit.anchor();
            let count = source.matches(anchor).count();
            assert_eq!(
                count, 1,
                "zakura_minimal: expected exactly one match in {file} for anchor {anchor:?}, found {count}"
            );
            let start = source.find(anchor).expect("the anchor matched once");
            source = edit.apply(&source, start);
        }
        let path = out_dir.join("zakura_minimal").join(file);
        fs::create_dir_all(path.parent().expect("edited source path has a parent"))
            .expect("unable to create edited source directory");
        if fs::read_to_string(&path).ok().as_deref() != Some(source.as_str()) {
            fs::write(&path, source).expect("unable to write edited source");
        }
        edited.insert(file, path);
    }
    edited
}
