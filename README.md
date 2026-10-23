# rust-rocksdb

> **Zakura fork.** This repository publishes `zakura-rocksdb` and
> `zakura-librocksdb-sys`, forks of [`rocksdb`](https://crates.io/crates/rocksdb)
> and [`librocksdb-sys`](https://crates.io/crates/librocksdb-sys) used by
> [Zakura](https://github.com/zakura-core/zakura). The only functional change
> is in `librocksdb-sys/build.rs`: RocksDB's C++ sources are compiled as a
> unity build, 16 sources per translation unit, so shared headers are parsed
> once per group instead of once per file. That cuts the bundled RocksDB
> compile by more than half. Set `ROCKSDB_UNITY_CHUNK=1` to compile one file
> at a time as upstream does.
>
> The library names stay `rocksdb` and `librocksdb_sys`, so depending on it as
> `rocksdb = { package = "zakura-rocksdb", ... }` needs no code changes.
> Upstream: <https://github.com/rust-rocksdb/rust-rocksdb>.

[![RocksDB build](https://github.com/rust-rocksdb/rust-rocksdb/actions/workflows/rust.yml/badge.svg?branch=master)](https://github.com/rust-rocksdb/rust-rocksdb/actions/workflows/rust.yml)
[![crates.io](https://img.shields.io/crates/v/rocksdb.svg)](https://crates.io/crates/rocksdb)
[![documentation](https://img.shields.io/docsrs/rocksdb/latest)](https://docs.rs/rocksdb)
[![license](https://img.shields.io/crates/l/rocksdb.svg)](https://github.com/rust-rocksdb/rust-rocksdb/blob/master/LICENSE)
[![Gitter chat](https://badges.gitter.im/rust-rocksdb/gitter.svg)](https://gitter.im/rust-rocksdb/lobby)
![rust 1.97 required](https://img.shields.io/badge/rust-1.97-blue.svg?label=MSRV)

![GitHub commits (since latest release)](https://img.shields.io/github/commits-since/rust-rocksdb/rust-rocksdb/latest.svg)

## Requirements

- Clang and LLVM

## Contributing

Feedback and pull requests are welcome! If a particular feature of RocksDB is
important to you, please let me know by opening an issue, and I'll
prioritize it.

## Usage

This binding is statically linked with a specific version of RocksDB. If you
want to build it yourself, make sure you've also cloned the RocksDB and
compression submodules:

```shell
git submodule update --init --recursive
```

## Compression Support

By default, support for [Snappy](https://github.com/google/snappy),
[LZ4](https://github.com/lz4/lz4), [Zstd](https://github.com/facebook/zstd),
[Zlib](https://zlib.net), and [Bzip2](http://www.bzip.org) compression
is enabled through crate features. If support for all of these compression
algorithms is not needed, default features can be disabled and specific
compression algorithms can be enabled. For example, to enable only LZ4
compression support, make these changes to your Cargo.toml:

```toml
[dependencies.rocksdb]
default-features = false
features = ["lz4"]
```

## Full RocksDB build (feature `full`)

The default feature `full` compiles all of RocksDB. Disabling default features
also disables `full`, which builds only what Zakura uses: it skips 58 of
RocksDB's 329 library sources and leaves about 570 KB less code in the final
binary. Without `full`, these features return `NotSupported` or fail to open:
SST file ingestion, column family import, remote compaction
(`CompactionService`), tracing and trace replay, encrypted file systems, plain
tables, cuckoo tables created from an options string, and TTL databases.

`librocksdb-sys/zakura_minimal.rs` makes the source edits that cut those
features off, and `librocksdb-sys/zakura_excluded_sources.txt` lists the
skipped sources. Most of the size saving comes from the edits; the linker
drops code that nothing reaches whether or not it is compiled.

No compiled code may reference a skipped source, so the archive defines every
RocksDB symbol it references. Otherwise the link depends on the linker
dropping the referencing code first: GCC 16 kept such a reference live by
speculatively devirtualizing a call, and MSVC never drops code early enough.
`.github/scripts/check_minimal_symbols.py` fails on any undefined RocksDB
symbol, and `.github/scripts/update_excluded_sources.py` stops skipping the
sources that define them, repeating until there are none. Run it after a
RocksDB upgrade or after adding sources to the list, and review the diff.

The "Minimal build toolchains" workflow links and runs
`tests/test_zakura_minimal.rs` in release and debug with many GCC and clang
versions, on Linux, macOS, and Windows, and runs the check on Linux:

```text
cargo test --release --no-default-features --features lz4,bindgen-runtime --test test_zakura_minimal
python3 .github/scripts/check_minimal_symbols.py target/*/build/zakura-librocksdb-sys-*/out/librocksdb.a
python3 .github/scripts/update_excluded_sources.py
```

## Multithreaded ColumnFamily alternation

RocksDB allows column families to be created and dropped
from multiple threads concurrently, but this crate doesn't allow it by default
for compatibility. If you need to modify column families concurrently, enable
the crate feature `multi-threaded-cf`, which makes this binding's
data structures use `RwLock` by default. Alternatively, you can directly create
`DBWithThreadMode<MultiThreaded>` without enabling the crate feature.

## Switch between /MT or /MD run time library (Only for Windows)

The feature `mt_static` will request the library to be built with [/MT](https://learn.microsoft.com/en-us/cpp/build/reference/md-mt-ld-use-run-time-library?view=msvc-170)
flag, which results in library using the static version of the run-time library.
*This can be useful in case there's a conflict in the dependency tree between different
run-time versions.*

## Switch between static and dynamic linking for bindgen (features `bindgen-static` and `bindgen-runtime`)

The feature `bindgen-runtime` will enable the `runtime` feature of bindgen, which dynamically
links to libclang. This is suitable for most platforms, and is enabled by default.

The feature `bindgen-static` will enable the `static` feature of bindgen, which statically
links to libclang. This is suitable for musllinux platforms, such as Alpine linux.
To build on Alpine linux for example, make these changes to your Cargo.toml:

```toml
[dependencies.rocksdb]
default-features = false
features = ["bindgen-static", "snappy", "lz4", "zstd", "zlib", "bzip2"]
```

Notice that `runtime` and `static` features are mutually exclusive, and won't compile if both are enabled.

## LTO
Enable the `lto` feature to enable link-time optimization. It will compile rocksdb with `-flto` flag. This feature is disabled by default.

> [!IMPORTANT]
> You must use clang as `CC`. Eg. `CC=/usr/bin/clang CXX=/usr/bin/clang++`. Clang llvm version must be the same as the one used by rust compiler.
> On the rust side you should use `RUSTFLAGS="-Clinker-plugin-lto -Clinker=clang -Clink-arg=-fuse-ld=lld"`.

Check the [Rust documentation](https://doc.rust-lang.org/rustc/linker-plugin-lto.html) for more information.
