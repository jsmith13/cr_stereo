#!/usr/bin/env python3
"""Build and validate the Circuit Rhythm build-5706 native stereo DSP patch."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path


EXPECTED_IMAGE_SIZE = 0x5A570
EXPECTED_IMAGE_SHA256 = "D5B9FF5DADF2E284F80D69ECB7087486CF0DF315A697BED8272AF9DE0E90DF15"
EXPECTED_PRIMARY_B_SHA256 = "D33B55580A0BAFE0028E861337A68D870D837FFFCF90A9070F9754676FF5C71A"
EXPECTED_PRIMARY_B_WORDS = 0x376B
EXPECTED_PRIMARY_B_TERMINATOR_INDEX = 14183
EXPECTED_PRIMARY_B_MAX_P = 0x0D31
OVERLAY_RAW_BASE = 0x2A33
EXTENSION_BASE = 0x0D32
EXTENSION_LIMIT = 0x1000
EXTENSION_MAX_WORDS = EXTENSION_LIMIT - EXTENSION_BASE


@dataclass(frozen=True)
class Hook:
    name: str
    raw: int
    vma: int
    extension: int
    mono_label: str
    continuation: int
    stock_words: tuple[int, int]


HOOKS = (
    Hook(
        "capture_finalize_entry",
        0x3299,
        0x0866,
        0x0D32,
        "finalize_mono",
        0x0868,
        (0x5EF000, 0x0371D5),
    ),
    Hook(
        "capture_peak_entry",
        0x32AE,
        0x087B,
        0x0D64,
        "peak_mono",
        0x087D,
        (0x60F400, 0x030790),
    ),
    Hook(
        "capture_average_entry",
        0x32D9,
        0x08A6,
        0x0DC0,
        "average_mono",
        0x08A8,
        (0x60F400, 0x030710),
    ),
    Hook(
        "capture_writer_entry",
        0x32F0,
        0x08BD,
        0x0DC9,
        "writer_mono",
        0x08BF,
        (0x5EF000, 0x0371D5),
    ),
    Hook(
        "capture_allocator_entry",
        0x35B6,
        0x0B83,
        0x0E23,
        "allocator_mono",
        0x0B85,
        (0x5EF000, 0x0371D1),
    ),
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest().upper()


def u24(value: int) -> bytes:
    if not 0 <= value <= 0xFFFFFF:
        raise ValueError(f"value is not a 24-bit word: {value:#x}")
    return value.to_bytes(3, "big")


def parse_loader_records(words: list[int]) -> tuple[list[tuple[int, int, int]], int]:
    records: list[tuple[int, int, int]] = []
    index = 2
    while index < len(words):
        space = words[index]
        if space > 2:
            return records, index
        if index + 2 >= len(words):
            raise ValueError("truncated DSP loader record header")
        count = words[index + 1]
        destination = words[index + 2]
        end = index + 3 + count
        if end > len(words):
            raise ValueError("truncated DSP loader record payload")
        records.append((space, destination, count))
        index = end
    raise ValueError("DSP loader stream has no terminator")


def parse_lod(path: Path) -> tuple[dict[int, int], dict[str, int]]:
    words: dict[int, int] = {}
    symbols: dict[str, int] = {}
    for line_number, line in enumerate(path.read_text(encoding="ascii").splitlines(), 1):
        line = line.strip()
        if not line or line.startswith(";"):
            continue
        symbol = re.fullmatch(r"I\s+([0-9A-Fa-f]+)\s+(\S+)", line)
        if symbol:
            symbols[symbol.group(2)] = int(symbol.group(1), 16)
            continue
        word = re.fullmatch(r"P\s+([0-9A-Fa-f]+)\s+([0-9A-Fa-f]{6})", line)
        if not word:
            raise ValueError(f"{path}:{line_number}: unsupported LOD line: {line}")
        address = int(word.group(1), 16)
        if address in words:
            raise ValueError(f"{path}:{line_number}: duplicate P:${address:04X}")
        words[address] = int(word.group(2), 16)
    if not words:
        raise ValueError(f"{path} contains no program words")
    return words, symbols


def run(command: list[str]) -> str:
    return subprocess.run(command, check=True, text=True, capture_output=True).stdout


def assemble(
    assembler: Path, source: Path, lod: Path, listing: Path
) -> tuple[dict[int, int], dict[str, int]]:
    run(
        [
            str(assembler),
            "-f",
            "lod",
            "-o",
            str(lod),
            "-l",
            str(listing),
            str(source),
        ]
    )
    return parse_lod(lod)


def disassemble(disassembler: Path, lod: Path, output: Path) -> None:
    text = run([str(disassembler), "-f", "lod", "-c", "never", str(lod)])
    output.write_text(text, encoding="ascii")
    decoded: dict[int, int] = {}
    for line in text.splitlines():
        match = re.match(r"P:\$([0-9A-Fa-f]+)\s+((?:[0-9A-Fa-f]{6}(?:\s+|$))+)", line)
        if not match:
            continue
        address = int(match.group(1), 16)
        for word in re.findall(r"[0-9A-Fa-f]{6}", match.group(2)):
            decoded[address] = int(word, 16)
            address += 1
    expected, _ = parse_lod(lod)
    if decoded != expected:
        raise ValueError(f"disassembler round trip differs for {lod.name}")


def disassemble_payload(
    disassembler: Path, payload: bytes, payload_path: Path, output: Path
) -> None:
    payload_path.write_bytes(payload)
    text = run(
        [
            str(disassembler),
            "-f",
            "u24be",
            "-s",
            hex(EXTENSION_BASE),
            "-c",
            "never",
            str(payload_path),
        ]
    )
    output.write_text(text, encoding="ascii")
    decoded: dict[int, int] = {}
    for line in text.splitlines():
        match = re.match(r"P:\$([0-9A-Fa-f]+)\s+((?:[0-9A-Fa-f]{6}(?:\s+|$))+)", line)
        if not match:
            continue
        address = int(match.group(1), 16)
        for word in re.findall(r"[0-9A-Fa-f]{6}", match.group(2)):
            decoded[address] = int(word, 16)
            address += 1
    expected = {
        EXTENSION_BASE + index: int.from_bytes(payload[index * 3 : index * 3 + 3], "big")
        for index in range(len(payload) // 3)
    }
    if decoded != expected:
        raise ValueError("packed extension payload failed disassembly round trip")


def packed_payload(words: dict[int, int]) -> bytes:
    if min(words) != EXTENSION_BASE:
        raise ValueError("extension does not begin at P:$0D32")
    return b"".join(
        u24(words.get(address, 0))
        for address in range(EXTENSION_BASE, max(words) + 1)
    )


def word_pairs(words: dict[int, int], opcode: int) -> list[int]:
    return [
        words[address + 1]
        for address in sorted(words)
        if words[address] == opcode and address + 1 in words
    ]


def validate_extension(
    words: dict[int, int], symbols: dict[str, int], stub_words: dict[int, int]
) -> dict[str, object]:
    if max(words) >= EXTENSION_LIMIT:
        raise ValueError("extension address exceeds reserved P:$0FFF")
    payload_words = max(words) - EXTENSION_BASE + 1
    if payload_words > EXTENSION_MAX_WORDS:
        raise ValueError("extension exceeds the 0x400-word reservation")

    expected_stub_addresses: set[int] = set()
    for hook in HOOKS:
        if hook.raw - hook.vma != OVERLAY_RAW_BASE:
            raise AssertionError(f"{hook.name}: raw/VMA relocation is inconsistent")
        if symbols.get(hook.name) != hook.extension:
            raise ValueError(f"{hook.name}: extension entry address changed")
        expected_stub_addresses.update((hook.vma, hook.vma + 1))
        if (stub_words.get(hook.vma), stub_words.get(hook.vma + 1)) != (
            0x0AF080,
            hook.extension,
        ):
            raise ValueError(f"{hook.name}: overlay stub is not the declared long jump")
        mono = symbols.get(hook.mono_label)
        if mono is None:
            raise ValueError(f"{hook.name}: missing mono continuation label")
        actual = tuple(words.get(mono + index) for index in range(4))
        expected = (*hook.stock_words, 0x0AF080, hook.continuation)
        if actual != expected:
            raise ValueError(f"{hook.name}: mono path is not the exact stock trampoline")
    if set(stub_words) != expected_stub_addresses:
        raise ValueError("stubs cover addresses outside the five declared hook ranges")

    required_symbols = {
        "stock_finalize_once",
        "stock_allocator_once",
        "resolve_slot_page",
        "peak_scan_done",
        "writer_copy_done",
        "allocator_fail",
        "allocator_restore_left_page",
        "stereo_active_state",
    }
    if not required_symbols <= symbols.keys():
        raise ValueError("extension is missing a required native-stereo routine")

    jsr_targets = word_pairs(words, 0x0BF080)
    jump_targets = word_pairs(words, 0x0AF080)
    if jsr_targets.count(0x000C97) != 1:
        raise ValueError("right slot is not enqueued exactly once through stock P:$0C97")
    if jsr_targets.count(symbols["stock_allocator_once"]) != 2:
        raise ValueError("paired allocation must execute exactly two stock page pops")
    if 0x000CE2 in jump_targets or 0x000CE2 in jsr_targets:
        raise ValueError("normalizer readiness hook is forbidden")
    if jump_targets.count(0x000973) != 1:
        raise ValueError("paired allocation failure must use stock finalization P:$0973")
    if (
        words.get(symbols["resolve_slot_page"]) != 0x275E00
        or words.get(symbols["stereo_active_state"]) != 0x56F400
        or words.get(symbols["stereo_active_state"] + 1) != 0x000004
        or words.get(symbols["allocator_restore_left_page"]) != 0x5C7000
        or words.get(symbols["allocator_restore_left_page"] + 1) != 0x0371D6
    ):
        raise ValueError(
            "load-bearing slot-stride, stereo-bit, or page-base encoding changed"
        )

    required_constants = {
        0x030710,
        0x030750,
        0x030001,
        0x0308C8,
        0x0366C8,
        0x036748,
        0x0371CF,
        0x0371D0,
        0x0371D1,
        0x0371D4,
        0x0371D5,
        0x0371D6,
        0x0371D7,
        0x0371E8,
        0x0371E9,
        0x0371F2,
    }
    missing_constants = sorted(required_constants - set(words.values()))
    if missing_constants:
        raise ValueError(
            "extension lacks required state/address constants: "
            + ", ".join(f"{value:06X}" for value in missing_constants)
        )
    for immediate in (0x00007F, 0x0000BC, 0x000A87, 0x000FFF):
        if immediate not in words.values():
            raise ValueError(f"extension lacks required invariant constant {immediate:06X}")

    return {
        "mono_stock_trampolines": True,
        "stereo_command_bit": "Y:$371D4.bit2",
        "left_slot_limit": 126,
        "shared_peak_slots": ["N", "N+1"],
        "paired_stock_page_pops": 2,
        "free_page_minimum": 2,
        "page_limit_per_slot": 376,
        "copy_frames_per_block": 32,
        "equal_extent_offsets": [1, 2],
        "normalizer_hooked": False,
        "ordinary_normalization_enqueues": 2,
    }


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", type=Path, required=True)
    parser.add_argument("--primary-b", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--assembler",
        type=Path,
        default=root / "tools" / "vendor" / "rust" / "cargo" / "bin" / "dsp56300-asm.exe",
    )
    parser.add_argument(
        "--disassembler",
        type=Path,
        default=root / "tools" / "vendor" / "rust" / "cargo" / "bin" / "dsp56300-disasm.exe",
    )
    args = parser.parse_args()

    image = args.image.read_bytes()
    if len(image) != EXPECTED_IMAGE_SIZE or sha256(image) != EXPECTED_IMAGE_SHA256:
        raise ValueError("input is not the pinned Circuit Rhythm build-5706 firmware")

    primary = args.primary_b.read_bytes()
    if sha256(primary) != EXPECTED_PRIMARY_B_SHA256:
        raise ValueError("primary-B block hash precondition failed")
    if len(primary) < 6:
        raise ValueError("primary-B block is truncated")
    count = int.from_bytes(primary[0:3], "big")
    destination = int.from_bytes(primary[3:6], "big")
    if count != EXPECTED_PRIMARY_B_WORDS or destination != 0:
        raise ValueError("primary-B header precondition failed")
    if len(primary) != (count + 2) * 3:
        raise ValueError("primary-B packed length does not match its header")
    primary_words = [
        int.from_bytes(primary[index : index + 3], "big")
        for index in range(0, len(primary), 3)
    ]
    primary_records, terminator_index = parse_loader_records(primary_words)
    if (
        terminator_index != EXPECTED_PRIMARY_B_TERMINATOR_INDEX
        or primary_words[terminator_index] != 0x000100
        or max(
            destination + record_count - 1
            for space, destination, record_count in primary_records
            if space == 0
        )
        != EXPECTED_PRIMARY_B_MAX_P
    ):
        raise ValueError("primary-B loader-record layout precondition failed")

    for hook in HOOKS:
        start = 6 + hook.raw * 3
        actual = tuple(
            int.from_bytes(primary[index : index + 3], "big")
            for index in range(start, start + 6, 3)
        )
        if actual != hook.stock_words:
            raise ValueError(
                f"{hook.name}: stock precondition failed at raw P:${hook.raw:04X}: "
                f"expected {hook.stock_words}, found {actual}"
            )

    args.output_dir.mkdir(parents=True, exist_ok=True)
    extension_lod = args.output_dir / "dual-native-mono-dsp-extension.lod"
    extension_listing = args.output_dir / "dual-native-mono-dsp-extension.lst"
    stubs_lod = args.output_dir / "dual-native-mono-dsp-stubs.lod"
    stubs_listing = args.output_dir / "dual-native-mono-dsp-stubs.lst"
    extension_words, symbols = assemble(
        args.assembler,
        root / "patches" / "dual-native-mono-dsp-extension.s",
        extension_lod,
        extension_listing,
    )
    stub_words, _ = assemble(
        args.assembler,
        root / "patches" / "dual-native-mono-dsp-stubs.s",
        stubs_lod,
        stubs_listing,
    )
    invariants = validate_extension(extension_words, symbols, stub_words)

    payload = packed_payload(extension_words)
    extension_count = len(payload) // 3
    package = (
        u24(0)
        + u24(extension_count)
        + u24(EXTENSION_BASE)
        + payload
        + u24(0x000100)
    )
    package_path = args.output_dir / "dual-native-mono-dsp-extension.dsp24"
    package_path.write_bytes(package)

    disassemble(
        args.disassembler,
        extension_lod,
        args.output_dir / "dual-native-mono-dsp-extension.disasm.txt",
    )
    disassemble(
        args.disassembler,
        stubs_lod,
        args.output_dir / "dual-native-mono-dsp-stubs.disasm.txt",
    )
    payload_path = args.output_dir / "dual-native-mono-dsp-extension.payload.u24be"
    disassemble_payload(
        args.disassembler,
        payload,
        payload_path,
        args.output_dir / "dual-native-mono-dsp-extension-packed.disasm.txt",
    )

    replacements = []
    for hook in HOOKS:
        replacement = b"".join(u24(stub_words[address]) for address in (hook.vma, hook.vma + 1))
        replacements.append(
            {
                "name": hook.name,
                "raw_lma_start": f"0x{hook.raw:04X}",
                "raw_lma_end_inclusive": f"0x{hook.raw + 1:04X}",
                "runtime_vma_start": f"0x{hook.vma:04X}",
                "runtime_vma_end_inclusive": f"0x{hook.vma + 1:04X}",
                "extension_entry": f"0x{hook.extension:04X}",
                "stock_continuation": f"0x{hook.continuation:04X}",
                "stock_precondition_words": [f"{word:06X}" for word in hook.stock_words],
                "stock_precondition_bytes": b"".join(u24(word) for word in hook.stock_words).hex().upper(),
                "replacement_words": [
                    f"{stub_words[hook.vma]:06X}",
                    f"{stub_words[hook.vma + 1]:06X}",
                ],
                "replacement_bytes": replacement.hex().upper(),
            }
        )

    metadata = {
        "schema": "circuit-rhythm-dual-native-mono-dsp-patch-v1",
        "status": "validated-native-dual-mono",
        "safe_for_integrated_firmware": True,
        "hardware_validation_required": True,
        "readiness_contract": (
            "ARM waits for capture inactive, equal nonzero final extents from source +0x10, "
            "and both normalized/readable extents from source +0x38 to reach final extent"
        ),
        "stock": {
            "build": 5706,
            "firmware_size": EXPECTED_IMAGE_SIZE,
            "firmware_sha256": EXPECTED_IMAGE_SHA256,
            "primary_b_sha256": EXPECTED_PRIMARY_B_SHA256,
            "primary_b_payload_words": EXPECTED_PRIMARY_B_WORDS,
            "primary_b_terminator_index": EXPECTED_PRIMARY_B_TERMINATOR_INDEX,
            "primary_b_streamed_prefix_bytes": (
                EXPECTED_PRIMARY_B_TERMINATOR_INDEX * 3
            ),
            "primary_b_max_program_address": f"0x{EXPECTED_PRIMARY_B_MAX_P:04X}",
        },
        "overlay_mapping": {
            "raw_lma_start": "0x2A33",
            "raw_lma_end_inclusive": "0x376A",
            "runtime_vma_start": "0x0000",
            "runtime_vma_end_inclusive": "0x0D37",
            "relocation": "-0x2A33",
        },
        "extension": {
            "base": "0x0D32",
            "word_count": extension_count,
            "maximum_word_count": EXTENSION_MAX_WORDS,
            "end_address_inclusive": f"0x{max(extension_words):04X}",
            "package_filename": package_path.name,
            "package_sha256": sha256(package),
            "payload_filename": payload_path.name,
            "payload_sha256": sha256(payload),
            "record_words": [
                "000000",
                f"{extension_count:06X}",
                f"{EXTENSION_BASE:06X}",
                "000100",
            ],
            "zero_filled_sparse_words": extension_count - len(extension_words),
            "loader_record_validated": (
                parse_loader_records(
                    primary_words[:terminator_index]
                    + [
                        0,
                        extension_count,
                        EXTENSION_BASE,
                        *[
                            int.from_bytes(payload[index : index + 3], "big")
                            for index in range(0, len(payload), 3)
                        ],
                        0x000100,
                    ]
                )[1]
                == terminator_index + 3 + extension_count
            ),
        },
        "invariants": invariants,
        "replacements": replacements,
        "round_trip_disassembly": True,
        "alternate_mode_modified": False,
        "firmware_image_emitted": False,
    }
    metadata_path = args.output_dir / "dual-native-mono-dsp-patch.json"
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="ascii")
    print(metadata_path)


if __name__ == "__main__":
    main()
