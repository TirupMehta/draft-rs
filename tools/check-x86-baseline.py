#!/usr/bin/env python3
"""Refuse to ship a draft.exe that was compiled for the build machine's CPU.

ggml builds its vendored CPU backend with `-march=native` unless GGML_NATIVE is
off. `.cargo/config.toml` pins it off, but that pin is one line away from being
deleted, and the failure it prevents is invisible on the machine that builds the
installer: a GitHub runner with AVX-512 produced the first 0.2.1 installer, and
draft.exe died with STATUS_ILLEGAL_INSTRUCTION (0xc000001d) on an AVX2 laptop.
The faulting instruction was `vmovups zmm0, zmmword ptr [rip + ...]`.

So decode .text and count instructions that touch a zmm register. A portable
build has a few hundred (decoder false positives on data bytes); a native build
on an AVX-512 host has thousands. The gap is wide enough that a threshold is a
reliable gate rather than a heuristic guess.

Usage: check-x86-baseline.py <draft.exe> [more.exe ...]
"""

import sys

from capstone import CS_ARCH_X86, CS_MODE_64, Cs

# Measured: 305 on an AVX2 build, 8,944 on the AVX-512 build that crashed.
THRESHOLD = 2000


def text_section(path):
    with open(path, "rb") as handle:
        data = handle.read()
    if data[:2] != b"MZ":
        raise SystemExit(f"{path}: not a PE file")
    pe = int.from_bytes(data[0x3C:0x40], "little")
    if data[pe:pe + 4] != b"PE\0\0":
        raise SystemExit(f"{path}: not a PE file")
    sections = int.from_bytes(data[pe + 6:pe + 8], "little")
    optional = int.from_bytes(data[pe + 20:pe + 22], "little")
    table = pe + 24 + optional
    for index in range(sections):
        entry = table + index * 40
        if data[entry:entry + 8].rstrip(b"\0") != b".text":
            continue
        size = int.from_bytes(data[entry + 16:entry + 20], "little")
        offset = int.from_bytes(data[entry + 20:entry + 24], "little")
        return data[offset:offset + size]
    raise SystemExit(f"{path}: no .text section")


def count_zmm(blob):
    decoder = Cs(CS_ARCH_X86, CS_MODE_64)
    hits = 0
    index = 0
    while True:
        index = blob.find(b"\x62", index)  # EVEX is the only 0x62-prefixed form
        if index < 0:
            return hits
        for insn in decoder.disasm(blob[index:index + 15], index, count=1):
            if insn.size and "zmm" in insn.op_str:
                hits += 1
                index += insn.size
                break
            index += 1
        else:
            index += 1


def main(paths):
    failed = False
    for path in paths:
        hits = count_zmm(text_section(path))
        state = "ok" if hits <= THRESHOLD else "AVX-512"
        print(f"{hits:>6} zmm instructions  {state:<8} {path}")
        failed = failed or hits > THRESHOLD
    if failed:
        print(
            f"\nThis binary needs AVX-512. Most Windows machines do not have it.\n"
            f"Check that .cargo/config.toml still passes -DGGML_NATIVE=OFF.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    raise SystemExit(main(sys.argv[1:]))
