#!/usr/bin/env python3
"""Disassemble a region of a raw Cortex-M firmware image."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

from capstone import CS_ARCH_ARM, CS_MODE_LITTLE_ENDIAN, CS_MODE_MCLASS, CS_MODE_THUMB, Cs


DEFAULT_BASE = 0x08010000


def parse_int(value: str) -> int:
    return int(value, 0)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("--base", type=parse_int, default=DEFAULT_BASE)
    parser.add_argument(
        "--address",
        type=parse_int,
        help="Address to disassemble; defaults to the vector-table reset handler",
    )
    parser.add_argument("--bytes", type=parse_int, default=0x100)
    args = parser.parse_args()

    image = args.image.read_bytes()
    address = args.address
    if address is None:
        if len(image) < 8:
            raise ValueError("Image is too small to contain a vector table")
        address = struct.unpack_from("<I", image, 4)[0]

    code_address = address & ~1
    offset = code_address - args.base
    if offset < 0 or offset >= len(image):
        raise ValueError(
            f"Address {address:#010x} is outside image "
            f"{args.base:#010x}..{args.base + len(image):#010x}"
        )

    disassembler = Cs(
        CS_ARCH_ARM,
        CS_MODE_THUMB | CS_MODE_MCLASS | CS_MODE_LITTLE_ENDIAN,
    )
    disassembler.detail = False
    data = image[offset : offset + args.bytes]
    instructions = list(disassembler.disasm(data, code_address))
    if not instructions:
        raise ValueError(f"No Thumb instructions decoded at {code_address:#010x}")

    print(
        f"image={args.image} base={args.base:#010x} "
        f"address={code_address:#010x} offset={offset:#x}"
    )
    for instruction in instructions:
        encoded = instruction.bytes.hex(" ")
        print(
            f"{instruction.address:08X}  {encoded:<14}  "
            f"{instruction.mnemonic:<8} {instruction.op_str}"
        )


if __name__ == "__main__":
    main()
