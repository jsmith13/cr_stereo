#!/usr/bin/env python3
"""Extract and validate Novation Circuit Tracks/Rhythm firmware from SysEx."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


NOVATION_HEADER = bytes((0x00, 0x20, 0x29, 0x00))
DEVICES = {
    0x63: "rhythm",
    0x64: "tracks",
}
COMMANDS = {
    0x71: "UPDATE_INIT",
    0x72: "UPDATE_WRITE",
    0x73: "UPDATE_FINISH",
    0x76: "UPDATE_FOOTER",
    0x7C: "UPDATE_HEADER",
}


def parse_nibbles(data: bytes) -> int:
    if any(value > 0x0F for value in data):
        raise ValueError("Expected a nibble-encoded integer")
    result = 0
    for value in data:
        result = (result << 4) | value
    return result


def unpack_midi_7bit(data: bytes) -> bytes:
    bits: list[int] = []
    for value in data:
        if value > 0x7F:
            raise ValueError("Firmware payload contains a non-MIDI data byte")
        bits.extend((value >> shift) & 1 for shift in range(6, -1, -1))

    # Each 38-byte payload carries 266 significant bits. The final three bits
    # are padding, leaving 263 bits; pack complete bytes and discard the tail.
    bits = bits[:-3]
    output = bytearray()
    for offset in range(0, len(bits) - 7, 8):
        value = 0
        for bit in bits[offset : offset + 8]:
            value = (value << 1) | bit
        output.append(value)
    return bytes(output)


def crc32_mpeg2(data: bytes) -> int:
    crc = 0xFFFFFFFF
    for value in data:
        crc ^= value << 24
        for _ in range(8):
            crc = ((crc << 1) ^ 0x04C11DB7) & 0xFFFFFFFF if crc & 0x80000000 else (crc << 1) & 0xFFFFFFFF
    return crc


def iter_sysex_messages(data: bytes):
    offset = 0
    while offset < len(data):
        start = data.find(b"\xF0", offset)
        if start == -1:
            if any(value not in (0x00, 0x0A, 0x0D) for value in data[offset:]):
                raise ValueError(f"Unexpected trailing data at offset {offset:#x}")
            return

        if any(value not in (0x00, 0x0A, 0x0D) for value in data[offset:start]):
            raise ValueError(f"Unexpected data before SysEx message at offset {start:#x}")

        end = data.find(b"\xF7", start + 1)
        if end == -1:
            raise ValueError(f"Missing SysEx terminator for message at offset {start:#x}")

        message = data[start + 1 : end]
        if not message.startswith(NOVATION_HEADER):
            raise ValueError(f"Non-Novation SysEx message at offset {start:#x}")
        if len(message) < len(NOVATION_HEADER) + 1:
            raise ValueError(f"Truncated SysEx message at offset {start:#x}")

        command_id = message[len(NOVATION_HEADER)]
        try:
            command = COMMANDS[command_id]
        except KeyError as error:
            raise ValueError(f"Unknown update command {command_id:#x} at offset {start:#x}") from error

        yield command, message[len(NOVATION_HEADER) + 1 :]
        offset = end + 1


def extract(data: bytes) -> tuple[bytes, dict[str, object]]:
    init_version = None
    header_version = None
    device = None
    declared_size = None
    declared_crc = None
    finish_block = None
    write_blocks: list[bytes] = []
    command_counts = {name: 0 for name in COMMANDS.values()}

    for command, payload in iter_sysex_messages(data):
        command_counts[command] += 1

        if command == "UPDATE_INIT":
            if len(payload) < 8:
                raise ValueError("Truncated UPDATE_INIT payload")
            try:
                device = DEVICES[payload[1]]
            except KeyError as error:
                raise ValueError(f"Unknown Circuit device ID {payload[1]:#x}") from error
            init_version = parse_nibbles(payload[2:8])
        elif command == "UPDATE_HEADER":
            if len(payload) < 23:
                raise ValueError("Truncated UPDATE_HEADER payload")
            header_version = parse_nibbles(payload[1:7])
            declared_size = parse_nibbles(payload[7:15])
            declared_crc = parse_nibbles(payload[15:23])
        elif command == "UPDATE_WRITE":
            write_blocks.append(unpack_midi_7bit(payload))
        elif command == "UPDATE_FINISH":
            if finish_block is not None:
                raise ValueError("Multiple UPDATE_FINISH messages")
            finish_block = unpack_midi_7bit(payload)

    if device is None or init_version is None or header_version is None:
        raise ValueError("Missing firmware identity metadata")
    if init_version != header_version:
        raise ValueError(
            f"Firmware version mismatch: INIT {init_version}, HEADER {header_version}"
        )
    version = header_version
    if declared_size is None or declared_crc is None:
        raise ValueError("Missing firmware header metadata")
    if finish_block is None:
        raise ValueError("Missing UPDATE_FINISH message")

    # Novation places the first decoded firmware block in UPDATE_FINISH even
    # though that message follows all UPDATE_WRITE messages in the SysEx file.
    firmware = (finish_block + b"".join(write_blocks))[:declared_size]
    if len(firmware) != declared_size:
        raise ValueError(
            f"Extracted {len(firmware):#x} bytes, expected {declared_size:#x}"
        )

    calculated_crc = crc32_mpeg2(firmware)
    if calculated_crc != declared_crc:
        raise ValueError(
            f"CRC mismatch: calculated {calculated_crc:08X}, expected {declared_crc:08X}"
        )

    metadata: dict[str, object] = {
        "device": device,
        "version": version,
        "size_bytes": len(firmware),
        "crc32_mpeg2": f"{calculated_crc:08X}",
        "sha256": hashlib.sha256(firmware).hexdigest().upper(),
        "command_counts": {key: value for key, value in command_counts.items() if value},
    }
    return firmware, metadata


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path, help="Novation firmware SysEx file")
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help="Output path; defaults to the input path with a .bin suffix",
    )
    args = parser.parse_args()

    output = args.output or args.input.with_suffix(".bin")
    firmware, metadata = extract(args.input.read_bytes())
    output.write_bytes(firmware)
    print(json.dumps({"output": str(output), **metadata}, indent=2))


if __name__ == "__main__":
    main()
