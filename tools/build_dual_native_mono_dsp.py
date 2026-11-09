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
# Granular probe builds only: core-B load telemetry in its own loader record
# after the capture extension. Its two hooks are one-word short `jsr`s, so the
# record must stay below P:$1000. The hook sites live in the stock loader
# record at P:$0100 (raw = VMA - $E8), not in the relocated overlay.
PROBE_BASE = 0x0EC0
PROBE_LIMIT = 0x1000
SHORT_JSR = 0x0D0000
# Granular looper builds only: the core-B refill copy declick. The first word
# (`move a,n5`) of each stock copy routine in the relocated overlay becomes a
# short `jmp` to the wrapper, which must therefore stay below P:$1000.
DECLICK_BASE = 0x0F00
DECLICK_LIMIT = 0x1000
SHORT_JMP = 0x0C0000
STOCK_MOVE_A_N5 = 0x21DD00
# Granular looper builds only: the core-B grain engine. The main loop's
# `jsr $0221` (voice pass) becomes a short jsr to the hook just above the
# declick record. The engine itself is too large for internal P, so it runs
# from shared memory at $37400 (P/X/Y alias one array): the loader puts a
# master copy in internal Y:$1800, which the hook copies to $37400 whenever the
# magic word there is missing.
ENGINE_INTERNAL_BASE = 0x0FA0
ENGINE_INTERNAL_LIMIT = 0x1000
ENGINE_SHARED_BASE = 0x037400
ENGINE_MASTER_Y = 0x1800
ENGINE_MASTER_LIMIT = 0x2000
ENGINE_MAGIC = 0x47524E
ENGINE_HOOK_VMA = 0x011D
ENGINE_HOOK_STOCK = 0x0D0221
LOADER_SPACE_Y = 2


@dataclass(frozen=True)
class DeclickHook:
    name: str
    vma: int
    entry: str
    raw: int
    stock_words: tuple[int, ...]
    long_branch: int | None = None

    def replacement_words(self, entry_address: int) -> tuple[int, ...]:
        if self.long_branch is None:
            return (SHORT_JMP | entry_address,)
        return (self.long_branch, entry_address)


# Raw offsets differ per loader record; main() re-derives each from the stock
# stream and refuses to build if they disagree.
DECLICK_HOOKS = (
    DeclickHook("declick_copy_forward", 0x073D, "dk_forward", 0x073D + OVERLAY_RAW_BASE,
                (STOCK_MOVE_A_N5,)),
    DeclickHook("declick_copy_reverse", 0x0798, "dk_reverse", 0x0798 + OVERLAY_RAW_BASE,
                (STOCK_MOVE_A_N5,)),
    # Envelope jump-table entry 4 (`bra $0636`): the retrigger fade-out.
    DeclickHook("declick_stage4", 0x05D2, "dk_stage4", 0x2FFE, (0x050CC4,)),
    # `jsne >$02FD` in the pending-start loop: defer a start while fading out.
    DeclickHook("declick_start", 0x02F2, "dk_start", 0x2B2F, (0x0BF0A2, 0x0002FD), 0x0BF0A2),
)


def p_raw_offset(words: list[int], vma: int) -> int:
    index = 2
    while index < len(words) and words[index] <= 2:
        space, count, destination = words[index : index + 3]
        if space == 0 and destination <= vma < destination + count:
            return index + 3 + vma - destination - 2
        index += 3 + count
    raise ValueError(f"P:${vma:04X} is not loaded by the stock stream")


@dataclass(frozen=True)
class ProbeHook:
    name: str
    vma: int
    raw: int
    entry: str
    stock_word: int


PROBE_HOOKS = (
    ProbeHook("probe_idle_spin", 0x0111, 0x0029, "gp_idle_spin", 0x0D07F8),
    ProbeHook("probe_block_end", 0x0126, 0x003E, "gp_block_end", 0x0D0A79),
)


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


def validate_probe(
    words: dict[int, int], symbols: dict[str, int], extension_end: int
) -> bytes:
    if min(words) != PROBE_BASE:
        raise ValueError(f"probe record does not begin at P:${PROBE_BASE:04X}")
    if extension_end >= PROBE_BASE:
        raise ValueError("capture extension overlaps the probe record")
    if max(words) >= PROBE_LIMIT:
        raise ValueError("probe record exceeds the short-jsr range")
    for hook in PROBE_HOOKS:
        entry = symbols.get(hook.entry)
        if entry is None or not PROBE_BASE <= entry < PROBE_LIMIT:
            raise ValueError(f"{hook.name}: missing or out-of-range probe entry")
    spin = symbols["gp_idle_spin"]
    if words.get(spin + 6) != 0x0AF080 or words.get(spin + 7) != 0x0007F8:
        raise ValueError("probe idle spin does not resume stock P:$07F8")
    block = symbols["gp_block_end"]
    if words.get(block) != 0x0BF080 or words.get(block + 1) != 0x000A79:
        raise ValueError("probe block hook does not call stock P:$0A79 first")
    return b"".join(
        u24(words.get(address, 0)) for address in range(PROBE_BASE, max(words) + 1)
    )


