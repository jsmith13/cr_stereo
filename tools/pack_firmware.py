#!/usr/bin/env python3
"""Repack a decoded Circuit firmware image using an official SysEx template."""

from __future__ import annotations

import argparse
from pathlib import Path

from extract_firmware import (
    COMMANDS,
    NOVATION_HEADER,
    crc32_mpeg2,
    extract,
    iter_sysex_messages,
)


COMMAND_IDS = {name: command_id for command_id, name in COMMANDS.items()}
BLOCK_SIZE = 32


def encode_nibbles(value: int, digits: int) -> bytes:
    if value < 0 or value >= 1 << (digits * 4):
        raise ValueError(f"{value:#x} does not fit in {digits} nibbles")
    return bytes((value >> shift) & 0xF for shift in range((digits - 1) * 4, -1, -4))


def pack_midi_7bit(data: bytes) -> bytes:
    if len(data) != BLOCK_SIZE:
        raise ValueError(f"Expected one {BLOCK_SIZE}-byte firmware block")

    bits: list[int] = []
    for value in data:
        bits.extend((value >> shift) & 1 for shift in range(7, -1, -1))
    bits.extend((0, 0, 0))

    output = bytearray()
    for offset in range(0, len(bits), 7):
        value = 0
        for bit in bits[offset : offset + 7]:
            value = (value << 1) | bit
        output.append(value)
    return bytes(output)


def frame(command: str, payload: bytes) -> bytes:
    return (
        b"\xF0"
        + NOVATION_HEADER
        + bytes((COMMAND_IDS[command],))
        + payload
        + b"\xF7"
    )


def blocks(image: bytes) -> list[bytes]:
    padded_size = (len(image) + BLOCK_SIZE - 1) // BLOCK_SIZE * BLOCK_SIZE
    padded = image.ljust(padded_size, b"\xFF")
    return [
        padded[offset : offset + BLOCK_SIZE]
        for offset in range(0, padded_size, BLOCK_SIZE)
    ]


def repack(template: bytes, image: bytes, version: int | None = None) -> bytes:
    if not image:
        raise ValueError("Firmware image is empty")

    image_blocks = blocks(image)
    image_crc = crc32_mpeg2(image)
    messages = list(iter_sysex_messages(template))
    output = bytearray()
    emitted_writes = False

    for command, payload in messages:
        if command == "UPDATE_INIT":
            if len(payload) < 8:
                raise ValueError("Truncated template UPDATE_INIT")
            updated = bytearray(payload)
            if version is not None:
                updated[2:8] = encode_nibbles(version, 6)
            output.extend(frame(command, updated))
        elif command == "UPDATE_HEADER":
            if len(payload) < 23:
                raise ValueError("Truncated template UPDATE_HEADER")
            updated = bytearray(payload)
            if version is not None:
                updated[1:7] = encode_nibbles(version, 6)
            updated[7:15] = encode_nibbles(len(image), 8)
            updated[15:23] = encode_nibbles(image_crc, 8)
            output.extend(frame(command, updated))
        elif command == "UPDATE_WRITE":
            if not emitted_writes:
                for block in image_blocks[1:]:
                    output.extend(frame(command, pack_midi_7bit(block)))
                emitted_writes = True
        elif command == "UPDATE_FINISH":
            output.extend(frame(command, pack_midi_7bit(image_blocks[0])))
        else:
            output.extend(frame(command, payload))

    if not emitted_writes:
        raise ValueError("Template contains no UPDATE_WRITE messages")

    decoded, _ = extract(bytes(output))
    if decoded != image:
        raise ValueError("Repacked SysEx failed decode round trip")
    return bytes(output)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("template", type=Path, help="Official SysEx container")
    parser.add_argument("image", type=Path, help="Decoded firmware image")
    parser.add_argument("--version", type=lambda value: int(value, 0))
    parser.add_argument("-o", "--output", required=True, type=Path)
    args = parser.parse_args()

    packed = repack(
        args.template.read_bytes(),
        args.image.read_bytes(),
        version=args.version,
    )
    args.output.write_bytes(packed)
    print(f"Wrote {len(packed)} bytes to {args.output}")


if __name__ == "__main__":
    main()
