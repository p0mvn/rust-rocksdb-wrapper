#!/usr/bin/env python3
"""Checks that a RocksDB archive built without `full` has no dangling
references outside the C API.

Without `full`, librocksdb-sys skips the sources in
`librocksdb-sys/zakura_excluded_sources.txt`, so the archive references
symbols that nothing defines. That is safe only while the code holding each
reference is never linked in. The C API functions (`rocksdb_*`) meet that:
the linker keeps one only when Rust calls it, and then the link fails, as the
README documents. Any other function can be kept live by the compiler, which
is how GCC 16 broke the link: it speculatively devirtualized a virtual call
into `WritePreparedSnapshotChecker::CheckInSnapshot`, which references the
skipped `WritePreparedTxnDB::GetCommitEntry`.

This script lists every RocksDB symbol the archive references but does not
define, finds the sections that reference it, and fails unless each one is a
C API function. It reads ELF archives built with `-ffunction-sections` (the
`cc` crate's default), and needs `nm`, `readelf` and `c++filt`.

Check optimized builds. Without optimization, compilers keep the unreachable
code after the early returns in `zakura_minimal.rs`, which the linker still
drops, and don't create the new calls this check looks for.

Usage: check_minimal_symbols.py ARCHIVE...
"""

import re
import subprocess
import sys
from collections import defaultdict

# A section that holds one C API function, including GCC's and clang's
# hot/cold splits of it.
C_API_SECTION = re.compile(
    r"^\.text(?:\.(?:unlikely|hot|startup|split|exit))?\.rocksdb_[A-Za-z0-9_]+$"
)
# Undefined symbols that can't cause a link error.
WEAK_UNDEFINED = {"w", "v"}


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


def demangle(symbols):
    if not symbols:
        return {}
    output = run_with_input(["c++filt"], "\n".join(symbols))
    return dict(zip(symbols, output.splitlines()))


def run_with_input(args, text):
    return subprocess.run(
        args, input=text, check=True, capture_output=True, text=True
    ).stdout


def dangling_symbols(archive):
    """Returns the RocksDB symbols `archive` references but never defines."""
    defined = set()
    undefined = set()
    for line in run("nm", archive).splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[0] == "U":
            undefined.add(fields[1])
        elif len(fields) == 3 and fields[1] not in WEAK_UNDEFINED:
            defined.add(fields[2])
    if not defined:
        sys.exit(f"{archive}: no symbols found; is it an LTO or non-ELF archive?")

    names = demangle(sorted(undefined - defined))
    return {
        symbol: name
        for symbol, name in names.items()
        if "rocksdb::" in name or symbol.startswith("rocksdb_")
    }


def referencing_sections(archive, symbols):
    """Maps each of `symbols` to the sections whose relocations reference it."""
    sections = defaultdict(set)
    section = None
    for line in run("readelf", "-W", "-r", archive).splitlines():
        header = re.match(r"Relocation section '\.rela?(\S+)'", line)
        if header:
            section = header.group(1)
            continue
        fields = line.split()
        if section and len(fields) >= 5 and fields[4] in symbols:
            sections[fields[4]].add(section)
    return sections


def check(archive):
    dangling = dangling_symbols(archive)
    sections = referencing_sections(archive, dangling)
    section_names = demangle(
        sorted(
            {
                re.sub(r"^\.(?:text|data|rodata)(?:\.[a-z.]+?)??\.(?=_Z|rocksdb_)", "", s)
                for refs in sections.values()
                for s in refs
            }
        )
    )

    c_api_only = 0
    failures = []
    for symbol, name in sorted(dangling.items(), key=lambda item: item[1]):
        refs = sections.get(symbol, set())
        if refs and all(C_API_SECTION.match(s) for s in refs):
            c_api_only += 1
            continue
        failures.append((name, refs))

    print(
        f"{archive}: {len(dangling)} undefined RocksDB symbols, "
        f"{c_api_only} referenced only from C API functions"
    )
    for name, refs in failures:
        print(f"  {name}")
        if not refs:
            print("    referenced from a section readelf could not attribute")
        for s in sorted(refs):
            stripped = re.sub(
                r"^\.(?:text|data|rodata)(?:\.[a-z.]+?)??\.(?=_Z|rocksdb_)", "", s
            )
            print(f"    <- {section_names.get(stripped, s)}")
    return not failures


def main(argv=None):
    archives = (argv if argv is not None else sys.argv[1:])
    if not archives:
        sys.exit(__doc__.strip().splitlines()[-1])
    ok = all([check(archive) for archive in archives])
    if not ok:
        print(
            "\nThese references can be kept live by the compiler and break the "
            "link. Edit the referencing code in librocksdb-sys/zakura_minimal.rs, "
            "or stop skipping the source that defines the symbol."
        )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