def validate_declick(
    words: dict[int, int], symbols: dict[str, int], extension_end: int
) -> bytes:
    if min(words) != DECLICK_BASE:
        raise ValueError(f"declick record does not begin at P:${DECLICK_BASE:04X}")
    if extension_end >= DECLICK_BASE:
        raise ValueError("capture extension overlaps the declick record")
    if max(words) >= DECLICK_LIMIT:
        raise ValueError("declick record exceeds the short-jump range")
    for hook in DECLICK_HOOKS:
        entry = symbols.get(hook.entry)
        if entry is None or not DECLICK_BASE <= entry < DECLICK_LIMIT:
            raise ValueError(f"{hook.name}: missing or out-of-range declick entry")
        if hook.stock_words != (STOCK_MOVE_A_N5,):
            continue
        if words.get(entry) != STOCK_MOVE_A_N5:
            raise ValueError(f"{hook.name}: wrapper does not replay the stock first word")
        stock_rest = hook.vma + 1
        if word_pairs(words, 0x0AF0AA).count(stock_rest) != 1:
            raise ValueError(f"{hook.name}: unflagged voices do not resume stock")
        if word_pairs(words, 0x0BF080).count(stock_rest) != 1:
            raise ValueError(f"{hook.name}: flagged voices do not run the stock copy")
    if word_pairs(words, 0x0AF0AA).count(0x0636) != 1 or word_pairs(words, 0x0AF0A9).count(0x0636) != 1:
        raise ValueError("declick_stage4: unflagged or quiet voices do not finish in stock stage 4")
    if word_pairs(words, 0x0AF0AA).count(0x02FD) != 2 or word_pairs(words, 0x0AF0A2).count(0x02FD) != 1:
        raise ValueError("declick_start: non-deferred starts do not reach the stock voice start")
    return b"".join(
        u24(words.get(address, 0)) for address in range(DECLICK_BASE, max(words) + 1)
    )


