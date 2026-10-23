#!/usr/bin/env python3
"""Checks that a RocksDB archive built without `full` references no code it
skips.

Without `full`, librocksdb-sys skips the sources in
`librocksdb-sys/zakura_excluded_sources.txt`. If compiled code still
references a skipped source, the archive has an undefined symbol, and whether
the link succeeds depends on the linker dropping the referencing code. GCC 16
kept such a reference live by speculatively devirtualizing a call, and MSVC's
linker resolves every symbol of an object before dropping anything. So the
archive must define every RocksDB symbol it references.

This script lists the RocksDB symbols each archive references but does not
define, and fails if there are any. `update_excluded_sources.py` uses it to
stop skipping the sources that define them. It reads ELF archives and needs
`nm` and `c++filt`.

Usage: check_minimal_symbols.py ARCHIVE...
"""

import subprocess
import sys

# Undefined symbols that can't cause a link error.
WEAK_UNDEFINED = {"w", "v"}


def undefined_rocksdb_symbols(archive):
    """Returns the demangled RocksDB symbols `archive` references but never
    defines."""
    nm = subprocess.run(["nm", archive], check=True, capture_output=True, text=True)
    defined = set()
    undefined = set()
    for line in nm.stdout.splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[0] == "U":
            undefined.add(fields[1])
        elif len(fields) == 3 and fields[1] not in WEAK_UNDEFINED:
            defined.add(fields[2])
    if not defined:
        sys.exit(f"{archive}: no symbols found; is it an LTO or non-ELF archive?")

    symbols = sorted(undefined - defined)
    demangled = subprocess.run(
        ["c++filt"], input="\n".join(symbols), check=True, capture_output=True, text=True
    ).stdout.splitlines()
    return sorted(
        name
        for symbol, name in zip(symbols, demangled)
        if "rocksdb::" in name or symbol.startswith("rocksdb_")
    )


def main(argv=None):
    archives = argv if argv is not None else sys.argv[1:]
    if not archives:
        sys.exit(__doc__.strip().splitlines()[-1])

    ok = True
    for archive in archives:
        undefined = undefined_rocksdb_symbols(archive)
        print(f"{archive}: {len(undefined)} undefined RocksDB symbols")
        for name in undefined:
            print(f"  {name}")
        ok = ok and not undefined
    if not ok:
        print(
            "\nCompiled code references skipped sources. Run "
            ".github/scripts/update_excluded_sources.py to stop skipping the "
            "sources that define these symbols, or remove the references with "
            "an edit in librocksdb-sys/zakura_minimal.rs."
        )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
