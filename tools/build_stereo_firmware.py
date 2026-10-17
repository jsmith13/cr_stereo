#!/usr/bin/env python3
"""Build an offline Circuit Rhythm stereo-capture POC firmware image."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
from pathlib import Path

from extract_firmware import extract
from pack_firmware import repack


IMAGE_BASE = 0x08010000
EXPECTED_SIZE = 0x5A570
EXPECTED_SHA256 = "d5b9ff5dadf2e284f80d69ecb7087486cf0df315a697bed8272af9de0e90df15"
PAYLOAD_ADDRESS = IMAGE_BASE + EXPECTED_SIZE
SECTOR_5_START = 0x08040000
SECTOR_6_START = 0x08080000
# The 381,168-byte figure was adopted while size was still suspected of causing
# the native candidate rejections. That suspicion was refuted: candidate 4 at
# 380,326 bytes was rejected and candidate 5 at 380,328 bytes was accepted, the
# two differing only by tail padding that made the image word aligned. The real
# invariant is the four-byte alignment enforced above. The largest image the
# bootloader has actually accepted is 381,460 bytes, so that is the proven
# ceiling. A rejected image is harmless - the bootloader refuses it and the
# unit keeps its current firmware.
PROVEN_NATIVE_BOOTLOADER_MAX_IMAGE_SIZE = 381460
STOCK_APPLICATION_BUILD = 5706
BUILD_NUMBER_ADDRESS = 0x080101CA
BUILD_STRING_ADDRESSES = (0x080101D9, 0x08025A5B)

# Multi-instruction stock preconditions are pinned by length and SHA-256 rather
# than quoted, so this repository carries no stock code sequences.
DSP_RANGES = {
    range(0x32D9, 0x32F0): (
        0x08019A81,
        (69, "29141f5390dbf6227f76eee73869d6cce862bf914f290177e3d4d0b10a04b52e"),
    ),
    range(0x330A, 0x3315): (
        0x08019B14,
        (33, "25ccf97c7cf3cf3801bb7ff453d35bcc9ee629255efdfd3160e967d90d8a0401"),
    ),
}

BASE_HOOKS = {
    0x0802F354: ("stereo_start_request", bytes.fromhex("2de9f041")),
    0x0802F450: ("stereo_pump", bytes.fromhex("feb50746")),
    0x0802F5CC: ("stereo_file_callback", bytes.fromhex("10b5039c")),
}
WRITER_REQUEST_PROBE_HOOK = {
    0x0802F354: ("writer_request_probe_entry", bytes.fromhex("2de9f041")),
}
FINAL_WRITE_TONE_HOOK = {
    0x0802E036: ("diagnostic_fatfs_write", bytes.fromhex("f4f761f8")),
}
SAFE_PAIR_GUARD_HOOK = {
    0x08041388: ("stereo_prepare_destination", bytes.fromhex("70b50446")),
}
SAMPLE_PAIR_LED_VTABLE_HOOK = {
    0x08067A24: ("sample_pair_render", bytes.fromhex("9bfe0408")),
}
SAMPLE_PAIR_IDENTITY_CALL_HOOK = {
    0x0802F1C4: ("writer_state_constructor", bytes.fromhex("16f046fe")),
    0x08039C7C: ("sample_pair_pack_load", bytes.fromhex("0cf014fa")),
    0x0806954C: ("sample_pair_direct_pack_load", bytes.fromhex("eaf76afc")),
    0x08053ED0: ("sample_pair_wav_validate", bytes.fromhex("07f0f0f9")),
    0x0803A1A4: ("sample_pair_clear_request", bytes.fromhex("0df0c5f8")),
    0x0803504A: ("sample_pair_save_publish", bytes.fromhex("fcf79afb")),
}
MAIN_LOOP_TICK_CALL_HOOK = {
    0x0802587C: ("stereo_main_loop_tick", bytes.fromhex("01f0d2fe")),
}
# Tail of the stock `FUN_0803a04a` epilogue -- the sole caller of the Sample
# Clear entry `0x0803A10E`, reached with r0 = UI and r1 = slot already loaded.
# Replacing the `pop.w {r2, r4, r5, lr}` (the shim replays it) puts the stereo
# pair canonicalisation ahead of the entry's occupancy check, which otherwise
# silently drops a Clear aimed at the right member of a pair.
SAMPLE_PAIR_CLEAR_ENTRY_HOOK = {
    0x0803A074: ("sample_pair_clear_entry", bytes.fromhex("bde83440")),
}
STEREO_SAVE_QUEUE_CALL_HOOK = {
    0x0803520E: ("stereo_save_request", bytes.fromhex("fff722ff")),
}
# Prologue of FUN_08045EB6, the common core of FUN_0804603A (op 6, sample-file
# read) and FUN_0804607A (op 7, write). Hooking here observes every sample-file
# transfer in one place, which is what the crop mirror replays.
CROP_IO_TRACE_HOOK = {
    0x08045EB6: ("crop_io_trace_entry", bytes.fromhex("2de9f04f")),
}
# The capture hardware's loop-window setters, reached only through vtable
# 0x08066E78. They write the window registers (0x371D8 start, 0x371D9 end) and
# are the strongest candidate for where a crop is applied to a slot.
CROP_WINDOW_VTABLE_HOOK = {
    0x08066E84: ("crop_window_set", bytes.fromhex("e5440608")),
    0x08066E88: ("crop_window_set_wait", bytes.fromhex("13450608")),
}
STEREO_STOCK_HEADER_CALL_HOOK = {
    0x0802F5C2: ("stereo_stock_header_write", bytes.fromhex("16f05afd")),
}
# Observer-interface slot of the sample-occupancy model. The model (ctor
# 0x08039EBC, vtable base 0x08067568) uses multiple inheritance: its primary
# vtable is base+8 (0x08067570) but it registers `this + 4` as the observer, so
# notifications dispatch through base+0x20 (0x08067588) into the adjustor thunk
# 0x08039FEC (`subs r0, r0, #4`) which falls through into 0x08039FEE. Patching
# the primary vtable therefore has no effect on the notification path; this is
# the slot that actually runs.
STEREO_STOCK_OCCUPANCY_VTABLE_HOOK = {
    0x08067588: ("sample_pair_occupancy_notify", bytes.fromhex("ed9f0308")),
}
STEREO_STOCK_SOURCE_READ_VTABLE_HOOK = {
    0x08066E6C: ("native_stock_capture_chunk_read", bytes.fromhex("4b440608")),
    0x08066E70: ("native_stock_capture_source_read", bytes.fromhex("dd740608")),
}
SAMPLE_PAIR_GESTURE_VTABLE_HOOKS = {
    0x08067A08: ("sample_pair_press", bytes.fromhex("d9f90408")),
    0x08067A0C: ("sample_pair_release", bytes.fromhex("27fb0408")),
    0x08067A10: ("sample_pair_page_event", bytes.fromhex("35fd0408")),
}
SAMPLE_PAIR_GESTURE_BRANCH_HOOK = {
    0x0804F9D0: ("sample_pair_view_init_epilogue", bytes.fromhex("204603b0")),
}
PAIRED_PLAYBACK_VTABLE_HOOKS = {
    0x080685E0: ("paired_playback_note_on", bytes.fromhex("191a0308")),
    0x080685E4: ("paired_playback_note_off", bytes.fromhex("231a0308")),
}
PAIRED_PLAYBACK_ALLOCATION_HOOK = {
    0x0802F1B2: (
        "writer_state_prepare_allocation",
        bytes.fromhex("7420a160"),
    ),
}
STEREO_POSITION_VTABLE_HOOKS = {
    0x08069084: ("stereo_position_observable_set", bytes.fromhex("dd650408")),
    0x08067918: ("stereo_position_mixer_press", bytes.fromhex("390c0508")),
    0x08067934: ("stereo_position_grid_render", bytes.fromhex("db0e0508")),
    0x0806811C: ("fx_send_mixer_knob", bytes.fromhex("6b8c0508")),
    0x080679AC: ("fx_send_pair_render", bytes.fromhex("e3f70408")),
    0x08067F14: ("sidechain_pair_press", bytes.fromhex("4b570508")),
    0x08067F30: ("sidechain_pair_render", bytes.fromhex("b1570508")),
    # UI root button press (0x0803D8B8): hold a track button, press a neighbour.
    0x080672EC: ("track_pair_button_press", bytes.fromhex("b9d80308")),
    # UI root button release (0x0803DDB0): undo a follower->leader redirect.
    0x080672F0: ("track_pair_button_release", bytes.fromhex("b1dd0308")),
}
STEREO_POSITION_KNOB_VTABLE_HOOK = {
    0x0806834C: ("stereo_position_mixer_knob", bytes.fromhex("f9f20508")),
}
# Shared pair colour. `bl main` in the C runtime start-up and the Main task's
# entry pointer (created by FUN_08026090) both initialise the RAM colour table
# before any colour lookup can run.
PAIR_COLOR_CALL_HOOK = {
    0x08068E34: ("pair_color_boot_main", bytes.fromhex("bcf728fc")),
}
PAIR_COLOR_POINTER_HOOK = {
    0x0802614C: ("pair_color_main_task", bytes.fromhex("71570208")),
    # Track-button LED layer observer handler (FUN_0804BF28 object).
    0x08067218: ("pair_track_buttons_notify", bytes.fromhex("dfc20408")),
    0x08067228: ("pair_track_buttons_notify_thunk", bytes.fromhex("ddc20408")),
    # Page group cell getter (FUN_0804DD1C): recolour the Sample page bank.
    0x080678F8: ("sample_bank_compose", bytes.fromhex("1ddd0408")),
}
# Every stock literal naming the colour-enum table (or an entry inside it) is
# redirected to the payload's RAM copy at the same offset.
PAIR_COLOR_TABLE_FLASH = 0x08062AB0
PAIR_COLOR_TABLE_BYTES = 568 * 2
PAIR_COLOR_TABLE_RAM = 0x2004F760
PAIR_COLOR_TABLE_LITERALS = 53
STEREO_POSITION_KNOB_BRANCH_HOOK = {
    0x0805A2C6: ("stereo_position_mixer_knob", bytes.fromhex("05f017b8")),
}
# The probe build substitutes only the volume-knob entry. The FX and sidechain
# coupling hooks are part of the shipping stereo-position feature and live in
# STEREO_POSITION_VTABLE_HOOKS above.
MIXER_VOLUME_PROBE_VTABLE_HOOK = {
    0x0806834C: ("stereo_position_mixer_knob", bytes.fromhex("f9f20508")),
}
MIXER_VOLUME_PROBE_BRANCH_HOOK = {
    0x0805A2C6: ("stereo_position_mixer_knob", bytes.fromhex("05f017b8")),
}
# Blank-project track template (FUN_08032FC2 copies it into all eight 41-byte
# track blocks). Byte 26 is track parameter 16, the mixer volume; stock is 100.
# The window covers bytes 24..27 (.., .., volume, pan=64) for a unique match.
DEFAULT_TRACK_VOLUME_PATCH = (
    0x08068808,
    bytes.fromhex("00006440"),
    bytes.fromhex("00005940"),
)
STEREO_POSITION_CALL_HOOK = {
    0x0803085E: ("stereo_position_dsp_update", bytes.fromhex("0ff056ff")),
}
SAMPLE_PAIR_WRITER_COUPLING_MARKER = "sample_pair_writer_coupling_marker"
SAMPLE_CACHE_REFRESH_MARKER = "sample_cache_refresh_marker"
PAIRED_PLAYBACK_MARKER = "paired_playback_marker"
STEREO_POSITION_MARKER = "stereo_position_marker"
NATIVE_DSP_EXTENSION_POINTER = "native_dsp_extension_pointer"
NATIVE_DSP_CONTINUATION_BYTES = 1134
FIRMWARE_IMAGE_ALIGNMENT = 4
NATIVE_CAPTURE_VTABLE_HOOK = {
    0x08066E80: ("native_capture_source_start", bytes.fromhex("6f440608")),
}
NATIVE_DSP_UPLOAD_CALL_HOOK = {
    0x08031170: ("native_dsp_upload_primary_b", bytes.fromhex("fff79eff")),
}
SAMPLE_COLOR_ASSERTIONS = {
    0x0804FCD8: bytes.fromhex("b02a0608"),
    0x0805985C: (12, "54bde5cabb37c838c3b5f678ecb9310af62d408beb766200e0c9c67f42b2a594"),
    0x08062D0A: bytes.fromhex("2200"),
    0x08062EC8: bytes.fromhex("a100a20055005600"),
    0x08062ED2: bytes.fromhex("5c00"),
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def image_offset(address: int) -> int:
    if address < IMAGE_BASE:
        raise ValueError(f"Address {address:#010x} precedes the application image")
    return address - IMAGE_BASE


def hashed_stock_bytes(
    image: bytearray, address: int, fingerprint: tuple[int, str]
) -> bytes:
    length, digest = fingerprint
    actual = bytes(image[image_offset(address) : image_offset(address) + length])
    if sha256(actual) != digest:
        raise ValueError(
            f"Precondition failed at {address:#010x}: "
            f"{length}-byte stock SHA-256 mismatch"
        )
    return actual


def require_bytes(
    image: bytearray, address: int, expected: bytes | tuple[int, str]
) -> None:
    if isinstance(expected, tuple):
        hashed_stock_bytes(image, address, expected)
        return
    actual = bytes(image[image_offset(address) : image_offset(address) + len(expected)])
    if actual != expected:
        raise ValueError(
            f"Precondition failed at {address:#010x}: "
            f"expected {expected.hex()}, found {actual.hex()}"
        )


def patch_bytes(
    image: bytearray,
    manifest: list[dict[str, object]],
    address: int,
    expected: bytes,
    replacement: bytes,
    purpose: str,
) -> None:
    if len(expected) != len(replacement):
        raise ValueError(f"In-place patch at {address:#010x} changes length")
    require_bytes(image, address, expected)
    offset = image_offset(address)
    image[offset : offset + len(replacement)] = replacement
    manifest.append(
        {
            "address": f"0x{address:08X}",
            "length": len(replacement),
            "purpose": purpose,
            "before": expected.hex(),
            "after": replacement.hex(),
        }
    )


def parse_lod(path: Path) -> dict[int, bytes]:
    words: dict[int, bytes] = {}
    for line_number, line in enumerate(path.read_text().splitlines(), 1):
        fields = line.split()
        if not fields or fields[0] == "I":
            continue
        if len(fields) != 3 or fields[0] != "P":
            raise ValueError(f"{path}:{line_number}: unsupported LOD line")
        address = int(fields[1], 16)
        value = int(fields[2], 16)
        if address in words or value >= 1 << 24:
            raise ValueError(f"{path}:{line_number}: invalid DSP word")
        words[address] = value.to_bytes(3, "big")
    return words


def parse_map_symbols(path: Path, hooks: dict[int, tuple[str, bytes]]) -> dict[str, int]:
    wanted = set(name for name, _ in hooks.values())
    symbols: dict[str, int] = {}
    pattern = map_symbol_pattern()
    for line in path.read_text().splitlines():
        match = pattern.match(line)
        if match and match.group(2) in wanted:
            symbols[match.group(2)] = int(match.group(1), 16) & ~1
    missing = wanted - symbols.keys()
    if missing:
        raise ValueError(f"Payload map is missing symbols: {', '.join(sorted(missing))}")
    return symbols


def map_symbol_pattern() -> re.Pattern[str]:
    return re.compile(
        r"^\s*([0-9a-fA-F]+)\s+[0-9a-fA-F]+\s+[0-9a-fA-F]+\s+\d+\s+(\S+)\s*$"
    )


def find_map_symbol(path: Path, wanted: str) -> int | None:
    pattern = map_symbol_pattern()
    for line in path.read_text().splitlines():
        match = pattern.match(line)
        if match and match.group(2) == wanted:
            return int(match.group(1), 16) & ~1
    return None


def encode_thumb_bw(source: int, target: int) -> bytes:
    displacement = target - (source + 4)
    if displacement & 1 or not -(1 << 24) <= displacement < 1 << 24:
        raise ValueError(
            f"Thumb B.W from {source:#010x} cannot reach {target:#010x}"
        )
    encoded = displacement & 0x01FF_FFFE
    sign = (encoded >> 24) & 1
    i1 = (encoded >> 23) & 1
    i2 = (encoded >> 22) & 1
    j1 = 1 ^ i1 ^ sign
    j2 = 1 ^ i2 ^ sign
    first = 0xF000 | (sign << 10) | ((encoded >> 12) & 0x3FF)
    second = 0x9000 | (j1 << 13) | (j2 << 11) | ((encoded >> 1) & 0x7FF)
    return struct.pack("<HH", first, second)


def encode_thumb_bl(source: int, target: int) -> bytes:
    first, second = struct.unpack("<HH", encode_thumb_bw(source, target))
    return struct.pack("<HH", first, (second & 0x2FFF) | 0xD000)


def decode_thumb_bw(source: int, instruction: bytes) -> int:
    first, second = struct.unpack("<HH", instruction)
    sign = (first >> 10) & 1
    j1 = (second >> 13) & 1
    j2 = (second >> 11) & 1
    i1 = 1 ^ j1 ^ sign
    i2 = 1 ^ j2 ^ sign
    encoded = (
        (sign << 24)
        | (i1 << 23)
        | (i2 << 22)
        | ((first & 0x3FF) << 12)
        | ((second & 0x7FF) << 1)
    )
    if sign:
        encoded -= 1 << 25
    return source + 4 + encoded


def require_thumb_literal(
    payload: bytes,
    symbol_address: int,
    register: int,
    expected: int,
) -> None:
    start = symbol_address - PAYLOAD_ADDRESS
    end = min(start + 0x40, len(payload))
    for offset in range(start, end - 1, 2):
        instruction = struct.unpack_from("<H", payload, offset)[0]
        if instruction & 0xF800 != 0x4800 or instruction >> 8 & 7 != register:
            continue
        instruction_address = PAYLOAD_ADDRESS + offset
        literal_address = (
            (instruction_address + 4) & ~3
        ) + (instruction & 0xFF) * 4
        literal_offset = literal_address - PAYLOAD_ADDRESS
        if 0 <= literal_offset <= len(payload) - 4:
            actual = struct.unpack_from("<I", payload, literal_offset)[0]
            if actual == expected:
                return
    raise ValueError(
        f"Thumb literal in symbol at {symbol_address:#010x} does not resolve "
        f"r{register} to {expected:#010x}"
    )


def apply_patches(
    stock: bytes,
    dsp_lod: Path,
    payload: bytes,
    payload_map: Path,
    application_build: int,
    stock_dsp: bool,
    final_write_tone: bool,
    safe_pair_guard: bool,
    sample_pair_led_poc: bool,
    sample_pair_identity_poc: bool,
    sample_pair_gesture_poc: bool,
    sample_pair_writer_coupling_poc: bool,
    sample_cache_refresh_poc: bool,
    paired_playback_poc: bool,
    stereo_position_poc: bool,
    mixer_volume_probe: bool,
    writer_request_probe_diagnostic: bool,
    native_dual_capture_poc: bool,
    native_capture_preflight_probe_diagnostic: bool,
    native_writer_readiness_probe_diagnostic: bool,
    native_writer_acquisition_probe_diagnostic: bool,
    native_arm_token_probe_diagnostic: bool,
    native_pack_value_probe_diagnostic: bool,
    native_pack_correction_probe_diagnostic: bool,
    native_finalization_probe_diagnostic: bool,
    native_publication_probe_diagnostic: bool,
    native_terminal_publish_probe_diagnostic: bool,
    native_publish_boundary_probe_diagnostic: bool,
    native_publish_slot_probe_diagnostic: bool,
    native_deferred_refresh_probe_diagnostic: bool,
    stereo_stock_save_queue: bool,
    crop_io_trace: bool,
    crop_mirror_window: bool,
    native_dsp_package: bytes | None,
    native_dsp_metadata: dict[str, object] | None,
) -> tuple[bytes, dict[str, object]]:
    if len(stock) != EXPECTED_SIZE or sha256(stock) != EXPECTED_SHA256:
        raise ValueError("Input is not the pinned Circuit Rhythm build 5706 image")
    if not payload:
        raise ValueError("Thumb payload is empty")
    if sample_pair_identity_poc and not (safe_pair_guard and sample_pair_led_poc):
        raise ValueError(
            "Pair identity requires both the safe-pair guard and pair LED POC"
        )
    if sample_pair_identity_poc and not sample_pair_writer_coupling_poc:
        raise ValueError("Pair identity requires pair writer coupling")
    if sample_pair_identity_poc and not sample_cache_refresh_poc:
        raise ValueError("Pair identity requires sample cache refresh")
    if sample_pair_gesture_poc and not sample_pair_led_poc:
        raise ValueError("Pair gesture requires the pair LED POC")
    if sample_pair_writer_coupling_poc and not (
        sample_pair_gesture_poc
        and sample_pair_identity_poc
        and safe_pair_guard
    ):
        raise ValueError(
            "Pair writer coupling requires gesture, identity, and safe-pair POCs"
        )
    if sample_cache_refresh_poc and not sample_pair_writer_coupling_poc:
        raise ValueError("Sample cache refresh requires pair writer coupling")
    if paired_playback_poc and not (
        sample_pair_led_poc
        and sample_pair_identity_poc
        and sample_pair_writer_coupling_poc
    ):
        raise ValueError("Paired playback requires LED, identity, and writer coupling")
    if stereo_position_poc and not paired_playback_poc:
        raise ValueError("Stereo position requires paired playback")
    if mixer_volume_probe and not stereo_position_poc:
        raise ValueError("Mixer volume probe requires stereo position")
    if stereo_stock_save_queue and not (
        native_dual_capture_poc
        and sample_pair_gesture_poc
        and sample_pair_writer_coupling_poc
        and sample_cache_refresh_poc
    ):
        raise ValueError(
            "Stock save queue requires native dual capture, the pair gesture, "
            "and pair writer coupling for the arm token"
        )
    if native_dual_capture_poc and not (
        safe_pair_guard
        and sample_pair_identity_poc
        and sample_pair_writer_coupling_poc
        and sample_cache_refresh_poc
    ):
        raise ValueError(
            "Native dual capture requires safe-pair, identity, writer-coupling, "
            "and cache POCs"
        )
    if native_dual_capture_poc and stock_dsp:
        raise ValueError("Native dual capture cannot use --stock-dsp")
    if native_capture_preflight_probe_diagnostic and not (
        native_dual_capture_poc and sample_pair_led_poc
    ):
        raise ValueError(
            "Native capture preflight probe requires native dual capture and pair LEDs"
        )
    if native_writer_readiness_probe_diagnostic and not (
        native_dual_capture_poc and sample_pair_led_poc
    ):
        raise ValueError(
            "Native writer readiness probe requires native dual capture and pair LEDs"
        )
    if native_writer_acquisition_probe_diagnostic and not (
        native_dual_capture_poc and sample_pair_led_poc
    ):
        raise ValueError(
            "Native writer acquisition probe requires native dual capture and pair LEDs"
        )
    if native_arm_token_probe_diagnostic and not (
        native_dual_capture_poc
        and sample_pair_led_poc
        and sample_pair_writer_coupling_poc
    ):
        raise ValueError(
            "Native arm token probe requires native dual capture, pair LEDs, "
            "and writer coupling"
        )
    if native_pack_value_probe_diagnostic and not (
        native_dual_capture_poc
        and sample_pair_led_poc
        and sample_pair_writer_coupling_poc
    ):
        raise ValueError(
            "Native pack value probe requires native dual capture, pair LEDs, "
            "and writer coupling"
        )
    if native_pack_correction_probe_diagnostic and not (
        native_dual_capture_poc
        and sample_pair_led_poc
        and sample_pair_writer_coupling_poc
    ):
        raise ValueError(
            "Native pack correction probe requires native dual capture, pair LEDs, "
            "and writer coupling"
        )
    if native_finalization_probe_diagnostic and not (
        native_dual_capture_poc
        and sample_pair_led_poc
        and sample_pair_writer_coupling_poc
    ):
        raise ValueError(
            "Native finalization probe requires native dual capture, pair LEDs, "
            "and writer coupling"
        )
    if native_publication_probe_diagnostic and not (
        native_dual_capture_poc
        and sample_pair_led_poc
        and sample_pair_writer_coupling_poc
        and sample_cache_refresh_poc
    ):
        raise ValueError(
            "Native publication probe requires native dual capture, pair LEDs, "
            "writer coupling, and cache refresh"
        )
    if native_terminal_publish_probe_diagnostic and not (
        native_dual_capture_poc
        and sample_pair_led_poc
        and sample_pair_writer_coupling_poc
        and sample_cache_refresh_poc
    ):
        raise ValueError(
            "Native terminal publication probe requires native dual capture, "
            "pair LEDs, writer coupling, and cache refresh"
        )
    if native_publish_boundary_probe_diagnostic and not (
        native_dual_capture_poc
        and sample_pair_led_poc
        and sample_pair_writer_coupling_poc
        and sample_cache_refresh_poc
    ):
        raise ValueError(
            "Native publication boundary probe requires native dual capture, "
            "pair LEDs, writer coupling, and cache refresh"
        )
    if native_publish_slot_probe_diagnostic and not (
        native_dual_capture_poc
        and sample_pair_led_poc
        and sample_pair_writer_coupling_poc
        and sample_cache_refresh_poc
    ):
        raise ValueError(
            "Native publication slot probe requires native dual capture, "
            "pair LEDs, writer coupling, and cache refresh"
        )
    if native_deferred_refresh_probe_diagnostic and not (
        native_dual_capture_poc
        and sample_pair_led_poc
        and sample_pair_identity_poc
        and sample_pair_gesture_poc
        and sample_pair_writer_coupling_poc
        and sample_cache_refresh_poc
    ):
        raise ValueError(
            "Native deferred refresh probe requires native dual capture and "
            "the complete sample-pair stack"
        )
    if sum(
        (
            native_capture_preflight_probe_diagnostic,
            native_writer_readiness_probe_diagnostic,
            native_writer_acquisition_probe_diagnostic,
            native_arm_token_probe_diagnostic,
            native_pack_value_probe_diagnostic,
            native_pack_correction_probe_diagnostic,
            native_finalization_probe_diagnostic,
            native_publication_probe_diagnostic,
            native_terminal_publish_probe_diagnostic,
            native_publish_boundary_probe_diagnostic,
            native_publish_slot_probe_diagnostic,
            native_deferred_refresh_probe_diagnostic,
        )
    ) > 1:
        raise ValueError("Native writer diagnostics are mutually exclusive")
    if native_dual_capture_poc and (
        native_dsp_package is None or native_dsp_metadata is None
    ):
        raise ValueError("Native dual capture requires its DSP package and metadata")

    payload = bytearray(payload)
    arm_payload_end = PAYLOAD_ADDRESS + len(payload)
    extension_address = (arm_payload_end + 3) & ~3
    extension_padding = extension_address - arm_payload_end
    if native_dual_capture_poc:
        assert native_dsp_package is not None
        if len(native_dsp_package) != NATIVE_DSP_CONTINUATION_BYTES:
            raise ValueError(
                "Native DSP continuation size changed; update the compact upload wrapper"
            )
        payload_pointer = find_map_symbol(payload_map, NATIVE_DSP_EXTENSION_POINTER)
        if payload_pointer is None:
            raise ValueError(
                "Payload map is missing the native DSP extension pointer"
            )
        pointer_offset = payload_pointer - PAYLOAD_ADDRESS
        if not 0 <= pointer_offset <= len(payload) - 4:
            raise ValueError("Native DSP pointer lies outside the ARM payload")
        if payload[pointer_offset : pointer_offset + 4] != bytes(4):
            raise ValueError("Native DSP pointer precondition is not zero")
        payload[pointer_offset : pointer_offset + 4] = struct.pack(
            "<I", extension_address
        )
        appended = bytes(payload) + bytes(extension_padding) + native_dsp_package
        image_tail_padding = (
            -(PAYLOAD_ADDRESS + len(appended) - IMAGE_BASE)
        ) % FIRMWARE_IMAGE_ALIGNMENT
        appended += bytes(image_tail_padding)
    else:
        extension_address = 0
        extension_padding = 0
        image_tail_padding = 0
        appended = bytes(payload)
    payload_end = PAYLOAD_ADDRESS + len(appended)
    if PAYLOAD_ADDRESS & 3:
        raise ValueError("Thumb payload address is not four-byte aligned")
    if (payload_end - IMAGE_BASE) % FIRMWARE_IMAGE_ALIGNMENT:
        raise ValueError("Patched firmware image is not word aligned")
    if not SECTOR_5_START <= PAYLOAD_ADDRESS < payload_end <= SECTOR_6_START:
        raise ValueError(
            "Appended payload would leave the stock image's existing flash sector: "
            f"{PAYLOAD_ADDRESS:#010x}..{payload_end:#010x}, "
            f"sector end {SECTOR_6_START:#010x}"
        )
    if payload_end - IMAGE_BASE > PROVEN_NATIVE_BOOTLOADER_MAX_IMAGE_SIZE:
        # Not an error. Every bootloader rejection on record is explained by the
        # four-byte alignment checked above: 381,897 (mod 4 = 1), 381,282 (mod 4
        # = 2) and 380,326 (mod 4 = 2) were all refused, while 380,328, 381,168
        # and 381,460 were all accepted. No word-aligned image has ever been
        # rejected, so there is no evidence of a size limit below the flash
        # sector bound enforced above. Sizes past the largest ever accepted are
        # nonetheless unproven, so say so rather than failing or staying silent.
        print(
            "Warning: image is "
            f"{payload_end - IMAGE_BASE} bytes, past the largest size the "
            f"bootloader is known to have accepted "
            f"({PROVEN_NATIVE_BOOTLOADER_MAX_IMAGE_SIZE}). It is word aligned "
            "and within the flash sector, and no aligned image has ever been "
            "refused, but this size is untested. A refused image is harmless: "
            "the unit keeps the firmware it already has."
        )

    image = bytearray(stock)
    changes: list[dict[str, object]] = []
    if not 0 <= application_build <= 9999:
        raise ValueError("Application build must be a four-digit decimal value")
    if application_build != STOCK_APPLICATION_BUILD:
        patch_bytes(
            image,
            changes,
            BUILD_NUMBER_ADDRESS,
            struct.pack("<H", STOCK_APPLICATION_BUILD),
            struct.pack("<H", application_build),
            "Lower application build for custom-image activation",
        )
        expected_text = str(STOCK_APPLICATION_BUILD).encode("ascii")
        replacement_text = f"{application_build:04d}".encode("ascii")
        for address in BUILD_STRING_ADDRESSES:
            patch_bytes(
                image,
                changes,
                address,
                expected_text,
                replacement_text,
                "Keep application build string consistent",
            )

    if native_dual_capture_poc:
        assert native_dsp_metadata is not None
        if not native_dsp_metadata.get("safe_for_integrated_firmware", False):
            raise ValueError("Native DSP metadata is not marked safe for integration")
        if native_dsp_metadata.get("schema") != (
            "circuit-rhythm-dual-native-mono-dsp-patch-v1"
        ):
            raise ValueError("Unsupported native DSP patch metadata schema")
        for replacement in native_dsp_metadata["replacements"]:
            raw = int(replacement["raw_lma_start"], 0)
            address = 0x080101F0 + 6 + raw * 3
            patch_bytes(
                image,
                changes,
                address,
                bytes.fromhex(replacement["stock_precondition_bytes"]),
                bytes.fromhex(replacement["replacement_bytes"]),
                f"Native DSP overlay hook {replacement['name']}",
            )
    elif not stock_dsp:
        dsp_words = parse_lod(dsp_lod)
        expected_dsp_addresses = {
            address for addresses in DSP_RANGES for address in addresses
        }
        if dsp_words.keys() != expected_dsp_addresses:
            missing = expected_dsp_addresses - dsp_words.keys()
            extra = dsp_words.keys() - expected_dsp_addresses
            raise ValueError(
                f"DSP LOD address mismatch; missing={sorted(missing)}, extra={sorted(extra)}"
            )

        for addresses, (firmware_address, fingerprint) in DSP_RANGES.items():
            expected = hashed_stock_bytes(image, firmware_address, fingerprint)
            replacement = b"".join(dsp_words[address] for address in addresses)
            patch_bytes(
                image,
                changes,
                firmware_address,
                expected,
                replacement,
                f"DSP stereo capture P:${addresses.start:04X}..P:${addresses.stop - 1:04X}",
            )

    if not sample_pair_identity_poc:
        patch_bytes(
            image,
            changes,
            0x0802F1B2,
            bytes.fromhex("7420"),
            bytes.fromhex(
                "fe20" if sample_pair_writer_coupling_poc else "8820"
            ),
            (
                "Grow writer state allocation from 0x74 to 0xFE bytes"
                if sample_pair_writer_coupling_poc
                else "Grow writer state allocation from 0x74 to 0x88 bytes"
            ),
        )

    hooks = dict(BASE_HOOKS)
    if writer_request_probe_diagnostic:
        hooks.update(WRITER_REQUEST_PROBE_HOOK)
    if final_write_tone:
        hooks.update(FINAL_WRITE_TONE_HOOK)
    if safe_pair_guard:
        hooks.update(SAFE_PAIR_GUARD_HOOK)
    if native_dual_capture_poc:
        hooks.update(NATIVE_DSP_UPLOAD_CALL_HOOK)
    if sample_pair_identity_poc:
        hooks.update(SAMPLE_PAIR_CLEAR_ENTRY_HOOK)
    if crop_io_trace:
        hooks.update(CROP_IO_TRACE_HOOK)
    symbol_hooks = dict(hooks)
    if sample_pair_led_poc:
        symbol_hooks.update(SAMPLE_PAIR_LED_VTABLE_HOOK)
    if sample_pair_identity_poc:
        symbol_hooks.update(SAMPLE_PAIR_IDENTITY_CALL_HOOK)
    if sample_pair_gesture_poc:
        symbol_hooks.update(SAMPLE_PAIR_GESTURE_VTABLE_HOOKS)
        symbol_hooks.update(SAMPLE_PAIR_GESTURE_BRANCH_HOOK)
    if paired_playback_poc:
        symbol_hooks.update(PAIRED_PLAYBACK_VTABLE_HOOKS)
    if sample_pair_identity_poc:
        symbol_hooks.update(PAIRED_PLAYBACK_ALLOCATION_HOOK)
    if stereo_position_poc:
        symbol_hooks.update(STEREO_POSITION_VTABLE_HOOKS)
        symbol_hooks.update(
            MIXER_VOLUME_PROBE_VTABLE_HOOK
            if mixer_volume_probe
            else STEREO_POSITION_KNOB_VTABLE_HOOK
        )
        symbol_hooks.update(STEREO_POSITION_CALL_HOOK)
        symbol_hooks.update(PAIR_COLOR_CALL_HOOK)
        symbol_hooks.update(PAIR_COLOR_POINTER_HOOK)
    if mixer_volume_probe:
        symbol_hooks.update(MIXER_VOLUME_PROBE_BRANCH_HOOK)
    elif stereo_position_poc:
        symbol_hooks.update(STEREO_POSITION_KNOB_BRANCH_HOOK)
    if native_dual_capture_poc:
        symbol_hooks.update(NATIVE_CAPTURE_VTABLE_HOOK)
    if (crop_io_trace or crop_mirror_window) and native_dual_capture_poc:
        symbol_hooks.update(CROP_WINDOW_VTABLE_HOOK)
    main_loop_tick_hook = (
        sample_cache_refresh_poc
        and sample_pair_identity_poc
        and native_dual_capture_poc
        and not native_deferred_refresh_probe_diagnostic
    )
    if main_loop_tick_hook:
        symbol_hooks.update(MAIN_LOOP_TICK_CALL_HOOK)
    if stereo_stock_save_queue:
        symbol_hooks.update(STEREO_SAVE_QUEUE_CALL_HOOK)
        symbol_hooks.update(STEREO_STOCK_HEADER_CALL_HOOK)
        symbol_hooks.update(STEREO_STOCK_SOURCE_READ_VTABLE_HOOK)
        symbol_hooks.update(STEREO_STOCK_OCCUPANCY_VTABLE_HOOK)
    symbols = parse_map_symbols(payload_map, symbol_hooks)
    if sample_pair_gesture_poc:
        require_thumb_literal(
            payload,
            symbols["sample_pair_press"],
            register=4,
            expected=0x0804F9D9,
        )
    coupling_marker = find_map_symbol(
        payload_map, SAMPLE_PAIR_WRITER_COUPLING_MARKER
    )
    if sample_pair_writer_coupling_poc != (coupling_marker is not None):
        raise ValueError(
            "Payload coupling cfg does not match --sample-pair-writer-coupling-poc"
        )
    if coupling_marker is not None:
        symbols[SAMPLE_PAIR_WRITER_COUPLING_MARKER] = coupling_marker
    refresh_marker = find_map_symbol(payload_map, SAMPLE_CACHE_REFRESH_MARKER)
    if sample_cache_refresh_poc != (refresh_marker is not None):
        raise ValueError(
            "Payload cache-refresh cfg does not match --sample-cache-refresh-poc"
        )
    if refresh_marker is not None:
        symbols[SAMPLE_CACHE_REFRESH_MARKER] = refresh_marker
    playback_marker = find_map_symbol(payload_map, PAIRED_PLAYBACK_MARKER)
    if paired_playback_poc != (playback_marker is not None):
        raise ValueError(
            "Payload paired-playback cfg does not match --paired-playback-poc"
        )
    if playback_marker is not None:
        symbols[PAIRED_PLAYBACK_MARKER] = playback_marker
    stereo_position_marker = find_map_symbol(payload_map, STEREO_POSITION_MARKER)
    if stereo_position_poc != (stereo_position_marker is not None):
        raise ValueError(
            "Payload stereo-position cfg does not match --stereo-position-poc"
        )
    if stereo_position_marker is not None:
        symbols[STEREO_POSITION_MARKER] = stereo_position_marker
    for symbol, address in symbols.items():
        if not PAYLOAD_ADDRESS <= address < arm_payload_end:
            raise ValueError(
                f"Payload symbol {symbol} at {address:#010x} is outside "
                f"{PAYLOAD_ADDRESS:#010x}..{payload_end:#010x}"
            )
    for address, (symbol, expected) in hooks.items():
        branch = (
            encode_thumb_bl(address, symbols[symbol])
            if address in FINAL_WRITE_TONE_HOOK
            or address in NATIVE_DSP_UPLOAD_CALL_HOOK
            else encode_thumb_bw(address, symbols[symbol])
        )
        if decode_thumb_bw(address, branch) != symbols[symbol]:
            raise AssertionError(f"Internal B.W round-trip failed for {symbol}")
        patch_bytes(
            image,
            changes,
            address,
            expected,
            branch,
            f"Branch to appended {symbol}",
        )

    if sample_pair_identity_poc:
        for address, (symbol, expected) in SAMPLE_PAIR_IDENTITY_CALL_HOOK.items():
            branch = encode_thumb_bl(address, symbols[symbol])
            if decode_thumb_bw(address, branch) != symbols[symbol]:
                raise AssertionError(
                    f"Internal BL round-trip failed for {symbol}"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                branch,
                f"Call appended {symbol}",
            )

    if main_loop_tick_hook:
        for address, (symbol, expected) in MAIN_LOOP_TICK_CALL_HOOK.items():
            branch = encode_thumb_bl(address, symbols[symbol])
            if decode_thumb_bw(address, branch) != symbols[symbol]:
                raise AssertionError(
                    f"Internal BL round-trip failed for {symbol}"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                branch,
                f"Call appended {symbol}",
            )

    if stereo_stock_save_queue:
        stock_queue_call_hooks = dict(STEREO_SAVE_QUEUE_CALL_HOOK)
        stock_queue_call_hooks.update(STEREO_STOCK_HEADER_CALL_HOOK)
        for address, (symbol, expected) in stock_queue_call_hooks.items():
            branch = encode_thumb_bl(address, symbols[symbol])
            if decode_thumb_bw(address, branch) != symbols[symbol]:
                raise AssertionError(
                    f"Internal BL round-trip failed for {symbol}"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                branch,
                f"Call appended {symbol}",
            )

    if stereo_position_poc:
        for address, (symbol, expected) in STEREO_POSITION_CALL_HOOK.items():
            branch = encode_thumb_bl(address, symbols[symbol])
            if decode_thumb_bw(address, branch) != symbols[symbol]:
                raise AssertionError(
                    f"Internal BL round-trip failed for {symbol}"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                branch,
                f"Call appended {symbol}",
            )

    if stereo_position_poc:
        stereo_position_branch_hooks = (
            MIXER_VOLUME_PROBE_BRANCH_HOOK
            if mixer_volume_probe
            else STEREO_POSITION_KNOB_BRANCH_HOOK
        )
        for address, (symbol, expected) in stereo_position_branch_hooks.items():
            branch = encode_thumb_bw(address, symbols[symbol])
            if decode_thumb_bw(address, branch) != symbols[symbol]:
                raise AssertionError(
                    f"Internal B.W round-trip failed for {symbol}"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                branch,
                f"Branch to appended {symbol}",
            )

    if sample_pair_identity_poc:
        for address, (symbol, expected) in PAIRED_PLAYBACK_ALLOCATION_HOOK.items():
            branch = encode_thumb_bl(address, symbols[symbol])
            if decode_thumb_bw(address, branch) != symbols[symbol]:
                raise AssertionError(
                    f"Internal BL round-trip failed for {symbol}"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                branch,
                (
                    "Store writer owner and request compile-time-sized pair-service state"
                ),
            )

    if sample_pair_gesture_poc:
        patch_bytes(
            image,
            changes,
            0x0803D312,
            bytes.fromhex("6020"),
            bytes.fromhex("6820"),
            "Grow sample view allocation from 0x60 to 0x68 bytes",
        )
        for address, (symbol, expected) in SAMPLE_PAIR_GESTURE_BRANCH_HOOK.items():
            branch = encode_thumb_bw(address, symbols[symbol])
            if decode_thumb_bw(address, branch) != symbols[symbol]:
                raise AssertionError(
                    f"Internal B.W round-trip failed for {symbol}"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                branch,
                f"Initialize appended sample-view gesture state in {symbol}",
            )
        for address, (symbol, expected) in SAMPLE_PAIR_GESTURE_VTABLE_HOOKS.items():
            if stock.count(expected) != 1:
                raise ValueError(
                    f"Sample-view pointer for {symbol} is not unique in the pinned image"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                struct.pack("<I", symbols[symbol] | 1),
                f"Redirect sample-view slot to {symbol}",
            )
            if bytes(image).count(expected) != 0:
                raise AssertionError(
                    f"Stock sample-view pointer for {symbol} remains after patch"
                )

    if sample_pair_led_poc:
        expected_pointer = SAMPLE_PAIR_LED_VTABLE_HOOK[0x08067A24][1]
        if stock.count(expected_pointer) != 1:
            raise ValueError(
                "Sample-view render pointer is not unique in the pinned image"
            )
        for address, expected in SAMPLE_COLOR_ASSERTIONS.items():
            require_bytes(image, address, expected)
        symbol = SAMPLE_PAIR_LED_VTABLE_HOOK[0x08067A24][0]
        replacement_pointer = symbols[symbol] | 1
        patch_bytes(
            image,
            changes,
            0x08067A24,
            expected_pointer,
            struct.pack("<I", replacement_pointer),
            "Redirect unique sample-view render slot to pair LED POC",
        )
        if bytes(image).count(expected_pointer) != 0:
            raise AssertionError("Stock sample-view render pointer remains after patch")

    if native_dual_capture_poc:
        for address, (symbol, expected) in NATIVE_CAPTURE_VTABLE_HOOK.items():
            if stock.count(expected) != 1:
                raise ValueError(
                    "Stock capture-start vtable pointer is not unique in the pinned image"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                struct.pack("<I", symbols[symbol] | 1),
                "Redirect capture-source start to native dual-mono wrapper",
            )

    if stereo_stock_save_queue:
        for address, (symbol, expected) in STEREO_STOCK_SOURCE_READ_VTABLE_HOOK.items():
            if stock.count(expected) != 1:
                raise ValueError(
                    "Stock capture-source read vtable pointer is not unique"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                struct.pack("<I", symbols[symbol] | 1),
                "Redirect capture-source reads to bounded native stereo wrapper",
            )
        for address, (symbol, expected) in STEREO_STOCK_OCCUPANCY_VTABLE_HOOK.items():
            if stock.count(expected) != 1:
                raise ValueError(
                    "Stock sample-occupancy observer pointer is not unique"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                struct.pack("<I", symbols[symbol] | 1),
                "Fill both stereo pads when recording stops, as stock mono does",
            )

    if (crop_io_trace or crop_mirror_window) and native_dual_capture_poc:
        for address, (symbol, expected) in CROP_WINDOW_VTABLE_HOOK.items():
            if stock.count(expected) != 1:
                raise ValueError("Stock loop-window vtable pointer is not unique")
            patch_bytes(
                image,
                changes,
                address,
                expected,
                struct.pack("<I", symbols[symbol] | 1),
                "Observe the capture loop-window setters a crop drives",
            )

    if paired_playback_poc:
        for address, (symbol, expected) in PAIRED_PLAYBACK_VTABLE_HOOKS.items():
            if stock.count(expected) != 1:
                raise ValueError(
                    f"Playback adapter pointer for {symbol} is not unique"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                struct.pack("<I", symbols[symbol] | 1),
                f"Redirect playback adapter slot to {symbol}",
            )
            if bytes(image).count(expected) != 0:
                raise AssertionError(
                    f"Stock playback pointer for {symbol} remains after patch"
                )

    if stereo_position_poc:
        stereo_position_vtable_hooks = dict(STEREO_POSITION_VTABLE_HOOKS)
        stereo_position_vtable_hooks.update(
            MIXER_VOLUME_PROBE_VTABLE_HOOK
            if mixer_volume_probe
            else STEREO_POSITION_KNOB_VTABLE_HOOK
        )
        for address, (symbol, expected) in stereo_position_vtable_hooks.items():
            if stock.count(expected) != 1:
                raise ValueError(
                    f"Stereo-position pointer for {symbol} is not unique"
                )
            patch_bytes(
                image,
                changes,
                address,
                expected,
                struct.pack("<I", symbols[symbol] | 1),
                f"Redirect paired-control slot to {symbol}",
            )
            if bytes(image).count(expected) != 0:
                raise AssertionError(
                    f"Stock stereo-position pointer for {symbol} remains after patch"
                )
        address, expected, replacement = DEFAULT_TRACK_VOLUME_PATCH
        patch_bytes(
            image,
            changes,
            address,
            expected,
            replacement,
            "Default track volume 100 -> 89 (70%) in blank-project template",
        )
        for address, (symbol, expected) in PAIR_COLOR_CALL_HOOK.items():
            branch = encode_thumb_bl(address, symbols[symbol])
            if decode_thumb_bw(address, branch) != symbols[symbol]:
                raise AssertionError(f"Internal BL round-trip failed for {symbol}")
            patch_bytes(
                image,
                changes,
                address,
                expected,
                branch,
                f"Call appended {symbol}",
            )
        for address, (symbol, expected) in PAIR_COLOR_POINTER_HOOK.items():
            if stock.count(expected) != 1:
                raise ValueError(f"Pointer for {symbol} is not unique")
            patch_bytes(
                image,
                changes,
                address,
                expected,
                struct.pack("<I", symbols[symbol] | 1),
                f"Redirect task entry to {symbol}",
            )
        table_literals = []
        for offset in range(0, EXPECTED_SIZE, 4):
            value = struct.unpack_from("<I", stock, offset)[0]
            if (
                PAIR_COLOR_TABLE_FLASH
                <= value
                < PAIR_COLOR_TABLE_FLASH + PAIR_COLOR_TABLE_BYTES
            ):
                table_literals.append((IMAGE_BASE + offset, value))
        if len(table_literals) != PAIR_COLOR_TABLE_LITERALS:
            raise ValueError(
                f"Expected {PAIR_COLOR_TABLE_LITERALS} colour-table literals, "
                f"found {len(table_literals)}"
            )
        for address, value in table_literals:
            patch_bytes(
                image,
                changes,
                address,
                struct.pack("<I", value),
                struct.pack(
                    "<I", value - PAIR_COLOR_TABLE_FLASH + PAIR_COLOR_TABLE_RAM
                ),
                "Redirect colour-table literal to the shared-pair-colour RAM copy",
            )

    if len(image) != EXPECTED_SIZE:
        raise AssertionError("An in-place patch unexpectedly changed the image size")
    image.extend(appended)
    changes.append(
        {
            "address": f"0x{PAYLOAD_ADDRESS:08X}",
            "length": len(appended),
            "purpose": "Appended ARM payload and native DSP extension",
            "before": "",
            "after_sha256": sha256(appended),
        }
    )

    result = bytes(image)
    manifest: dict[str, object] = {
        "input_sha256": EXPECTED_SHA256,
        "output_sha256": sha256(result),
        "input_size": len(stock),
        "output_size": len(result),
        "application_build": application_build,
        "stock_dsp": stock_dsp,
        "payload_address": f"0x{PAYLOAD_ADDRESS:08X}",
        "payload_size": len(payload),
        "native_dual_capture_poc": native_dual_capture_poc,
        "flash_layout": {
            "stock_image_end": f"0x{PAYLOAD_ADDRESS:08X}",
            "patched_image_end": f"0x{payload_end:08X}",
            "containing_sector": 5,
            "sector_start": f"0x{SECTOR_5_START:08X}",
            "sector_end_exclusive": f"0x{SECTOR_6_START:08X}",
            "remaining_sector_bytes": SECTOR_6_START - payload_end,
            "native_bootloader_size_ceiling": (
                PROVEN_NATIVE_BOOTLOADER_MAX_IMAGE_SIZE
            ),
            "bootloader_accepted_size_headroom": (
                PROVEN_NATIVE_BOOTLOADER_MAX_IMAGE_SIZE - len(result)
            ),
            "adds_erase_sector": False,
            "bootloader_acceptance_proven": False,
        },
        "symbols": {name: f"0x{address:08X}" for name, address in symbols.items()},
        "changes": changes,
    }
    if native_dual_capture_poc:
        assert native_dsp_package is not None
        manifest["native_dsp_extension"] = {
            "address": f"0x{extension_address:08X}",
            "padding": extension_padding,
            "size": len(native_dsp_package),
            "image_tail_padding": image_tail_padding,
            "sha256": sha256(native_dsp_package),
            "upload": "primary-B continuation record before stock terminator",
        }
    manifest["native_capture_preflight_probe_diagnostic"] = (
        native_capture_preflight_probe_diagnostic
    )
    manifest["native_writer_readiness_probe_diagnostic"] = (
        native_writer_readiness_probe_diagnostic
    )
    manifest["native_writer_acquisition_probe_diagnostic"] = (
        native_writer_acquisition_probe_diagnostic
    )
    manifest["native_arm_token_probe_diagnostic"] = native_arm_token_probe_diagnostic
    manifest["native_pack_value_probe_diagnostic"] = native_pack_value_probe_diagnostic
    manifest["native_pack_correction_probe_diagnostic"] = (
        native_pack_correction_probe_diagnostic
    )
    manifest["native_finalization_probe_diagnostic"] = (
        native_finalization_probe_diagnostic
    )
    manifest["native_publication_probe_diagnostic"] = (
        native_publication_probe_diagnostic
    )
    manifest["native_terminal_publish_probe_diagnostic"] = (
        native_terminal_publish_probe_diagnostic
    )
    manifest["native_publish_boundary_probe_diagnostic"] = (
        native_publish_boundary_probe_diagnostic
    )
    manifest["native_publish_slot_probe_diagnostic"] = (
        native_publish_slot_probe_diagnostic
    )
    manifest["stereo_stock_save_queue"] = stereo_stock_save_queue
    manifest["crop_io_trace"] = crop_io_trace
    manifest["crop_mirror_window"] = crop_mirror_window
    manifest["native_deferred_refresh_probe_diagnostic"] = (
        native_deferred_refresh_probe_diagnostic
    )
    return result, manifest


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", type=Path, required=True)
    parser.add_argument("--template", type=Path, required=True)
    parser.add_argument("--dsp-lod", type=Path, required=True)
    parser.add_argument("--payload", type=Path, required=True)
    parser.add_argument("--payload-map", type=Path, required=True)
    parser.add_argument("--output-image", type=Path, required=True)
    parser.add_argument("--output-sysex", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--protocol-version", type=lambda value: int(value, 0), required=True)
    parser.add_argument("--application-build", type=int, required=True)
    parser.add_argument("--stock-dsp", action="store_true")
    parser.add_argument("--final-write-tone", action="store_true")
    parser.add_argument("--safe-pair-guard", action="store_true")
    parser.add_argument("--sample-pair-led-poc", action="store_true")
    parser.add_argument("--sample-pair-identity-poc", action="store_true")
    parser.add_argument("--sample-pair-gesture-poc", action="store_true")
    parser.add_argument("--sample-pair-writer-coupling-poc", action="store_true")
    parser.add_argument("--sample-cache-refresh-poc", action="store_true")
    parser.add_argument("--paired-playback-poc", action="store_true")
    parser.add_argument("--stereo-position-poc", action="store_true")
    parser.add_argument("--mixer-volume-probe", action="store_true")
    parser.add_argument("--writer-request-probe-diagnostic", action="store_true")
    parser.add_argument("--native-dual-capture-poc", action="store_true")
    parser.add_argument(
        "--native-capture-preflight-probe-diagnostic", action="store_true"
    )
    parser.add_argument(
        "--native-writer-readiness-probe-diagnostic", action="store_true"
    )
    parser.add_argument(
        "--native-writer-acquisition-probe-diagnostic", action="store_true"
    )
    parser.add_argument("--native-arm-token-probe-diagnostic", action="store_true")
    parser.add_argument("--native-pack-value-probe-diagnostic", action="store_true")
    parser.add_argument(
        "--native-pack-correction-probe-diagnostic", action="store_true"
    )
    parser.add_argument("--native-finalization-probe-diagnostic", action="store_true")
    parser.add_argument("--native-publication-probe-diagnostic", action="store_true")
    parser.add_argument(
        "--native-terminal-publish-probe-diagnostic", action="store_true"
    )
    parser.add_argument(
        "--native-publish-boundary-probe-diagnostic", action="store_true"
    )
    parser.add_argument("--native-publish-slot-probe-diagnostic", action="store_true")
    parser.add_argument(
        "--stereo-stock-save-queue", action="store_true"
    )
    parser.add_argument("--crop-io-trace", action="store_true")
    parser.add_argument("--crop-mirror-window", action="store_true")
    parser.add_argument(
        "--native-deferred-refresh-probe-diagnostic", action="store_true"
    )
    parser.add_argument("--native-dsp-package", type=Path)
    parser.add_argument("--native-dsp-metadata", type=Path)
    args = parser.parse_args()

    patched, manifest = apply_patches(
        args.image.read_bytes(),
        args.dsp_lod,
        args.payload.read_bytes(),
        args.payload_map,
        args.application_build,
        args.stock_dsp,
        args.final_write_tone,
        args.safe_pair_guard,
        args.sample_pair_led_poc,
        args.sample_pair_identity_poc,
        args.sample_pair_gesture_poc,
        args.sample_pair_writer_coupling_poc,
        args.sample_cache_refresh_poc,
        args.paired_playback_poc,
        args.stereo_position_poc,
        args.mixer_volume_probe,
        args.writer_request_probe_diagnostic,
        args.native_dual_capture_poc,
        args.native_capture_preflight_probe_diagnostic,
        args.native_writer_readiness_probe_diagnostic,
        args.native_writer_acquisition_probe_diagnostic,
        args.native_arm_token_probe_diagnostic,
        args.native_pack_value_probe_diagnostic,
        args.native_pack_correction_probe_diagnostic,
        args.native_finalization_probe_diagnostic,
        args.native_publication_probe_diagnostic,
        args.native_terminal_publish_probe_diagnostic,
        args.native_publish_boundary_probe_diagnostic,
        args.native_publish_slot_probe_diagnostic,
        args.native_deferred_refresh_probe_diagnostic,
        args.stereo_stock_save_queue,
        args.crop_io_trace,
        args.crop_mirror_window,
        args.native_dsp_package.read_bytes() if args.native_dsp_package else None,
        (
            json.loads(args.native_dsp_metadata.read_text())
            if args.native_dsp_metadata
            else None
        ),
    )
    packed = repack(
        args.template.read_bytes(),
        patched,
        version=args.protocol_version,
    )
    decoded, container = extract(packed)
    if decoded != patched:
        raise AssertionError("Repacked stereo image did not decode byte-exactly")
    if container["version"] != args.protocol_version:
        raise AssertionError("Repacked stereo image has the wrong protocol version")
    manifest["protocol_version"] = args.protocol_version
    manifest["container"] = container

    for path in (args.output_image, args.output_sysex, args.manifest):
        path.parent.mkdir(parents=True, exist_ok=True)
    args.output_image.write_bytes(patched)
    args.output_sysex.write_bytes(packed)
    args.manifest.write_text(json.dumps(manifest, indent=2) + "\n")

    print(
        f"Patched image: {args.output_image} "
        f"({len(patched)} bytes, SHA-256 {manifest['output_sha256']})"
    )
    print(f"Repacked SysEx: {args.output_sysex} ({len(packed)} bytes)")
    print(f"Patch manifest: {args.manifest}")


if __name__ == "__main__":
    main()
