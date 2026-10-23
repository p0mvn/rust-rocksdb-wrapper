#!/usr/bin/env python3
"""Stops skipping the sources that compiled code still references.

Builds `zakura-librocksdb-sys` without `full`, in release and debug, finds the RocksDB symbols the
archive references but does not define (see `check_minimal_symbols.py`), and
removes from `librocksdb-sys/zakura_excluded_sources.txt` each skipped source
that defines one. It repeats until no symbol is undefined, since a source it
restores can reference other skipped sources.

Run it after a RocksDB upgrade, or after adding sources to the list, and
review the diff. A source is matched to a symbol by searching the skipped
sources for the symbol's definition, so a symbol it can't match stops it;
match that one by hand.

Usage: update_excluded_sources.py [--max-rounds N]
"""

import argparse
import glob
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from check_minimal_symbols import undefined_rocksdb_symbols  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SYS_DIR = os.path.join(ROOT, "librocksdb-sys")
EXCLUDED = os.path.join(SYS_DIR, "zakura_excluded_sources.txt")
BUILD = [
    "cargo", "build", "-p", "zakura-librocksdb-sys",
    "--no-default-features", "--features", "lz4,bindgen-runtime",
]
# Unoptimized builds emit inline functions that optimized builds drop, so they
# can reference more symbols.
PROFILES = {"release": ["--release"], "debug": []}


def excluded_sources():
    with open(EXCLUDED) as f:
        return [l.strip() for l in f if l.strip() and not l.startswith("#")]


def build_archive(profile):
    """Builds the minimal library and returns the path of its archive."""
    subprocess.run(BUILD + PROFILES[profile], cwd=ROOT, check=True)
    archives = glob.glob(
        os.path.join(ROOT, f"target/{profile}/build/zakura-librocksdb-sys-*/out/librocksdb.a")
    )
    return max(archives, key=os.path.getmtime)


def undefined_symbols():
    """Returns the RocksDB symbols a release or debug archive leaves undefined."""
    return sorted({s for p in PROFILES for s in undefined_rocksdb_symbols(build_archive(p))})


def definition_patterns(symbol):
    """Returns regexes that match the definition of a demangled `symbol`.

    RocksDB starts each function definition at column 0, while calls are
    indented, and a declaration ends with `;`.
    """
    name = re.sub(r"^(vtable|typeinfo|typeinfo name) for ", "", symbol)
    is_type = name != symbol
    name = re.sub(r"\(.*$", "", name).replace("[abi:cxx11]", "")
    parts = [p for p in name.split("::") if p and p != "rocksdb"]
    if is_type:
        # A vtable is emitted with the class's first out-of-line virtual function.
        needle = re.escape(parts[-1]) + r"::~?\w+"
    elif len(parts) >= 2:
        needle = re.escape(f"{parts[-2]}::{parts[-1]}")
    else:
        needle = r"\b" + re.escape(parts[-1])
    # The return type is optional: constructors start with the class name.
    return [r"(?m)^(?:[^\s#/][^;\n]*?)?" + needle + r"\s*(\(|=)"]


def defining_source(symbol, sources):
    for source in sources:
        with open(os.path.join(SYS_DIR, "rocksdb", source), errors="ignore") as f:
            text = f.read()
        if any(re.search(p, text) for p in definition_patterns(symbol)):
            return source
    return None


def restore(sources):
    with open(EXCLUDED) as f:
        lines = f.readlines()
    with open(EXCLUDED, "w") as f:
        f.writelines(l for l in lines if l.strip() not in sources)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.strip().splitlines()[0])
    parser.add_argument("--max-rounds", type=int, default=10)
    args = parser.parse_args(argv)

    for round_ in range(1, args.max_rounds + 1):
        undefined = undefined_symbols()
        print(f"round {round_}: {len(undefined)} undefined RocksDB symbols")
        if not undefined:
            return 0

        excluded = excluded_sources()
        to_restore = set()
        unmatched = []
        for symbol in undefined:
            source = defining_source(symbol, excluded)
            if source:
                to_restore.add(source)
            else:
                unmatched.append(symbol)
        for source in sorted(to_restore):
            print(f"  compiling again: {source}")
        if unmatched:
            print("  no skipped source defines:")
            for symbol in unmatched:
                print(f"    {symbol}")
            return 1
        restore(to_restore)

    print(f"still undefined after {args.max_rounds} rounds")
    return 1


if __name__ == "__main__":
    sys.exit(main())
