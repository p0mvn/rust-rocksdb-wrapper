//! Source edits that remove RocksDB features Zakura does not use.
//!
//! Without the `full` feature, each listed RocksDB source file is copied into
//! `OUT_DIR`, edited, and compiled in place of the original. Every edit makes
//! an entry point of an unused feature return early, so the compiler drops the
//! call into that feature and the linker drops the feature's code. The
//! `rocksdb` submodule itself stays unmodified.
//!
//! Each anchor must occur exactly once in its file. When a RocksDB upgrade
//! changes the code around an anchor, the build fails here instead of
//! silently compiling the unedited file.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

struct Edit {
    file: &'static str,
    anchor: &'static str,
    replacement: &'static str,
}

const EDITS: &[Edit] = &[
    // SST file ingestion (`DB::IngestExternalFile*`).
    Edit {
        file: "db/db_impl/db_impl.cc",
        anchor: "Status DBImpl::IngestExternalFiles(\n    const std::vector<IngestExternalFileArg>& args) {\n",
        replacement: "Status DBImpl::IngestExternalFiles(\n    const std::vector<IngestExternalFileArg>& args) {\n  return Status::NotSupported(\"SST file ingestion is not built in\");\n",
    },
    // Column family import (`DB::CreateColumnFamilyWithImport`).
    Edit {
        file: "db/db_impl/db_impl.cc",
        anchor: "    const std::vector<const ExportImportFilesMetaData*>& metadatas,\n    ColumnFamilyHandle** handle) {\n",
        replacement: "    const std::vector<const ExportImportFilesMetaData*>& metadatas,\n    ColumnFamilyHandle** handle) {\n  return Status::NotSupported(\"column family import is not built in\");\n",
    },
    // Query and block cache tracing, and trace replay.
    Edit {
        file: "db/db_impl/db_impl.cc",
        anchor: "Status DBImpl::StartTrace(const TraceOptions& trace_options,\n                          std::unique_ptr<TraceWriter>&& trace_writer) {\n",
        replacement: "Status DBImpl::StartTrace(const TraceOptions& trace_options,\n                          std::unique_ptr<TraceWriter>&& trace_writer) {\n  return Status::NotSupported(\"tracing is not built in\");\n",
    },
    Edit {
        file: "db/db_impl/db_impl.cc",
        anchor: "Status DBImpl::StartBlockCacheTrace(\n    const TraceOptions& trace_options,\n    std::unique_ptr<TraceWriter>&& trace_writer) {\n",
        replacement: "Status DBImpl::StartBlockCacheTrace(\n    const TraceOptions& trace_options,\n    std::unique_ptr<TraceWriter>&& trace_writer) {\n  return Status::NotSupported(\"tracing is not built in\");\n",
    },
    Edit {
        file: "db/db_impl/db_impl.cc",
        anchor: "Status DBImpl::StartBlockCacheTrace(\n    const BlockCacheTraceOptions& trace_options,\n    std::unique_ptr<BlockCacheTraceWriter>&& trace_writer) {\n",
        replacement: "Status DBImpl::StartBlockCacheTrace(\n    const BlockCacheTraceOptions& trace_options,\n    std::unique_ptr<BlockCacheTraceWriter>&& trace_writer) {\n  return Status::NotSupported(\"tracing is not built in\");\n",
    },
    Edit {
        file: "db/db_impl/db_impl.cc",
        anchor: "Status DBImpl::StartIOTrace(const TraceOptions& trace_options,\n                            std::unique_ptr<TraceWriter>&& trace_writer) {\n",
        replacement: "Status DBImpl::StartIOTrace(const TraceOptions& trace_options,\n                            std::unique_ptr<TraceWriter>&& trace_writer) {\n  return Status::NotSupported(\"tracing is not built in\");\n",
    },
    Edit {
        file: "db/db_impl/db_impl.cc",
        anchor: "    std::unique_ptr<Replayer>* replayer) {\n  replayer->reset(new ReplayerImpl(this, handles, std::move(reader)));\n",
        replacement: "    std::unique_ptr<Replayer>* replayer) {\n  return Status::NotSupported(\"trace replay is not built in\");\n  replayer->reset(new ReplayerImpl(this, handles, std::move(reader)));\n",
    },
    // Remote compaction (`CompactionService`, `DB::OpenAndCompact`).
    Edit {
        file: "db/db_impl/db_impl_secondary.cc",
        anchor: "    std::string* output,\n    const CompactionServiceOptionsOverride& override_options) {\n",
        replacement: "    std::string* output,\n    const CompactionServiceOptionsOverride& override_options) {\n  return Status::NotSupported(\"remote compaction is not built in\");\n",
    },
    Edit {
        file: "db/compaction/compaction_job.cc",
        anchor: "  if (db_options_.compaction_service) {\n    CompactionServiceJobStatus comp_status =\n",
        replacement: "  if (false) {\n    CompactionServiceJobStatus comp_status =\n",
    },
    // Encrypted file systems.
    Edit {
        file: "env/file_system.cc",
        anchor: "        Status s = NewEncryptedFileSystemImpl(nullptr, nullptr, guard);\n",
        replacement: "        Status s = Status::NotSupported(\"encryption is not built in\");\n",
    },
    // Plain and cuckoo table formats.
    Edit {
        file: "table/table_factory.cc",
        anchor: "          guard->reset(new PlainTableFactory());\n",
        replacement: "          guard->reset();\n",
    },
    Edit {
        file: "table/table_factory.cc",
        anchor: "          guard->reset(new CuckooTableFactory());\n",
        replacement: "          guard->reset();\n",
    },
    Edit {
        file: "options/cf_options.cc",
        anchor: "      new_factory.reset(NewPlainTableFactory());\n",
        replacement: "      return Status::NotSupported(\"plain tables are not built in\");\n",
    },
];

/// Writes the edited copies under `out_dir` and returns, for each edited
/// source path (relative to the `rocksdb` directory), the path to compile.
pub fn write_edited_sources(rocksdb_dir: &Path, out_dir: &Path) -> HashMap<&'static str, PathBuf> {
    let mut by_file: HashMap<&'static str, Vec<&Edit>> = HashMap::new();
    for edit in EDITS {
        by_file.entry(edit.file).or_default().push(edit);
    }
    let mut edited = HashMap::new();
    for (file, edits) in by_file {
        let original = rocksdb_dir.join(file);
        println!("cargo:rerun-if-changed={}", original.display());
        let mut source = fs::read_to_string(&original)
            .unwrap_or_else(|e| panic!("unable to read {}: {e}", original.display()));
        for edit in edits {
            let count = source.matches(edit.anchor).count();
            assert_eq!(
                count, 1,
                "zakura_minimal: expected exactly one match in {file} for anchor {:?}, found {count}",
                edit.anchor
            );
            source = source.replacen(edit.anchor, edit.replacement, 1);
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