def validate_engine(
    words: dict[int, int], symbols: dict[str, int], declick_end: int
) -> tuple[bytes, bytes]:
    internal = {a: w for a, w in words.items() if a < ENGINE_INTERNAL_LIMIT}
    shared = {a: w for a, w in words.items() if a >= ENGINE_SHARED_BASE}
    if len(internal) + len(shared) != len(words):
        raise ValueError("grain engine has words outside its two sections")
    if min(internal) != ENGINE_INTERNAL_BASE:
        raise ValueError(f"grain hook record does not begin at P:${ENGINE_INTERNAL_BASE:04X}")
    if declick_end >= ENGINE_INTERNAL_BASE:
        raise ValueError("declick record overlaps the grain hook record")
    if symbols.get("ge_hook") != ENGINE_INTERNAL_BASE:
        raise ValueError("ge_hook is not the first internal word")
    if internal[ENGINE_INTERNAL_BASE] != ENGINE_HOOK_STOCK:
        raise ValueError("grain hook does not run the stock voice pass first")
    if min(shared) != ENGINE_SHARED_BASE or shared[ENGINE_SHARED_BASE] != ENGINE_MAGIC:
        raise ValueError("grain engine does not begin with its magic word")
    shared_words = max(shared) - ENGINE_SHARED_BASE + 1
    if ENGINE_MASTER_Y + shared_words > ENGINE_MASTER_LIMIT:
        raise ValueError("grain engine master copy exceeds Y:$1FFF")
    if symbols.get("ge_end", 0) - ENGINE_SHARED_BASE != shared_words:
        raise ValueError("grain engine length symbol does not match its words")
    if symbols.get("ge_run") != ENGINE_SHARED_BASE + 1:
        raise ValueError("ge_run does not follow the magic word")
    copy_do = 0x060080 | ((shared_words & 0xFF) << 8) | (shared_words >> 8)
    if copy_do not in internal.values():
        raise ValueError("grain hook copy count does not match the engine length")
    internal_payload = b"".join(
        u24(internal.get(a, 0)) for a in range(ENGINE_INTERNAL_BASE, max(internal) + 1)
    )
    shared_payload = b"".join(
        u24(shared.get(a, 0)) for a in range(ENGINE_SHARED_BASE, max(shared) + 1)
    )
    return internal_payload, shared_payload


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
    parser.add_argument(
        "--granular-probe",
        action="store_true",
        help="append the core-B load telemetry record (diagnostic builds only)",
    )
    parser.add_argument(
        "--looper-declick",
        action="store_true",
        help="append the core-B refill declick record (granular looper builds only)",
    )
    parser.add_argument(
        "--granular-engine",
        action="store_true",
        help="append the core-B grain engine (granular looper builds only; needs --looper-declick)",
    )
    args = parser.parse_args()
    if args.granular_probe and args.looper_declick:
        raise ValueError("the granular probe and looper declick are exclusive")
    if args.granular_engine and not args.looper_declick:
        raise ValueError("the grain engine requires the looper declick")

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
    if args.granular_probe:
        for probe_hook in PROBE_HOOKS:
            start = 6 + probe_hook.raw * 3
            actual_word = int.from_bytes(primary[start : start + 3], "big")
            if actual_word != probe_hook.stock_word:
                raise ValueError(
                    f"{probe_hook.name}: stock precondition failed at P:${probe_hook.vma:04X}"
                )
    if args.looper_declick:
        stock_stream = [
            int.from_bytes(primary[index : index + 3], "big")
            for index in range(0, len(primary) - len(primary) % 3, 3)
        ]
        for declick_hook in DECLICK_HOOKS:
            if p_raw_offset(stock_stream, declick_hook.vma) != declick_hook.raw:
                raise AssertionError(f"{declick_hook.name}: raw offset does not match the loader stream")
            start = 6 + declick_hook.raw * 3
            width = 3 * len(declick_hook.stock_words)
            if primary[start : start + width] != b"".join(map(u24, declick_hook.stock_words)):
                raise ValueError(
                    f"{declick_hook.name}: stock precondition failed at P:${declick_hook.vma:04X}"
                )
    engine_hook_raw = 0
    if args.granular_engine:
        engine_hook_raw = p_raw_offset(stock_stream, ENGINE_HOOK_VMA)
        start = 6 + engine_hook_raw * 3
        if primary[start : start + 3] != u24(ENGINE_HOOK_STOCK):
            raise ValueError("grain engine hook: stock precondition failed at P:$011D")

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
    probe_record = b""
    probe_payload = b""
    probe_symbols: dict[str, int] = {}
    if args.granular_probe:
        probe_lod = args.output_dir / "granular-probe-dsp.lod"
        probe_words, probe_symbols = assemble(
            args.assembler,
            root / "patches" / "granular-probe-dsp.s",
            probe_lod,
            args.output_dir / "granular-probe-dsp.lst",
        )
        probe_payload = validate_probe(probe_words, probe_symbols, max(extension_words))
        probe_record = u24(0) + u24(len(probe_payload) // 3) + u24(PROBE_BASE) + probe_payload
        disassemble(
            args.disassembler,
            probe_lod,
            args.output_dir / "granular-probe-dsp.disasm.txt",
        )
    declick_record = b""
    declick_payload = b""
    declick_symbols: dict[str, int] = {}
    if args.looper_declick:
        declick_lod = args.output_dir / "looper-declick-dsp.lod"
        declick_words, declick_symbols = assemble(
            args.assembler,
            root / "patches" / "looper-declick-dsp.s",
            declick_lod,
            args.output_dir / "looper-declick-dsp.lst",
        )
        declick_payload = validate_declick(
            declick_words, declick_symbols, max(extension_words)
        )
        declick_record = (
            u24(0) + u24(len(declick_payload) // 3) + u24(DECLICK_BASE) + declick_payload
        )
        disassemble(
            args.disassembler,
            declick_lod,
            args.output_dir / "looper-declick-dsp.disasm.txt",
        )
    engine_record = b""
    engine_internal = b""
    engine_shared = b""
    engine_symbols: dict[str, int] = {}
    if args.granular_engine:
        engine_lod = args.output_dir / "granular-engine-dsp.lod"
        engine_words, engine_symbols = assemble(
            args.assembler,
            root / "patches" / "granular-engine-dsp.s",
            engine_lod,
            args.output_dir / "granular-engine-dsp.lst",
        )
        engine_internal, engine_shared = validate_engine(
            engine_words, engine_symbols, DECLICK_BASE + len(declick_payload) // 3 - 1
        )
        engine_record = (
            u24(0) + u24(len(engine_internal) // 3) + u24(ENGINE_INTERNAL_BASE) + engine_internal
            + u24(LOADER_SPACE_Y) + u24(len(engine_shared) // 3) + u24(ENGINE_MASTER_Y)
            + engine_shared
        )
        disassemble(
            args.disassembler,
            engine_lod,
            args.output_dir / "granular-engine-dsp.disasm.txt",
        )
    package = (
        u24(0)
        + u24(extension_count)
        + u24(EXTENSION_BASE)
        + payload
        + probe_record
        + declick_record
        + engine_record
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
    for probe_hook in PROBE_HOOKS if args.granular_probe else ():
        raw = probe_hook.raw
        word = SHORT_JSR | probe_symbols[probe_hook.entry]
        replacements.append(
            {
                "name": probe_hook.name,
                "raw_lma_start": f"0x{raw:04X}",
                "raw_lma_end_inclusive": f"0x{raw:04X}",
                "runtime_vma_start": f"0x{probe_hook.vma:04X}",
                "runtime_vma_end_inclusive": f"0x{probe_hook.vma:04X}",
                "extension_entry": f"0x{probe_symbols[probe_hook.entry]:04X}",
                "stock_continuation": f"0x{probe_hook.vma + 1:04X}",
                "stock_precondition_words": [f"{probe_hook.stock_word:06X}"],
                "stock_precondition_bytes": u24(probe_hook.stock_word).hex().upper(),
                "replacement_words": [f"{word:06X}"],
                "replacement_bytes": u24(word).hex().upper(),
            }
        )
    for declick_hook in DECLICK_HOOKS if args.looper_declick else ():
        raw = declick_hook.raw
        entry_address = declick_symbols[declick_hook.entry]
        new_words = declick_hook.replacement_words(entry_address)
        last = len(new_words) - 1
        replacements.append(
            {
                "name": declick_hook.name,
                "raw_lma_start": f"0x{raw:04X}",
                "raw_lma_end_inclusive": f"0x{raw + last:04X}",
                "runtime_vma_start": f"0x{declick_hook.vma:04X}",
                "runtime_vma_end_inclusive": f"0x{declick_hook.vma + last:04X}",
                "extension_entry": f"0x{entry_address:04X}",
                "stock_continuation": f"0x{declick_hook.vma + len(new_words):04X}",
                "stock_precondition_words": [f"{w:06X}" for w in declick_hook.stock_words],
                "stock_precondition_bytes": b"".join(map(u24, declick_hook.stock_words)).hex().upper(),
                "replacement_words": [f"{w:06X}" for w in new_words],
                "replacement_bytes": b"".join(map(u24, new_words)).hex().upper(),
            }
        )
    if args.granular_engine:
        word = SHORT_JSR | engine_symbols["ge_hook"]
        replacements.append(
            {
                "name": "granular_engine_voice_pass",
                "raw_lma_start": f"0x{engine_hook_raw:04X}",
                "raw_lma_end_inclusive": f"0x{engine_hook_raw:04X}",
                "runtime_vma_start": f"0x{ENGINE_HOOK_VMA:04X}",
                "runtime_vma_end_inclusive": f"0x{ENGINE_HOOK_VMA:04X}",
                "extension_entry": f"0x{engine_symbols['ge_hook']:04X}",
                "stock_continuation": f"0x{ENGINE_HOOK_VMA + 1:04X}",
                "stock_precondition_words": [f"{ENGINE_HOOK_STOCK:06X}"],
                "stock_precondition_bytes": u24(ENGINE_HOOK_STOCK).hex().upper(),
                "replacement_words": [f"{word:06X}"],
                "replacement_bytes": u24(word).hex().upper(),
            }
        )
    package_words = [
        int.from_bytes(package[index : index + 3], "big")
        for index in range(0, len(package), 3)
    ]
    stream = primary_words[:terminator_index] + package_words
    stream_records, stream_end = parse_loader_records(stream)
    if stream_end != len(stream) - 1 or stream[stream_end] != 0x000100:
        raise ValueError("patched primary-B loader stream does not end at its terminator")

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
            "package_bytes": len(package),
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
            "package_stream_records": len(stream_records) - len(primary_records),
        },
        "granular_probe": (
            {
                "base": f"0x{PROBE_BASE:04X}",
                "word_count": len(probe_payload) // 3,
                "end_address_inclusive": f"0x{PROBE_BASE + len(probe_payload) // 3 - 1:04X}",
                "telemetry": "Y:$37F00..$37F08 (shared)",
            }
            if args.granular_probe
            else None
        ),
        "looper_declick": (
            {
                "base": f"0x{DECLICK_BASE:04X}",
                "word_count": len(declick_payload) // 3,
                "end_address_inclusive": (
                    f"0x{DECLICK_BASE + len(declick_payload) // 3 - 1:04X}"
                ),
                "opt_in": "Y:(param block + 31) != 0",
                "scratch": "X:$30..$33",
            }
            if args.looper_declick
            else None
        ),
        "granular_engine": (
            {
                "hook_base": f"0x{ENGINE_INTERNAL_BASE:04X}",
                "hook_word_count": len(engine_internal) // 3,
                "engine_master_y": f"0x{ENGINE_MASTER_Y:04X}",
                "engine_run_address": f"0x{ENGINE_SHARED_BASE:06X}",
                "engine_word_count": len(engine_shared) // 3,
                "params": "Y:(param block + 26..30)",
                "state": "X:$2B00..$2E1F, X:$34..$35",
                "telemetry": "Y:$37F10..$37F13 (shared)",
            }
            if args.granular_engine
            else None
        ),
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
