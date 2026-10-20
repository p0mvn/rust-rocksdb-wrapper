//! Source edits that remove RocksDB features Zakura does not use.
//!
//! Without the `full` feature, each listed RocksDB source file is copied into
//! `OUT_DIR`, edited, and compiled in place of the original. Every edit makes
//! an entry point of an unused feature return early, so the compiler drops the
//! call into that feature and the linker drops the feature's code. The
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
    Edit::InsertAfter {
        file: "db/db_impl/db_impl.cc",
        after: r#"
Status DBImpl::IngestExternalFiles(
    const std::vector<IngestExternalFileArg>& args) {
"#,
        insert: r#"
  return Status::NotSupported("SST file ingestion is not built in");
"#,
    },
    // Column family import (`DB::CreateColumnFamilyWithImport`).
    Edit::InsertAfter {
        file: "db/db_impl/db_impl.cc",
        after: r#"
    const std::vector<const ExportImportFilesMetaData*>& metadatas,
    ColumnFamilyHandle** handle) {
"#,
        insert: r#"
  return Status::NotSupported("column family import is not built in");
"#,
    },
    // Query, block cache, and I/O tracing, and trace replay.
    Edit::InsertAfter {
        file: "db/db_impl/db_impl.cc",
        after: r#"
Status DBImpl::StartTrace(const TraceOptions& trace_options,
                          std::unique_ptr<TraceWriter>&& trace_writer) {
"#,
        insert: r#"
  return Status::NotSupported("tracing is not built in");
"#,
    },
    Edit::InsertAfter {
        file: "db/db_impl/db_impl.cc",
        after: r#"
Status DBImpl::StartBlockCacheTrace(
    const TraceOptions& trace_options,
    std::unique_ptr<TraceWriter>&& trace_writer) {
"#,
        insert: r#"
  return Status::NotSupported("tracing is not built in");
"#,
    },
    Edit::InsertAfter {
        file: "db/db_impl/db_impl.cc",
        after: r#"
Status DBImpl::StartBlockCacheTrace(
    const BlockCacheTraceOptions& trace_options,
    std::unique_ptr<BlockCacheTraceWriter>&& trace_writer) {
"#,
        insert: r#"
  return Status::NotSupported("tracing is not built in");
"#,
    },
    Edit::InsertAfter {
        file: "db/db_impl/db_impl.cc",
        after: r#"
Status DBImpl::StartIOTrace(const TraceOptions& trace_options,
                            std::unique_ptr<TraceWriter>&& trace_writer) {
"#,
        insert: r#"
  return Status::NotSupported("tracing is not built in");
"#,
    },
    Edit::InsertAfter {
        file: "db/db_impl/db_impl.cc",
        after: r#"
    std::unique_ptr<Replayer>* replayer) {
"#,
        insert: r#"
  return Status::NotSupported("trace replay is not built in");
"#,
    },
    // Remote compaction (`CompactionService`, `DB::OpenAndCompact`).
    Edit::InsertAfter {
        file: "db/db_impl/db_impl_secondary.cc",
        after: r#"
    std::string* output,
    const CompactionServiceOptionsOverride& override_options) {
"#,
        insert: r#"
  return Status::NotSupported("remote compaction is not built in");
"#,
    },
    Edit::Replace {
        file: "db/compaction/compaction_job.cc",
        find: r#"
  if (db_options_.compaction_service) {
    CompactionServiceJobStatus comp_status =
"#,
        with: r#"
  if (false) {
    CompactionServiceJobStatus comp_status =
"#,
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
];

/// The C++ in a raw string, without the newline after its opening quote.
fn code(text: &'static str) -> &'static str {
    text.strip_prefix('\n').unwrap_or(text)
}

impl Edit {
    fn file(&self) -> &'static str {
        match self {
            Edit::InsertAfter { file, .. } | Edit::Replace { file, .. } => file,
        }
    }

    /// The anchor this edit matches, and the text that takes its place.
    fn anchor_and_replacement(&self) -> (&'static str, String) {
        match self {
            Edit::InsertAfter { after, insert, .. } => {
                (code(after), format!("{}{}", code(after), code(insert)))
            }
            Edit::Replace { find, with, .. } => (code(find), code(with).to_owned()),
        }
    }
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
            let (anchor, replacement) = edit.anchor_and_replacement();
            let count = source.matches(anchor).count();
            assert_eq!(
                count, 1,
                "zakura_minimal: expected exactly one match in {file} for anchor {anchor:?}, found {count}"
            );
            source = source.replacen(anchor, &replacement, 1);
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
