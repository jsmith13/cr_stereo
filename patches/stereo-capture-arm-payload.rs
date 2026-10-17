#![no_std]

use core::ptr::{read_volatile, write_volatile};

pub const TRANSFORM_MONO: u32 = 0;
pub const TRANSFORM_LEFT: u32 = 1;
pub const TRANSFORM_RIGHT: u32 = 2;

const WRITER_MAGIC_MONO: u32 = 0x4d4f_4e4f;
const WRITER_MAGIC_LEFT: u32 = 0x4c45_4654;
const WRITER_MAGIC_RIGHT_PENDING: u32 = 0x5250_454e;
const WRITER_MAGIC_LEFT_ROLLBACK_PENDING: u32 = 0x4c52_4250;
const WRITER_MAGIC_RIGHT: u32 = 0x5247_4854;
const PROTECTED_OCCUPIED_STATUS: u32 = 2;
const FIRST_RESERVED_STEREO_SLOT: u8 = 112;
const LAST_RESERVED_STEREO_SLOT: u8 = 127;

const STATE_ASYNC: usize = 0x01;
const STATE_TOTAL_FRAMES: usize = 0x04;
const STATE_WRITTEN_FRAMES: usize = 0x08;
const STATE_BUFFER_OFFSET: usize = 0x0c;
const STATE_DESCRIPTOR: usize = 0x10;
const STATE_COMPLETION: usize = 0x1c;
const STATE_COMPLETION_CONTEXT: usize = 0x20;
const STATE_CAPTURE_SLOT: usize = 0x24;
const STATE_BUFFER: usize = 0x28;
const STATE_BUFFERED_FRAMES: usize = 0x2c;
const STATE_OWNER: usize = 0x30;
const STATE_MAGIC: usize = 0x74;
const STATE_SYNTHETIC_BUFFER: usize = 0x78;
#[cfg(sample_pair_writer_coupling_poc)]
const STATE_ARM_TOKEN: usize = 0x78;
#[cfg(sample_pair_writer_coupling_poc)]
const PAIR_COUNT: usize = 112;
#[cfg(not(sample_pair_writer_coupling_poc))]
const PAIR_COUNT: usize = 8;
const PAIR_BANK_SIZE: usize = (PAIR_COUNT + 7) / 8;
#[cfg(sample_pair_writer_coupling_poc)]
const STATE_PAIR_BANK_0: usize = 0x7c;
#[cfg(not(sample_pair_writer_coupling_poc))]
const STATE_PAIR_BANK_0: usize = 0x78;
const STATE_PAIR_BANK_1: usize = STATE_PAIR_BANK_0 + PAIR_BANK_SIZE;
const STATE_PAIR_ACTIVE_BANK: usize = STATE_PAIR_BANK_1 + PAIR_BANK_SIZE;
const STATE_PAIR_ACTIVE_PACK: usize = STATE_PAIR_ACTIVE_BANK + 1;
const STATE_PAIR_VALID: usize = STATE_PAIR_ACTIVE_PACK + 1;
const STATE_SCAN_MODE: usize = STATE_PAIR_VALID + 1;
const STATE_SCAN_BANK: usize = STATE_SCAN_MODE + 1;
const STATE_SCAN_PENDING_LEFT: usize = STATE_SCAN_BANK + 1;
const STATE_SCAN_EPOCH: usize = (STATE_SCAN_PENDING_LEFT + 4) & !3;
const STATE_MUTATION_EPOCH: usize = STATE_SCAN_EPOCH + 4;
const STATE_REGISTRY_GENERATION: usize = STATE_MUTATION_EPOCH + 4;
const STATE_REFRESH_TOKEN: usize = STATE_REGISTRY_GENERATION + 4;
const STATE_HEADER_WRITE_MARKER: usize = STATE_REFRESH_TOKEN + 4;
const STATE_WRITER_MODE: usize = STATE_HEADER_WRITE_MARKER + 4;
const STATE_WRITER_PHASE: usize = STATE_WRITER_MODE + 1;
const STATE_WRITER_WAIT_TICKS: usize = STATE_WRITER_PHASE + 1;
const STATE_CLEAR_PHASE: usize = STATE_WRITER_WAIT_TICKS + 1;
const STATE_CLEAR_PACK: usize = STATE_CLEAR_PHASE + 1;
const STATE_CLEAR_LEFT: usize = STATE_CLEAR_PACK + 1;
const STATE_CLEAR_COORDINATOR: usize = (STATE_CLEAR_LEFT + 4) & !3;
const STATE_CLEAR_COMPLETION: usize = STATE_CLEAR_COORDINATOR + 4;
const STATE_CLEAR_DESCRIPTOR_RIGHT: usize = STATE_CLEAR_COMPLETION + 4;
const STATE_CLEAR_DESCRIPTOR_LEFT: usize = STATE_CLEAR_DESCRIPTOR_RIGHT + 12;
const STATE_CLEAR_FIRST_STATUS: usize = STATE_CLEAR_DESCRIPTOR_LEFT + 12;
// One latch per possible leader track. Pairing is derived from the sample
// assignment and any adjacent pair is permitted, so leaders are tracks 0..6
// rather than only the even ones.
const STATE_PLAYBACK_LATCHES: usize = (STATE_CLEAR_FIRST_STATUS + 4) & !3;
// Saved LED words for the paired-control mirrors.
//
// The stock colour buffer is persistent: stock only rewrites a word when the
// observed state behind it changes. A word mirrored from a leader onto its
// follower therefore outlives the pairing that produced it, and worse, is read
// back as if it were the leader's own colour if that track later becomes a
// leader itself. Each render restores the previous frame's mirrored words
// *before* calling stock, so stock always sees its own buffer and these saved
// originals can never go stale.
//
// One group per mirror site: a mask of the followers currently overwritten,
// then their saved original words.
const PAIR_LED_GROUP_STRIDE: usize = 20;
const PAIR_LED_GROUPS: usize = 3;
const PAIR_LED_GROUP_MIXER_MUTE: usize = 0;
const PAIR_LED_GROUP_MIXER_MACRO: usize = 1;
const PAIR_LED_GROUP_FX_SEND: usize = 2;
const STATE_PAIR_LED_SAVE: usize = STATE_PLAYBACK_LATCHES + 8;
// Which leaders were paired on the previous service pass, so pair formation and
// dissolution can be detected and the mechanically imposed follower pan undone.
const STATE_PAIR_POSITION_MASK: usize =
    STATE_PAIR_LED_SAVE + PAIR_LED_GROUP_STRIDE * PAIR_LED_GROUPS;
// Per-controller pairing mask for FX send reconciliation. The FX page writes
// parameter `controller[+0x40] + track`, a per-page base, so sends can only be
// driven through a controller and the FX page appears to instantiate the same
// class more than once (one per send destination). Each instance therefore
// needs its own record of the pairing it last reconciled.
const PAIR_SEND_SLOTS: usize = 2;
const PAIR_SEND_SLOT_STRIDE: usize = 8;
const STATE_PAIR_SEND_SLOTS: usize = (STATE_PAIR_POSITION_MASK + 4) & !3;
// Mixer volume reconciliation. Volume is not track parameter 22 (that was the
// rejected V7 premise); the mixer emits it as ID `0x91 + track` through its own
// controller, so it can only be driven through a latched mixer controller.
// `PENDING` holds leaders whose pair dissolved but whose follower has not yet
// been returned to full scale, so a dissolution seen before any controller is
// available is applied later rather than lost.
const STATE_PAIR_VOLUME_CONTROLLER: usize =
    STATE_PAIR_SEND_SLOTS + PAIR_SEND_SLOTS * PAIR_SEND_SLOT_STRIDE;
const STATE_PAIR_VOLUME_PENDING: usize = STATE_PAIR_VOLUME_CONTROLLER + 4;
// Sidechain reconciliation: the latched sidechain page controller and the
// pairing it last reconciled. Both fit in the padding before the probe block,
// so nothing after them moves.
const STATE_PAIR_SIDECHAIN_MASK: usize = STATE_PAIR_VOLUME_PENDING + 1;
const STATE_PAIR_SIDECHAIN_CONTROLLER: usize = STATE_PAIR_VOLUME_CONTROLLER + 8;
const STATE_MIXER_PROBE_FLAGS: usize = (STATE_PAIR_VOLUME_PENDING + 8) & !3;
const _: () = assert!(STATE_PAIR_SIDECHAIN_CONTROLLER + 4 <= STATE_MIXER_PROBE_FLAGS);
const STATE_MIXER_PROBE_PATH: usize = STATE_MIXER_PROBE_FLAGS + 1;
const STATE_MIXER_PROBE_MODE: usize = STATE_MIXER_PROBE_PATH + 1;
const STATE_MIXER_PROBE_KNOB: usize = STATE_MIXER_PROBE_MODE + 1;
const STATE_MIXER_PROBE_DELTA: usize = STATE_MIXER_PROBE_KNOB + 1;
const STATE_MIXER_PROBE_18_BEFORE: usize = STATE_MIXER_PROBE_DELTA + 1;
const STATE_MIXER_PROBE_18_AFTER: usize = STATE_MIXER_PROBE_18_BEFORE + 1;
const STATE_MIXER_PROBE_1C_BEFORE: usize = STATE_MIXER_PROBE_18_AFTER + 1;
const STATE_MIXER_PROBE_1C_AFTER: usize = STATE_MIXER_PROBE_1C_BEFORE + 1;
const STATE_MIXER_PROBE_VTABLE_COUNT: usize = STATE_MIXER_PROBE_1C_AFTER + 1;
const STATE_MIXER_PROBE_BRANCH_COUNT: usize = STATE_MIXER_PROBE_VTABLE_COUNT + 1;
const STATE_WRITER_REQUEST_SOURCE: usize = (STATE_MIXER_PROBE_BRANCH_COUNT + 4) & !3;
#[cfg(native_dual_capture_poc)]
const STATE_CAPTURE_UI_TOKEN: usize = STATE_WRITER_REQUEST_SOURCE;
#[cfg(native_dual_capture_poc)]
const STATE_NATIVE_RETAINED_PCM: usize = STATE_WRITER_REQUEST_SOURCE + 4;
#[cfg(native_dual_capture_poc)]
const STATE_DEFERRED_REFRESH_VIEW: usize = STATE_NATIVE_RETAINED_PCM + 44;
// Capture-time latches owned exclusively by the save pipeline.
//
// `STATE_ARM_TOKEN` and `STATE_DEFERRED_REFRESH_VIEW` are owned by the pad
// gesture and are torn down by `sample_pair_gesture_press_before_stock` on the
// next press that finds no pad held, and by `sample_pair_gesture_clear`. The
// right slot's Save completion runs long after the gesture has ended, so the
// terminal publication must not depend on either of them still being live.
// `native_capture_source_start` copies both into these latches when a stereo
// capture actually begins; only the save pipeline clears them.
#[cfg(native_dual_capture_poc)]
const STATE_SAVE_PAIR_LATCH: usize = STATE_DEFERRED_REFRESH_VIEW + 4;
#[cfg(native_dual_capture_poc)]
const STATE_SAVE_VIEW_LATCH: usize = STATE_DEFERRED_REFRESH_VIEW + 8;
// Identity of a stereo pair whose save is still in flight when the *next*
// capture begins. `native_capture_source_start` overwrites (or zeroes) the
// primary latch, which used to strand the in-flight pair: its right channel's
// header write could no longer tell it was a right channel, so the 96002 WAV
// byte-rate marker was never stamped and the pair was permanently unlinked --
// the boot rescan had nothing to rebuild it from. Carrying the displaced latch
// here keeps the in-flight save resolvable.
//
// This is a FIFO table, not a single word: hardware showed that one entry is
// enough for two overlapping captures but still strands the oldest when three
// overlap. With the primary latch, `STEREO_CARRY_SLOTS` entries cover
// `STEREO_CARRY_SLOTS + 1` concurrently saving pairs.
#[cfg(native_dual_capture_poc)]
const STATE_SAVE_PAIR_CARRY: usize = STATE_DEFERRED_REFRESH_VIEW + 12;
#[cfg(native_dual_capture_poc)]
const STEREO_CARRY_SLOTS: usize = 4;
// Deferred peer-member Clear. Pair-wide Clear is implemented by letting the
// stock transaction delete the pressed (canonical left) member, then replaying
// the stock Sample Clear UI entry 0x0803A10E for the peer from the main loop.
// Re-entering the storage worker from inside its own completion is what broke
// every earlier pair-Clear attempt, so the peer request is deferred instead.
#[cfg(native_dual_capture_poc)]
const STATE_PEER_CLEAR_UI: usize = STATE_SAVE_PAIR_CARRY + 4 * STEREO_CARRY_SLOTS;
#[cfg(native_dual_capture_poc)]
const STATE_PEER_CLEAR_SLOT: usize = STATE_PEER_CLEAR_UI + 4;
#[cfg(native_dual_capture_poc)]
const STATE_PEER_CLEAR_ATTEMPTS: usize = STATE_PEER_CLEAR_UI + 5;
#[cfg(native_dual_capture_poc)]
const STATE_PEER_CLEAR_TICKS: usize = STATE_PEER_CLEAR_UI + 6;
/// Consecutive ticks the Sample Clear transaction has been observed busy.
#[cfg(native_dual_capture_poc)]
const STATE_PEER_CLEAR_BUSY_TICKS: usize = STATE_PEER_CLEAR_UI + 8;
#[cfg(native_dual_capture_poc)]
const PEER_CLEAR_NONE: u8 = 0xff;
// Deferred peer Clear pacing.
#[cfg(native_dual_capture_poc)]
/// Consecutive Main-loop ticks that must observe an idle Sample Clear
/// transaction before the peer's request is submitted.
///
/// `busy == 0` is *not* on its own proof that storage is idle: the delete
/// callback `0x08047344` enqueues the op-8 sync and then immediately invokes
/// the completion, so `SampleClear_OnComplete` clears busy while the sync is
/// still draining. Submitting into that gap piles a second request onto a
/// still-working queue, and the enqueue is fire-and-forget
/// (`FUN_08045F42` returns void, `0x0803A10E` ignores it), so a dropped post
/// strands the busy flag at 2 and silently rejects every later Clear.
const PEER_CLEAR_SETTLE_TICKS: u16 = 512;
/// Bounded re-submissions, used only when the peer's occupancy bit is still
/// set after a previous attempt should have cleared it.
#[cfg(native_dual_capture_poc)]
const PEER_CLEAR_MAX_ATTEMPTS: u8 = 8;
/// Consecutive busy ticks after which the Sample Clear transaction is treated
/// as dead and its busy byte is released.
///
/// The submit path is fire-and-forget all the way down (`FUN_08045F42` returns
/// void and `0x0803A10E` ignores it), and `STORAGE_sub_08041388` can fail
/// without ever reaching the completion at `0x08047344`. When that happens the
/// busy byte stays at 2 forever and *every* later Clear press is silently
/// rejected until the unit is power-cycled. This only ever runs while a
/// deferred peer Clear is outstanding, so its blast radius is the one window
/// where the wedge has actually been observed.
#[cfg(native_dual_capture_poc)]
const PEER_CLEAR_WEDGE_TICKS: u16 = 10000;
/// Standalone voice-channel probe storage.
///
/// Deliberately does **not** reuse `native_capture_preflight_probe_diagnostic`.
/// That flag selects a larger `WRITER_STATE_SIZE` (`0x14C` instead of `0x148`)
/// and instruments the capture paths, and hardware bisection
/// (`stereo-voiceprobe-c`) showed the flag alone crashes the instrument
/// whenever a sample is played. This probe must not depend on it.
///
/// `STATE_PEER_CLEAR_BUSY_TICKS` is a `u16` at `0x144` and the block is padded
/// to `0x148`, so `0x146..0x147` is existing dead padding. Storing the probe
/// there keeps `WRITER_STATE_SIZE` **unchanged**, which is the whole point:
/// nothing about the allocation or any other offset moves.
#[cfg(all(native_dual_capture_poc, voice_channel_probe))]
const STATE_VOICE_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 2;
#[cfg(all(native_dual_capture_poc, voice_channel_probe))]
const VOICE_PROBE_SEEN: u16 = 1 << 15;
/// Storage-I/O trace for the crop mirror.
///
/// `FUN_0804603A` (op 6, read) and `FUN_0804607A` (op 7, write) both funnel
/// into `FUN_08045EB6(op, desc, &{offset, buf, len, ...})`, so a single hook at
/// `0x08045EB6` observes every sample-file transfer. `desc` is
/// `{u32 cmd = 4, u32 slot, u8 pack}`; only `cmd == 4` is a sample file.
///
/// Reads are constant during normal operation (every upload pulls a 0x40-byte
/// header), so a raw log would be swamped. Writes to sample files are rare -
/// essentially only recording and cropping - so the first write acts as the
/// trigger, and the immediately preceding read is snapshotted to capture the
/// crop's source offset.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const STATE_CROP_IO: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_COUNTS: usize = STATE_CROP_IO;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_SLOT: usize = STATE_CROP_IO + 4;
/// Next expected data-write offset while a crop candidate is live. The shift is
/// one contiguous run from `0x2C`, so this both validates the candidate and
/// makes the accumulated length trustworthy.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_NEXT_WRITE: usize = STATE_CROP_IO + 8;
/// Length of the read that preceded the shift. The crop reads the kept region
/// into RAM before writing any of it back, so this is the region's extent.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_SNAP_READ_LEN: usize = STATE_CROP_IO + 12;
/// Set while a crop candidate is live. A crop that only trims the end has
/// `S == 0`, so liveness cannot be inferred from the shift distance.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_ACTIVE: usize = STATE_CROP_IO + 16;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_TOTAL_WRITE: usize = STATE_CROP_IO + 20;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_LAST_READ_OFF: usize = STATE_CROP_IO + 24;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_LAST_READ_LEN: usize = STATE_CROP_IO + 28;
/// Slot the rolling read belongs to. Without this a *recording*, which also
/// writes from `0x2C`, is indistinguishable from a crop; only a crop is
/// preceded by an audio-region read **on the slot it then writes**.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_LAST_READ_SLOT: usize = STATE_CROP_IO + 32;
/// Source offset of the crop's shift, i.e. how far into the data region the
/// kept span began, as observed. Diagnostic only.
///
/// `STATE_CROP_IO + 36/40/48..60` are deliberately left unused: they held the
/// byte-shift mirror's phase/progress fields, and the offsets are kept vacant
/// so the surrounding trace layout is unchanged.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_SRC_OFF: usize = STATE_CROP_IO + 44;
/// Detection evidence, so a failure explains itself without another build.
/// Bits 0-4 count writes at the data start, bits 5-9 how many of those were
/// preceded by a same-slot read further into the file, bits 10-15 count
/// trailing 44-byte header writes.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_DIAG: usize = STATE_CROP_IO + 64;
/// Monotonic count of sample-file operations, used only to require that the
/// crop's source read is the operation *immediately* before the shift's first
/// write. A stale read left behind by playback must not be able to qualify.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_OP_COUNT: usize = STATE_CROP_IO + 68;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_LAST_READ_OP: usize = STATE_CROP_IO + 72;
/// Operations between the source read and the shift's first write, as actually
/// observed. The gap must be bounded or a read left behind by playback could
/// qualify a later recording, but the bound has to come from hardware.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_GAP: usize = STATE_CROP_IO + 76;

/// Per-slot witness of the byte-rate sentinels the pack-load scan observes.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_SCAN_LEFT: usize = STATE_CROP_IO + 80;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_SCAN_RIGHT: usize = STATE_CROP_IO + 84;
/// Registry health at the moment a crop asks for a partner.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_REG_DIAG: usize = STATE_CROP_IO + 104;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_REG_BITS: usize = STATE_CROP_IO + 108;
/// Count of sample-file writes observed. The source read compares this against
/// its own previous snapshot: if writes happened between two reads of the same
/// slot, those reads belong to a rewrite rather than to playback.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_SRC_WRITES: usize = STATE_CROP_IO + 100;
/// Return address of whatever invoked the sample-file API for the crop's first
/// data write. The static call graph does not reach the code that persists a
/// crop, so the rewriter is identified by asking it directly.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_IO_CALLER: usize = STATE_CROP_IO + 148;
/// Loop-window trace. `AudioCapture_SetLoopWindow(obj, slot, start, end)` and
/// its blocking variant write the capture hardware's window registers
/// (0x371D8 start, 0x371D9 end) and are the strongest candidate for where a
/// crop is actually applied. These latch what the crop passes.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_WIN_SLOT: usize = STATE_CROP_IO + 116;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_WIN_START: usize = STATE_CROP_IO + 120;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_WIN_END: usize = STATE_CROP_IO + 124;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_WIN_CALLS: usize = STATE_CROP_IO + 128;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const CROP_WIN_CALLS_WAIT: usize = STATE_CROP_IO + 132;
/// The widest window seen, which for a crop that narrows a sample is the
/// pre-crop extent - useful for telling a crop apart from a plain re-arm.
///
/// These three are written by `crop_window_mirror`, so they are only live when
/// the mirror is also compiled in.
#[cfg(all(native_dual_capture_poc, crop_io_trace, crop_mirror_window))]
const CROP_WIN_PARTNER: usize = STATE_CROP_IO + 136;
#[cfg(all(native_dual_capture_poc, crop_io_trace, crop_mirror_window))]
const CROP_WIN_MIRRORS: usize = STATE_CROP_IO + 140;
/// Attempts and the reason the last one did not mirror. Written unconditionally
/// so that an unmirrored crop reports *why* rather than leaving a stale field to
/// be misread as data.
#[cfg(all(native_dual_capture_poc, crop_io_trace, crop_mirror_window))]
const CROP_WIN_ATTEMPTS: usize = STATE_CROP_IO + 144;

#[cfg(all(native_dual_capture_poc, native_capture_preflight_probe_diagnostic))]
const STATE_CAPTURE_PREFLIGHT_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_writer_readiness_probe_diagnostic))]
const STATE_NATIVE_WRITER_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_writer_acquisition_probe_diagnostic))]
const STATE_NATIVE_WRITER_ACQUISITION_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_arm_token_probe_diagnostic))]
const STATE_NATIVE_ARM_TOKEN_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_pack_value_probe_diagnostic))]
const STATE_NATIVE_PACK_VALUE_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_pack_correction_probe_diagnostic))]
const STATE_NATIVE_PACK_CORRECTION_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_finalization_probe_diagnostic))]
const STATE_NATIVE_FINALIZATION_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_publication_probe_diagnostic))]
const STATE_NATIVE_PUBLICATION_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_terminal_publish_probe_diagnostic))]
const STATE_NATIVE_TERMINAL_PUBLISH_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_publish_boundary_probe_diagnostic))]
const STATE_NATIVE_PUBLISH_BOUNDARY_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(all(native_dual_capture_poc, native_publish_slot_probe_diagnostic))]
const STATE_NATIVE_PUBLISH_SLOT_PROBE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
/// Slot whose stereo partner has had a crop window mirrored onto it, plus a
/// validity tag. Consumed by the save request so the partner's file is only
/// ever rewritten when its window was actually changed by a mirror - never on
/// an unrelated save of a paired slot.
#[cfg(all(native_dual_capture_poc, crop_mirror_window, not(crop_io_trace)))]
const STATE_CROP_MIRROR_ARM: usize = STATE_PEER_CLEAR_BUSY_TICKS + 8;
#[cfg(all(native_dual_capture_poc, crop_mirror_window, crop_io_trace))]
const STATE_CROP_MIRROR_ARM: usize = STATE_CROP_IO + 152;
/// Tag distinguishing an armed partner rewrite from a zeroed or reused field.
/// The low 7 bits carry the slot whose partner was mirrored.
#[cfg(all(native_dual_capture_poc, crop_mirror_window))]
const CROP_MIRROR_ARM_MAGIC: u32 = 0x4352_0000;
/// Writer-state size. Every arm below must cover the highest field offset any
/// enabled cfg defines, and the assertions at the end of the block must be
/// edited together with it.
#[cfg(all(
    native_dual_capture_poc,
    crop_mirror_window,
    not(crop_io_trace)
))]
const WRITER_STATE_SIZE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 12;
#[cfg(all(
    native_dual_capture_poc,
    not(crop_io_trace),
    not(crop_mirror_window),
    any(
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
        native_publish_slot_probe_diagnostic
    )
))]
const WRITER_STATE_SIZE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 8;
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const WRITER_STATE_SIZE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 160;
#[cfg(all(
    native_dual_capture_poc,
    not(crop_io_trace),
    not(crop_mirror_window),
    not(any(
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
        native_publish_slot_probe_diagnostic
    ))
))]
const WRITER_STATE_SIZE: usize = STATE_PEER_CLEAR_BUSY_TICKS + 4;
#[cfg(not(native_dual_capture_poc))]
const WRITER_STATE_SIZE: usize = STATE_WRITER_REQUEST_SOURCE + 4;
const _: [(); 0x74] = [(); STATE_MAGIC];
const _: [(); 0] = [(); WRITER_STATE_SIZE & 3];
#[cfg(all(native_dual_capture_poc, sample_pair_writer_coupling_poc))]
const _: [(); 0x78] = [(); STATE_ARM_TOKEN];
#[cfg(all(native_dual_capture_poc, sample_pair_writer_coupling_poc))]
const _: [(); 0x7c] = [(); STATE_PAIR_BANK_0];
#[cfg(native_dual_capture_poc)]
const _: [(); 0xb4] = [(); STATE_WRITER_MODE];
#[cfg(native_dual_capture_poc)]
const _: [(); 0xb5] = [(); STATE_WRITER_PHASE];
#[cfg(native_dual_capture_poc)]
const _: [(); 0x154] = [(); STATE_NATIVE_RETAINED_PCM];
#[cfg(native_dual_capture_poc)]
const _: [(); 0x180] = [(); STATE_DEFERRED_REFRESH_VIEW];
#[cfg(all(native_dual_capture_poc, native_capture_preflight_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_CAPTURE_PREFLIGHT_PROBE];
#[cfg(all(native_dual_capture_poc, native_writer_readiness_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_WRITER_PROBE];
#[cfg(all(native_dual_capture_poc, native_writer_acquisition_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_WRITER_ACQUISITION_PROBE];
#[cfg(all(native_dual_capture_poc, native_arm_token_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_ARM_TOKEN_PROBE];
#[cfg(all(native_dual_capture_poc, native_pack_value_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_PACK_VALUE_PROBE];
#[cfg(all(native_dual_capture_poc, native_pack_correction_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_PACK_CORRECTION_PROBE];
#[cfg(all(native_dual_capture_poc, native_finalization_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_FINALIZATION_PROBE];
#[cfg(all(native_dual_capture_poc, native_publication_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_PUBLICATION_PROBE];
#[cfg(all(native_dual_capture_poc, native_terminal_publish_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_TERMINAL_PUBLISH_PROBE];
#[cfg(all(native_dual_capture_poc, native_publish_boundary_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_PUBLISH_BOUNDARY_PROBE];
#[cfg(all(native_dual_capture_poc, native_publish_slot_probe_diagnostic))]
const _: [(); 0x1a8] = [(); STATE_NATIVE_PUBLISH_SLOT_PROBE];
#[cfg(all(
    native_dual_capture_poc,
    crop_mirror_window,
    not(crop_io_trace)
))]
const _: [(); 0x1b0] = [(); WRITER_STATE_SIZE];
#[cfg(all(
    native_dual_capture_poc,
    not(crop_io_trace),
    not(crop_mirror_window),
    any(
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
        native_publish_slot_probe_diagnostic
    )
))]
const _: [(); 0x1ac] = [(); WRITER_STATE_SIZE];
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
const _: [(); 0x244] = [(); WRITER_STATE_SIZE];
#[cfg(all(
    native_dual_capture_poc,
    not(crop_io_trace),
    not(crop_mirror_window),
    not(any(
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
        native_publish_slot_probe_diagnostic
    ))
))]
const _: [(); 0x1a8] = [(); WRITER_STATE_SIZE];
#[cfg(sample_pair_writer_coupling_poc)]
const ARM_TOKEN_MAGIC: u32 = 0xa55a_0000;
#[cfg(sample_pair_writer_coupling_poc)]
const ARM_TOKEN_MAGIC_MASK: u32 = 0xffff_0000;
/// One latch per possible leader index: any adjacent pair means leaders 0..=6.
#[cfg(paired_playback_poc)]
const PLAYBACK_PAIR_COUNT: usize = 7;
#[cfg(sample_pair_identity_poc)]
const WAV_BYTE_RATE_OFFSET: usize = 0x1c;
#[cfg(sample_pair_identity_poc)]
const WAV_BYTE_RATE_STEREO_LEFT: u32 = 96_001;
#[cfg(sample_pair_identity_poc)]
const WAV_BYTE_RATE_STEREO_RIGHT: u32 = 96_002;
const SCAN_MODE_IDLE: u8 = 0;
const SCAN_MODE_FULL: u8 = 1;
const SCAN_BANK_NONE: u8 = 0xff;
const WRITER_MODE_IDLE: u8 = 0;
const WRITER_MODE_STOCK: u8 = 1;
const WRITER_MODE_CUSTOM_MONO: u8 = 2;
const WRITER_MODE_CUSTOM_STEREO: u8 = 3;
#[cfg(native_dual_capture_poc)]
const WRITER_MODE_NATIVE_STEREO: u8 = WRITER_MODE_CUSTOM_STEREO;
const WRITER_PHASE_IDLE: u8 = 0;
#[cfg(native_dual_capture_poc)]
const WRITER_PHASE_WAIT_NATIVE_NORMALIZATION: u8 = 1;
const WRITER_PHASE_PREPARE_LEFT: u8 = 3;
const WRITER_PHASE_WRITE_LEFT: u8 = 4;
const WRITER_PHASE_PREPARE_RIGHT: u8 = 6;
const WRITER_PHASE_WRITE_RIGHT: u8 = 7;
#[cfg(native_dual_capture_poc)]
const WRITER_PHASE_PREPARE_NATIVE_LEFT: u8 = WRITER_PHASE_PREPARE_LEFT;
#[cfg(native_dual_capture_poc)]
const WRITER_PHASE_WRITE_NATIVE_LEFT: u8 = WRITER_PHASE_WRITE_LEFT;
#[cfg(native_dual_capture_poc)]
const WRITER_PHASE_PREPARE_NATIVE_RIGHT: u8 = WRITER_PHASE_PREPARE_RIGHT;
#[cfg(native_dual_capture_poc)]
const WRITER_PHASE_WRITE_NATIVE_RIGHT: u8 = WRITER_PHASE_WRITE_RIGHT;
const WRITER_PHASE_COMMITTED_WAIT_UI: u8 = 9;
const WRITER_PHASE_ROLLBACK: u8 = 10;
const WRITER_PHASE_RIGHT_PUBLICATION_PENDING: u8 = 11;
const WRITER_PHASE_RIGHT_PUBLICATION_ACTIVE: u8 = 12;
#[cfg(native_slot_metadata_probe_diagnostic)]
const NATIVE_METADATA_PROBE_MAGIC: u32 = 0xd500_0000;
#[cfg(native_capture_preflight_probe_diagnostic)]
const CAPTURE_PREFLIGHT_PROBE_MAGIC: u32 = 0xd160_0000;
#[cfg(native_capture_preflight_probe_diagnostic)]
const CAPTURE_PREFLIGHT_PROBE_ENTRY: u32 = 1 << 15;
#[cfg(native_writer_readiness_probe_diagnostic)]
const NATIVE_WRITER_PROBE_MAGIC: u32 = 0xd600_0000;
#[cfg(native_writer_acquisition_probe_diagnostic)]
const NATIVE_WRITER_ACQUISITION_PROBE_MAGIC: u32 = 0xd700_0000;
#[cfg(native_arm_token_probe_diagnostic)]
const NATIVE_ARM_TOKEN_PROBE_MAGIC: u32 = 0xd800_0000;
#[cfg(native_pack_value_probe_diagnostic)]
const NATIVE_PACK_VALUE_PROBE_MAGIC: u32 = 0xd900_0000;
#[cfg(native_pack_correction_probe_diagnostic)]
const NATIVE_PACK_CORRECTION_PROBE_MAGIC: u32 = 0xda00_0000;
#[cfg(native_finalization_probe_diagnostic)]
const NATIVE_FINALIZATION_PROBE_MAGIC: u32 = 0xdb00_0000;
#[cfg(native_publication_probe_diagnostic)]
const NATIVE_PUBLICATION_PROBE_MAGIC: u32 = 0xdc00_0000;
#[cfg(native_terminal_publish_probe_diagnostic)]
const NATIVE_TERMINAL_PUBLISH_PROBE_MAGIC: u32 = 0xdd00_0000;
#[cfg(native_publish_boundary_probe_diagnostic)]
const NATIVE_PUBLISH_BOUNDARY_PROBE_MAGIC: u32 = 0xde00_0000;
#[cfg(native_publish_slot_probe_diagnostic)]
const NATIVE_PUBLISH_SLOT_PROBE_MAGIC: u32 = 0xdf00_0000;
#[cfg(native_deferred_refresh_probe_diagnostic)]
const DEFERRED_REFRESH_READY_MAGIC: u32 = 0xd0a0_0000;
#[cfg(native_deferred_refresh_probe_diagnostic)]
const NATIVE_DEFERRED_REFRESH_REPORT_MAGIC: u32 = 0xd0a1_0000;
const CLEAR_PHASE_IDLE: u8 = 0;
const CLEAR_PHASE_RIGHT: u8 = 1;
const CLEAR_PHASE_LEFT: u8 = 2;
const CLEAR_PHASE_DEFERRED: u8 = 3;
const CLEAR_PHASE_SINGLE: u8 = 4;
#[cfg(sample_cache_refresh_poc)]
const REFRESH_TOKEN_MAGIC: u32 = 0x5aa5_0000;
#[cfg(sample_cache_refresh_poc)]
const REFRESH_TOKEN_STEREO: u32 = 1 << 13;
#[cfg(sample_cache_refresh_poc)]
const REFRESH_DIAGNOSTIC_QUERY_MISS: u32 = 0xd1a1_0000;
#[cfg(sample_cache_refresh_poc)]
const REFRESH_DIAGNOSTIC_LOAD_FAILED: u32 = 0xd1a2_0000;
#[cfg(sample_cache_refresh_poc)]
const RELOAD_QUERY_MISS: u32 = 0;
#[cfg(sample_cache_refresh_poc)]
const RELOAD_FAILED: u32 = 1;
#[cfg(sample_cache_refresh_poc)]
const RELOAD_SUCCEEDED: u32 = 2;
const SYNTHETIC_CHUNK_SAMPLES: u32 = 8;
const SYNTHETIC_TOTAL_SAMPLES: u32 = 2048;
const FIRST_OUTPUT_SPAN: u32 = 0x3ea;
const OUTPUT_BLOCK_SPAN: u32 = 0x400;
const RAW_READ_MAX_WORDS: u32 = 0x100;
#[cfg(native_dual_capture_poc)]
const DSP_PAGE_FRAMES: u32 = 0x1000;
const RAW_SCRATCH_WORD_OFFSET: usize = 0x400;
const SAMPLE_VIEW_HELD_MASK: usize = 0x60;
const SAMPLE_VIEW_ANCHOR_PAD: usize = 0x62;
const SAMPLE_VIEW_ARMED_LEFT: usize = 0x63;
const SAMPLE_VIEW_EXTENSION_MAGIC: usize = 0x64;
const SAMPLE_VIEW_MAGIC: u32 = 0x5041_4952;
const SAMPLE_VIEW_NO_PAD: u8 = 0x20;
const SAMPLE_VIEW_NO_ARM: u8 = 0xff;
#[cfg(paired_playback_poc)]
const PLAYBACK_LATCH_ACTIVE: u8 = 0x80;
#[cfg(paired_playback_poc)]
const PLAYBACK_LATCH_NOTE_MASK: u8 = 0x7f;
#[cfg(paired_playback_poc)]
const PLAYBACK_LEADER_TRACK: u32 = 0x10;

const DESCRIPTOR_TYPE: usize = 0x00;
const DESCRIPTOR_SLOT: usize = 0x04;
const DESCRIPTOR_PACK: usize = 0x08;

const ASYNC_OPEN: usize = 0x0804_6099;
const ASYNC_WRITE: usize = 0x0804_607b;
const ASYNC_CLOSE: usize = 0x0804_60c9;
const RESET_WRITER: usize = 0x0802_f327;
const CONTENT_TYPE_TO_INDEX: usize = 0x0802_e9d1;
const CONTENT_INDEX_GET_ITEM: usize = 0x0804_56d9;
const CONTENT_STORE_DELETE_ITEM: usize = 0x0802_e8e5;
const SAMPLING_MANAGER: *const u32 = 0x2000_0828 as *const u32;
const ACTIVE_CONTENT_STORE: *const u32 = 0x2000_6e24 as *const u32;
const FILESYSTEM_INTERFACE: *const u32 = 0x2000_3750 as *const u32;
#[cfg(native_dual_capture_poc)]
const DSP_PROGRAM_STATE: *const u32 = 0x2000_0310 as *const u32;
#[cfg(any(sample_cache_refresh_poc, sample_pair_identity_poc))]
const SAVE_CALLBACK_REGISTRATION: *const u32 = 0x2000_6ed8 as *const u32;

type SourceTotal = unsafe extern "C" fn(*mut u8, u16) -> u32;
type SourceStatus = unsafe extern "C" fn(*mut u8, u16) -> u32;
type SourcePosition = unsafe extern "C" fn(*mut u8, u16, u32) -> u32;
type SourceRead = unsafe extern "C" fn(*mut u8, u16, u32, *mut i16, u32) -> u32;
type SourceChunkRead = unsafe extern "C" fn(*mut u8, u32, *mut i16, u32) -> u64;
type SourceBuffer = unsafe extern "C" fn(*mut u8, u16) -> *mut i16;
type AsyncOpen = unsafe extern "C" fn(*mut u8, usize, *mut u8) -> u32;
type AsyncWrite =
    unsafe extern "C" fn(*mut u8, u32, *mut i16, u32, usize, *mut u8) -> u32;
type AsyncClose = unsafe extern "C" fn(*mut u8, usize, *mut u8) -> u32;
type ResetWriter = unsafe extern "C" fn(*mut u8);
type Completion = unsafe extern "C" fn(u32, u32);
type FatFsWrite = unsafe extern "C" fn(*mut u8, *mut i16, u32, *mut u32) -> u32;
type WriterStateConstructor = unsafe extern "C" fn(*mut u8, *mut u8) -> *mut u8;
type StockStartRequest =
    unsafe extern "C" fn(*mut u8, u32, *const u8, usize, u32) -> u32;
type StockPump = unsafe extern "C" fn(*mut u8, *mut u8, u32, u32) -> u32;
type StockFileCallback =
    unsafe extern "C" fn(u32, u32, u32, u32, u32, *mut u8);
type StockClearCoordinator = unsafe extern "C" fn(*mut u8, *const u8, *mut u8);
type SaveControllerRequestSlot = unsafe extern "C" fn(*mut u8, u32);
type AsyncStorageRequest = unsafe extern "C" fn(*const u8, usize, *mut u8) -> u32;
type CompletionObjectCall = unsafe extern "C" fn(*mut u8, u32);
type ObservableValue = unsafe extern "C" fn(u32, u32) -> *mut u8;
type FirmwareMethod4 = unsafe extern "C" fn(u32, u32, u32, u32) -> u32;
type SampleSlotOccupied = unsafe extern "C" fn(*const u8, u32) -> u32;
#[cfg(native_dual_capture_poc)]
type CaptureSourceStart = unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u32;
#[cfg(native_dual_capture_poc)]
type CaptureSourceClear = unsafe extern "C" fn(*mut u8, u32);
#[cfg(native_dual_capture_poc)]
type DspWrite = unsafe extern "C" fn(u32, u32, u32);
#[cfg(native_dual_capture_poc)]
type DspRead = unsafe extern "C" fn(u32, u32) -> u32;
#[cfg(native_dual_capture_poc)]
type SchedulerYield = unsafe extern "C" fn();
#[cfg(native_dual_capture_poc)]
type QuerySampleSlot = unsafe extern "C" fn(u32, *const u32) -> u32;
#[cfg(sample_cache_refresh_poc)]
type InvalidateSampleSlot = unsafe extern "C" fn(u32);
#[cfg(sample_cache_refresh_poc)]
type LoadSampleSlot = unsafe extern "C" fn(u32) -> u32;
#[cfg(any(sample_cache_refresh_poc, sample_pair_identity_poc))]
type PublishSampleSlot = unsafe extern "C" fn(*mut u8, u32) -> u32;
#[cfg(paired_playback_poc)]
type TrackParameterGet = unsafe extern "C" fn(u32, u32) -> u32;
#[cfg(any(auto_pan_poc, stereo_position_poc))]
type TrackParameterSet = unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u32;
#[cfg(stereo_position_poc)]
type TrackParameterDspUpdate = unsafe extern "C" fn(u32, u32, u32, u32) -> u64;
#[cfg(stereo_position_poc)]
type GridRender = unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u64;
#[cfg(stereo_position_poc)]
type ObservableSet = unsafe extern "C" fn(*mut u8, u32, u32) -> u32;
#[cfg(stereo_position_poc)]
type MixerPadPress = unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u32;
#[cfg(stereo_position_poc)]
type MixerKnobInput = unsafe extern "C" fn(*mut u8, u32, i32, u32) -> u32;
#[cfg(mixer_volume_probe)]
type FxKnobInput = unsafe extern "C" fn(*mut u8, u32, i32);
#[cfg(stereo_position_poc)]
type FxRender = unsafe extern "C" fn(*mut u8) -> u32;
#[cfg(mixer_volume_probe)]
type RootKnobInput = unsafe extern "C" fn(*mut u8, u32, i32);
#[cfg(stereo_position_poc)]
type SidechainPadInput = unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u32;
#[cfg(paired_playback_poc)]
type TrackNoteOnAdapter = unsafe extern "C" fn(*mut u8, u32, u32, u32);
#[cfg(paired_playback_poc)]
type TrackNoteOffAdapter = unsafe extern "C" fn(*mut u8, u32, u32);

#[cfg(all(
    sample_pair_identity_poc,
    not(all(sample_pair_led_poc, safe_pair_guard))
))]
compile_error!("sample_pair_identity_poc requires sample_pair_led_poc and safe_pair_guard");

#[cfg(all(sample_pair_identity_poc, not(sample_pair_writer_coupling_poc)))]
compile_error!("sample_pair_identity_poc requires sample_pair_writer_coupling_poc");

#[cfg(all(sample_pair_identity_poc, not(sample_cache_refresh_poc)))]
compile_error!("sample_pair_identity_poc requires sample_cache_refresh_poc");

#[cfg(all(
    sample_pair_identity_poc,
    any(
        synthetic_commit_probe_diagnostic,
        single_pass_left_diagnostic,
        single_pass_right_diagnostic
    )
))]
compile_error!("sample_pair_identity_poc conflicts with synthetic and single-pass diagnostics");

#[cfg(all(sample_pair_gesture_poc, not(sample_pair_led_poc)))]
compile_error!("sample_pair_gesture_poc requires sample_pair_led_poc");

#[cfg(all(
    sample_pair_writer_coupling_poc,
    not(all(
        sample_pair_gesture_poc,
        sample_pair_identity_poc,
        safe_pair_guard
    ))
))]
compile_error!(
    "sample_pair_writer_coupling_poc requires gesture, identity, and safe-pair POCs"
);

#[cfg(all(
    paired_playback_poc,
    not(all(
        sample_pair_led_poc,
        sample_pair_identity_poc,
        sample_pair_writer_coupling_poc
    ))
))]
compile_error!(
    "paired_playback_poc requires LED, identity, and writer-coupling POCs"
);

#[cfg(all(pan_parameter_probe, not(paired_playback_poc)))]
compile_error!("pan_parameter_probe requires paired_playback_poc");

#[cfg(all(auto_pan_poc, not(paired_playback_poc)))]
compile_error!("auto_pan_poc requires paired_playback_poc");

#[cfg(all(stereo_position_poc, not(paired_playback_poc)))]
compile_error!("stereo_position_poc requires paired_playback_poc");

#[cfg(all(stereo_position_poc, auto_pan_poc))]
compile_error!("stereo_position_poc replaces auto_pan_poc");

#[cfg(all(mixer_volume_probe, not(stereo_position_poc)))]
compile_error!("mixer_volume_probe requires stereo_position_poc");

#[cfg(all(sample_pair_writer_coupling_poc, force_stereo_diagnostic))]
compile_error!("sample_pair_writer_coupling_poc conflicts with force_stereo_diagnostic");

#[cfg(all(sample_cache_refresh_poc, not(sample_pair_writer_coupling_poc)))]
compile_error!("sample_cache_refresh_poc requires sample_pair_writer_coupling_poc");

#[cfg(all(
    native_dual_capture_poc,
    not(all(
        safe_pair_guard,
        sample_pair_identity_poc,
        sample_pair_writer_coupling_poc,
        sample_cache_refresh_poc
    ))
))]
compile_error!(
    "native_dual_capture_poc requires safe-pair, identity, writer-coupling, and cache POCs"
);

#[cfg(all(
    native_capture_preflight_probe_diagnostic,
    not(all(native_dual_capture_poc, sample_pair_led_poc))
))]
compile_error!(
    "native_capture_preflight_probe_diagnostic requires native dual capture and pair LEDs"
);

#[cfg(all(
    native_writer_readiness_probe_diagnostic,
    not(all(native_dual_capture_poc, sample_pair_led_poc))
))]
compile_error!(
    "native_writer_readiness_probe_diagnostic requires native dual capture and pair LEDs"
);

#[cfg(all(
    native_writer_acquisition_probe_diagnostic,
    not(all(native_dual_capture_poc, sample_pair_led_poc))
))]
compile_error!(
    "native_writer_acquisition_probe_diagnostic requires native dual capture and pair LEDs"
);

#[cfg(all(
    native_arm_token_probe_diagnostic,
    not(all(
        native_dual_capture_poc,
        sample_pair_led_poc,
        sample_pair_writer_coupling_poc
    ))
))]
compile_error!(
    "native_arm_token_probe_diagnostic requires native dual capture, pair LEDs, and writer coupling"
);

#[cfg(all(
    native_pack_value_probe_diagnostic,
    not(all(
        native_dual_capture_poc,
        sample_pair_led_poc,
        sample_pair_writer_coupling_poc
    ))
))]
compile_error!(
    "native_pack_value_probe_diagnostic requires native dual capture, pair LEDs, and writer coupling"
);

#[cfg(all(
    native_pack_correction_probe_diagnostic,
    not(all(
        native_dual_capture_poc,
        sample_pair_led_poc,
        sample_pair_writer_coupling_poc
    ))
))]
compile_error!(
    "native_pack_correction_probe_diagnostic requires native dual capture, pair LEDs, and writer coupling"
);

#[cfg(all(
    native_finalization_probe_diagnostic,
    not(all(
        native_dual_capture_poc,
        sample_pair_led_poc,
        sample_pair_writer_coupling_poc
    ))
))]
compile_error!(
    "native_finalization_probe_diagnostic requires native dual capture, pair LEDs, and writer coupling"
);

#[cfg(all(
    native_publication_probe_diagnostic,
    not(all(
        native_dual_capture_poc,
        sample_pair_led_poc,
        sample_pair_writer_coupling_poc,
        sample_cache_refresh_poc
    ))
))]
compile_error!(
    "native_publication_probe_diagnostic requires native dual capture, pair LEDs, writer coupling, and cache refresh"
);

#[cfg(all(
    native_terminal_publish_probe_diagnostic,
    not(all(
        native_dual_capture_poc,
        sample_pair_led_poc,
        sample_pair_writer_coupling_poc,
        sample_cache_refresh_poc
    ))
))]
compile_error!(
    "native_terminal_publish_probe_diagnostic requires native dual capture, pair LEDs, writer coupling, and cache refresh"
);

#[cfg(all(
    native_publish_boundary_probe_diagnostic,
    not(all(
        native_dual_capture_poc,
        sample_pair_led_poc,
        sample_pair_writer_coupling_poc,
        sample_cache_refresh_poc
    ))
))]
compile_error!(
    "native_publish_boundary_probe_diagnostic requires native dual capture, pair LEDs, writer coupling, and cache refresh"
);

#[cfg(all(
    native_publish_slot_probe_diagnostic,
    not(all(
        native_dual_capture_poc,
        sample_pair_led_poc,
        sample_pair_writer_coupling_poc,
        sample_cache_refresh_poc
    ))
))]
compile_error!(
    "native_publish_slot_probe_diagnostic requires native dual capture, pair LEDs, writer coupling, and cache refresh"
);

#[cfg(all(
    native_deferred_refresh_probe_diagnostic,
    not(all(
        native_dual_capture_poc,
        sample_pair_led_poc,
        sample_pair_identity_poc,
        sample_pair_gesture_poc,
        sample_pair_writer_coupling_poc,
        sample_cache_refresh_poc
    ))
))]
compile_error!(
    "native_deferred_refresh_probe_diagnostic requires native dual capture and the complete sample-pair stack"
);

#[cfg(any(
    all(
        native_capture_preflight_probe_diagnostic,
        native_writer_readiness_probe_diagnostic
    ),
    all(
        native_capture_preflight_probe_diagnostic,
        native_writer_acquisition_probe_diagnostic
    ),
    all(
        native_writer_readiness_probe_diagnostic,
        native_writer_acquisition_probe_diagnostic
    ),
    all(
        native_capture_preflight_probe_diagnostic,
        native_arm_token_probe_diagnostic
    ),
    all(
        native_writer_readiness_probe_diagnostic,
        native_arm_token_probe_diagnostic
    ),
    all(
        native_writer_acquisition_probe_diagnostic,
        native_arm_token_probe_diagnostic
    ),
    all(
        native_capture_preflight_probe_diagnostic,
        native_pack_value_probe_diagnostic
    ),
    all(
        native_writer_readiness_probe_diagnostic,
        native_pack_value_probe_diagnostic
    ),
    all(
        native_writer_acquisition_probe_diagnostic,
        native_pack_value_probe_diagnostic
    ),
    all(
        native_arm_token_probe_diagnostic,
        native_pack_value_probe_diagnostic
    ),
    all(
        native_capture_preflight_probe_diagnostic,
        native_pack_correction_probe_diagnostic
    ),
    all(
        native_writer_readiness_probe_diagnostic,
        native_pack_correction_probe_diagnostic
    ),
    all(
        native_writer_acquisition_probe_diagnostic,
        native_pack_correction_probe_diagnostic
    ),
    all(
        native_arm_token_probe_diagnostic,
        native_pack_correction_probe_diagnostic
    ),
    all(
        native_pack_value_probe_diagnostic,
        native_pack_correction_probe_diagnostic
    ),
    all(
        native_capture_preflight_probe_diagnostic,
        native_finalization_probe_diagnostic
    ),
    all(
        native_writer_readiness_probe_diagnostic,
        native_finalization_probe_diagnostic
    ),
    all(
        native_writer_acquisition_probe_diagnostic,
        native_finalization_probe_diagnostic
    ),
    all(
        native_arm_token_probe_diagnostic,
        native_finalization_probe_diagnostic
    ),
    all(
        native_pack_value_probe_diagnostic,
        native_finalization_probe_diagnostic
    ),
    all(
        native_pack_correction_probe_diagnostic,
        native_finalization_probe_diagnostic
    ),
    all(
        native_capture_preflight_probe_diagnostic,
        native_publication_probe_diagnostic
    ),
    all(
        native_writer_readiness_probe_diagnostic,
        native_publication_probe_diagnostic
    ),
    all(
        native_writer_acquisition_probe_diagnostic,
        native_publication_probe_diagnostic
    ),
    all(
        native_arm_token_probe_diagnostic,
        native_publication_probe_diagnostic
    ),
    all(
        native_pack_value_probe_diagnostic,
        native_publication_probe_diagnostic
    ),
    all(
        native_pack_correction_probe_diagnostic,
        native_publication_probe_diagnostic
    ),
    all(
        native_finalization_probe_diagnostic,
        native_publication_probe_diagnostic
    ),
    all(
        native_capture_preflight_probe_diagnostic,
        native_terminal_publish_probe_diagnostic
    ),
    all(
        native_writer_readiness_probe_diagnostic,
        native_terminal_publish_probe_diagnostic
    ),
    all(
        native_writer_acquisition_probe_diagnostic,
        native_terminal_publish_probe_diagnostic
    ),
    all(
        native_arm_token_probe_diagnostic,
        native_terminal_publish_probe_diagnostic
    ),
    all(
        native_pack_value_probe_diagnostic,
        native_terminal_publish_probe_diagnostic
    ),
    all(
        native_pack_correction_probe_diagnostic,
        native_terminal_publish_probe_diagnostic
    ),
    all(
        native_finalization_probe_diagnostic,
        native_terminal_publish_probe_diagnostic
    ),
    all(
        native_publication_probe_diagnostic,
        native_terminal_publish_probe_diagnostic
    ),
    all(
        native_capture_preflight_probe_diagnostic,
        native_publish_boundary_probe_diagnostic
    ),
    all(
        native_writer_readiness_probe_diagnostic,
        native_publish_boundary_probe_diagnostic
    ),
    all(
        native_writer_acquisition_probe_diagnostic,
        native_publish_boundary_probe_diagnostic
    ),
    all(
        native_arm_token_probe_diagnostic,
        native_publish_boundary_probe_diagnostic
    ),
    all(
        native_pack_value_probe_diagnostic,
        native_publish_boundary_probe_diagnostic
    ),
    all(
        native_pack_correction_probe_diagnostic,
        native_publish_boundary_probe_diagnostic
    ),
    all(
        native_finalization_probe_diagnostic,
        native_publish_boundary_probe_diagnostic
    ),
    all(
        native_publication_probe_diagnostic,
        native_publish_boundary_probe_diagnostic
    ),
    all(
        native_terminal_publish_probe_diagnostic,
        native_publish_boundary_probe_diagnostic
    ),
    all(
        native_capture_preflight_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    ),
    all(
        native_writer_readiness_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    ),
    all(
        native_writer_acquisition_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    ),
    all(
        native_arm_token_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    ),
    all(
        native_pack_value_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    ),
    all(
        native_pack_correction_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    ),
    all(
        native_finalization_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    ),
    all(
        native_publication_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    ),
    all(
        native_terminal_publish_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    ),
    all(
        native_publish_boundary_probe_diagnostic,
        native_publish_slot_probe_diagnostic
    )
))]
compile_error!("native writer diagnostics are mutually exclusive");

#[cfg(sample_pair_writer_coupling_poc)]
#[no_mangle]
pub extern "C" fn sample_pair_writer_coupling_marker() {}

#[cfg(sample_cache_refresh_poc)]
#[no_mangle]
pub extern "C" fn sample_cache_refresh_marker() {}

#[cfg(paired_playback_poc)]
#[no_mangle]
pub extern "C" fn paired_playback_marker() {}

#[cfg(stereo_position_poc)]
#[no_mangle]
pub extern "C" fn stereo_position_marker() {}

#[cfg(sample_pair_identity_poc)]
core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb
    .section .text.writer_state_prepare_allocation,"ax",%progbits
    .global writer_state_prepare_allocation
    .type writer_state_prepare_allocation,%function
    .thumb_func
writer_state_prepare_allocation:
    str r1, [r4, #8]
    movw r0, #{state_size}
    bx lr
    .size writer_state_prepare_allocation, .-writer_state_prepare_allocation
"#,
    state_size = const WRITER_STATE_SIZE,
);

/// True for the follower of an enabled pair. The leader's sequence drives both
/// halves (its note-ons and note-offs are mirrored by calling stock directly),
/// so anything reaching the hooks for the follower is its own pattern, which is
/// kept but silent while paired and plays again once the pair is split.
#[cfg(paired_playback_poc)]
unsafe fn paired_follower_track(track: u32) -> bool {
    track > PLAYBACK_LEADER_TRACK
        && track < PLAYBACK_LEADER_TRACK + 8
        && paired_pair_enabled(track - 1)
}

#[cfg(paired_playback_poc)]
#[no_mangle]
pub unsafe extern "C" fn paired_playback_note_on(
    receiver: *mut u8,
    track: u32,
    note: u32,
    velocity: u32,
) {
    let stock = function::<TrackNoteOnAdapter>(0x0803_1a19);

    // Probe: every sample trigger in the image funnels through this vtable slot
    // (`FUN_08031A18` -> `FUN_08046780` -> `FUN_08040778`), so if the Sample Rec
    // page's slot preview uses the voice engine at all, it is observable here.
    // Records the channel and note of the most recent note-on.
    //   pad 1      : a note-on was observed
    //   pads 2-4   : channel & 7
    //   pad 5      : channel >= 8
    //   pads 6-12  : note & 0x7f
    //   pad 13     : channel == 0xff (metronome)
    #[cfg(all(native_dual_capture_poc, voice_channel_probe))]
    {
        // `pair_state()` performs no validation - it simply returns `manager[4]`
        // after null-checking the manager. The null check below is the same
        // guard the latch code immediately underneath already relies on, which
        // has been proven safe on hardware across every shipped build.
        let pstate = pair_state();
        if !pstate.is_null() {
            let mut bits = (track as u16) & 7;
            if track >= 8 {
                bits |= 1 << 3;
            }
            bits |= ((note as u16) & 0x7f) << 4;
            if track == 0xff {
                bits |= 1 << 11;
            }
            write_u16(pstate, STATE_VOICE_PROBE, VOICE_PROBE_SEEN | bits);
        }
    }

    if paired_follower_track(track) {
        return;
    }
    let pair = playback_pair_index(track);
    let state = pair.map(|_| pair_state()).unwrap_or(core::ptr::null_mut());
    if let Some(pair) = pair {
        if !state.is_null() {
            let latch = read_u8(state, STATE_PLAYBACK_LATCHES + pair);
            if latch & PLAYBACK_LATCH_ACTIVE != 0 {
                function::<TrackNoteOffAdapter>(0x0803_1a23)(
                    receiver,
                    track + 1,
                    (latch & PLAYBACK_LATCH_NOTE_MASK) as u32,
                );
                write_u8(state, STATE_PLAYBACK_LATCHES + pair, 0);
            }
        }
    }
    let mirror = pair.is_some()
        && note <= PLAYBACK_LATCH_NOTE_MASK as u32
        && paired_pair_enabled(track);
    #[cfg(auto_pan_poc)]
    if mirror {
        enforce_stereo_pan(receiver, track);
    }
    #[cfg(stereo_position_poc)]
    if mirror {
        initialize_stereo_position(receiver, track);
    }
    stock(receiver, track, note, velocity);
    if mirror {
        stock(receiver, track + 1, note, velocity);
        if let Some(pair) = pair {
            write_u8(
                state,
                STATE_PLAYBACK_LATCHES + pair,
                PLAYBACK_LATCH_ACTIVE | (note as u8 & PLAYBACK_LATCH_NOTE_MASK),
            );
        }
    }
}

#[cfg(sample_cache_refresh_poc)]
unsafe fn queue_stereo_right_publication(right: u8) -> bool {
    const SAVE_CONTROLLER_REQUEST_SLOT: usize = 0x0803_5057;

    let controller = read_volatile(SAVE_CALLBACK_REGISTRATION.add(1)) as *mut u8;
    if controller.is_null() {
        return false;
    }
    function::<SaveControllerRequestSlot>(SAVE_CONTROLLER_REQUEST_SLOT)(
        controller,
        right as u32,
    );
    sample_slot_save_pending(right)
}

#[cfg(stereo_position_poc)]
unsafe fn initialize_stereo_position(receiver: *mut u8, leader_track: u32) {
    const TRACK_PARAMETER_GET: usize = 0x0804_6871;
    const TRACK_PARAMETER_SET: usize = 0x0803_1a3b;
    const PAN_PARAMETER: u32 = 17;

    let get = function::<TrackParameterGet>(TRACK_PARAMETER_GET);
    let set = function::<TrackParameterSet>(TRACK_PARAMETER_SET);
    let follower_track = leader_track + 1;
    let follower_pan = get(follower_track, PAN_PARAMETER);
    let mut position = get(leader_track, PAN_PARAMETER).min(127);
    if position == 0 && follower_pan == 127 {
        position = 64;
    }
    set(receiver, leader_track, PAN_PARAMETER, position);
}

/// Resolves a track's parameter object, through which pan can be published
/// without holding a note receiver.
#[cfg(stereo_position_poc)]
unsafe fn track_parameter_object(track_index: u32) -> *mut u8 {
    const PARAMETER_REGISTRATION: *const u32 = 0x2000_6eb8 as *const u32;
    const TRACK_OBJECT_BASE: usize = 0x1a4;

    if track_index >= 8 {
        return core::ptr::null_mut();
    }
    let owner = read_volatile(PARAMETER_REGISTRATION.add(1)) as *const u8;
    if owner.is_null() {
        return core::ptr::null_mut();
    }
    read_u32(
        owner,
        TRACK_OBJECT_BASE + ((PLAYBACK_LEADER_TRACK + track_index) as usize) * 4,
    ) as *mut u8
}

/// Keeps the follower's pan consistent with the pairs that currently exist.
///
/// The follower's pan is not a user setting: it is mechanically derived from
/// the leader's stereo position, so it must be undone when the pair that
/// imposed it goes away. Without this, a track that was a follower keeps its
/// hard-right pan of 127, and if it then becomes a leader itself
/// `stereo_left_pan(127)` is also 127, so the new pair plays hard right instead
/// of hard left.
///
/// Dissolution is serviced before formation so that a track moving straight
/// from follower to leader is restored to centre before its new pair reads it.
/// Neither path ever writes the leader's own position, so a deliberately placed
/// stereo image survives, including across a boot where every existing pair
/// looks newly formed.
#[cfg(stereo_position_poc)]
unsafe fn service_pair_positions() {
    const TRACK_PARAMETER_GET: usize = 0x0804_6871;
    const STOCK_OBSERVABLE_SET: usize = 0x0804_65dd;
    const PAN_PARAMETER: u32 = 17;
    const STEREO_POSITION_CENTRE: u32 = 64;

    let state = pair_state();
    if state.is_null() {
        return;
    }
    let current = current_pair_mask();
    let previous = read_u8(state, STATE_PAIR_POSITION_MASK);
    if current == previous {
        return;
    }
    write_u8(state, STATE_PAIR_POSITION_MASK, current);
    let dissolved = previous & !current;
    write_u8(
        state,
        STATE_PAIR_VOLUME_PENDING,
        read_u8(state, STATE_PAIR_VOLUME_PENDING) | dissolved,
    );

    let set = function::<ObservableSet>(STOCK_OBSERVABLE_SET);
    let get = function::<TrackParameterGet>(TRACK_PARAMETER_GET);
    let mut leader = 0u32;
    while leader < 7 {
        let bit = 1u8 << leader;
        if dissolved & bit != 0 {
            let object = track_parameter_object(leader + 1);
            if !object.is_null() {
                set(object, PAN_PARAMETER, STEREO_POSITION_CENTRE);
            }
        }
        leader += 1;
    }
    let mut leader = 0u32;
    while leader < 7 {
        let bit = 1u8 << leader;
        if current & bit != 0 && previous & bit == 0 {
            let object = track_parameter_object(leader + 1);
            if !object.is_null() {
                let position = get(PLAYBACK_LEADER_TRACK + leader, PAN_PARAMETER).min(127);
                set(object, PAN_PARAMETER, stereo_right_pan(position));
            }
        }
        leader += 1;
    }
}

/// Returns a dissolved pair's follower to the default volume, then keeps every
/// live follower at its leader's level, through the latched mixer controller.
///
/// A follower's level is slaved to its leader for as long as the pair lasts,
/// so on dissolution it holds the leader's level rather than its own; it is
/// reset to the patched blank-project default (track parameter 16, template
/// byte 0x0806880A). Writes go through the stock mixer handler, the
/// same path the hardware-validated coupling uses, and only while the mixer is
/// in volume mode - in the other modes the same call edits pan or a page
/// field. Work found in the wrong mode stays pending and is retried.
#[cfg(stereo_position_poc)]
unsafe fn service_pair_volume() {
    const STOCK_MIXER_KNOB: usize = 0x0805_f2f9;
    const VOLUME_VALUES: usize = 0x18;
    const MIXER_MODE: usize = 0x20;
    const MIXER_SUBMODE: usize = 0x24;
    const VOLUME_DEFAULT: i32 = 89;

    let state = pair_state();
    if state.is_null() {
        return;
    }
    let controller = read_u32(state, STATE_PAIR_VOLUME_CONTROLLER) as *mut u8;
    if controller.is_null() {
        return;
    }
    // Stock takes the volume path only when bit 0 of `+0x24` is clear and the
    // `+0x20` observable is zero (0x0805F30E, 0x0805F3BC).
    let read = function::<ObservableValue>(0x0803_fc59);
    let submode = read_u32(controller, MIXER_SUBMODE);
    let mode = read_u32(controller, MIXER_MODE);
    let values = read_u32(controller, VOLUME_VALUES);
    if submode == 0 || mode == 0 || values == 0 {
        return;
    }
    let submode = read(submode, 0);
    let mode = read(mode, 0);
    if submode.is_null() || mode.is_null() || *submode & 1 != 0 || *mode != 0 {
        return;
    }

    let stock = function::<MixerKnobInput>(STOCK_MIXER_KNOB);
    let pending = read_u8(state, STATE_PAIR_VOLUME_PENDING);
    write_u8(state, STATE_PAIR_VOLUME_PENDING, 0);
    let current = current_pair_mask();
    let mut leader = 0u32;
    while leader < 7 {
        let bit = 1u8 << leader;
        let follower = read(values, leader + 1);
        if !follower.is_null() {
            let mut target = *follower as i32;
            if pending & bit != 0 {
                target = VOLUME_DEFAULT;
            }
            if current & bit != 0 {
                let value = read(values, leader);
                if !value.is_null() {
                    target = *value as i32;
                }
            }
            let delta = target - *follower as i32;
            if delta != 0 {
                stock(controller, leader + 1, delta, 0);
            }
        }
        leader += 1;
    }
}

#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn stereo_position_observable_set(
    object: *mut u8,
    parameter: u32,
    value: u32,
) -> u32 {
    const STOCK_OBSERVABLE_SET: usize = 0x0804_65dd;
    const PARAMETER_REGISTRATION: *const u32 = 0x2000_6eb8 as *const u32;
    const RECEIVER_ID: usize = 8;
    const TRACK_OBJECT_BASE: usize = 0x1a4;
    const PAN_PARAMETER: u32 = 17;

    let stock = function::<ObservableSet>(STOCK_OBSERVABLE_SET);
    if object.is_null() || parameter != PAN_PARAMETER {
        return stock(object, parameter, value);
    }
    let track = read_u8(object, RECEIVER_ID) as u32;
    if !(PLAYBACK_LEADER_TRACK..PLAYBACK_LEADER_TRACK + 8).contains(&track) {
        return stock(object, parameter, value);
    }
    let Some(leader_index) = paired_track_leader_index(track - PLAYBACK_LEADER_TRACK)
    else {
        return stock(object, parameter, value);
    };
    let leader_track = PLAYBACK_LEADER_TRACK + leader_index;
    if track != leader_track {
        return 0;
    }
    let leader_value = value.min(127);
    let result = stock(object, parameter, leader_value);
    let follower_value = stereo_right_pan(leader_value);
    let track_owner = read_volatile(PARAMETER_REGISTRATION.add(1)) as *const u8;
    if !track_owner.is_null() {
        let follower_object =
            read_u32(track_owner, TRACK_OBJECT_BASE + ((leader_track + 1) as usize) * 4)
                as *mut u8;
        if !follower_object.is_null() {
            stock(follower_object, parameter, follower_value);
        }
    }
    result
}

#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn stereo_position_dsp_update(
    track_index: u32,
    parameter: u32,
    value: u32,
    context: u32,
) -> u64 {
    const STOCK_DSP_UPDATE: usize = 0x0804_070f;
    const PAN_PARAMETER: u32 = 17;

    let stock = function::<TrackParameterDspUpdate>(STOCK_DSP_UPDATE);
    if parameter == PAN_PARAMETER && track_index < 8 {
        // Only the leader's own pan is narrowed to the left half of the field;
        // the follower is driven explicitly by `stereo_position_observable_set`.
        if paired_pair_enabled(PLAYBACK_LEADER_TRACK + track_index) {
            return stock(track_index, parameter, stereo_left_pan(value.min(127)), context);
        }
    }
    stock(track_index, parameter, value, context)
}

#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn stereo_position_mixer_knob(
    controller: *mut u8,
    knob: u32,
    delta: i32,
    context: u32,
) -> u32 {
    const STOCK_MIXER_KNOB: usize = 0x0805_f2f9;
    const VOLUME_VALUES: usize = 0x18;
    const MIXER_MODE: usize = 0x20;

    let stock = function::<MixerKnobInput>(STOCK_MIXER_KNOB);
    if controller.is_null() || knob >= 8 {
        return stock(controller, knob, delta, context);
    }
    let state = pair_state();
    if !state.is_null() {
        write_u32(state, STATE_PAIR_VOLUME_CONTROLLER, controller as u32);
    }
    let Some(leader) = paired_track_leader_index(knob) else {
        return stock(controller, knob, delta, context);
    };

    let mode = read_u32(controller, MIXER_MODE);
    if mode == 0 {
        return stock(controller, leader, delta, context);
    }
    let mode_value = function::<ObservableValue>(0x0803_fc59)(mode, 0);
    if mode_value.is_null() || *mode_value != 0 {
        return stock(controller, leader, delta, context);
    }

    let result = stock(controller, leader, delta, context);
    let values = read_u32(controller, VOLUME_VALUES);
    if values != 0 {
        let read = function::<ObservableValue>(0x0803_fc59);
        let leader_value = read(values, leader);
        let follower_value = read(values, leader + 1);
        if !leader_value.is_null() && !follower_value.is_null() {
            let follower_delta = *leader_value as i32 - *follower_value as i32;
            if follower_delta != 0 {
                stock(controller, leader + 1, follower_delta, context);
            }
        }
    }
    result
}

#[cfg(mixer_volume_probe)]
unsafe fn mixer_volume_probe_value(controller: *const u8, offset: usize, knob: u32) -> u8 {
    let source = read_u32(controller, offset);
    if source == 0 || knob >= 8 {
        return 0xff;
    }
    let value = function::<ObservableValue>(0x0803_fc59)(source, knob);
    if value.is_null() {
        0xff
    } else {
        *value
    }
}

#[cfg(mixer_volume_probe)]
unsafe fn mixer_volume_probe_entry(
    controller: *mut u8,
    knob: u32,
    delta: i32,
    context: u32,
    path: u8,
) -> u32 {
    const STOCK_MIXER_KNOB: usize = 0x0805_f2f9;
    const VTABLE_PATH: u8 = 1;

    let state = pair_state();
    let mode = if controller.is_null() {
        0
    } else {
        read_u32(controller, 0x20)
    };
    let mode_value = if mode == 0 {
        0xff
    } else {
        let value = function::<ObservableValue>(0x0803_fc59)(mode, 0);
        if value.is_null() { 0xff } else { *value }
    };
    let before_18 = if controller.is_null() {
        0xff
    } else {
        mixer_volume_probe_value(controller, 0x18, knob)
    };
    let before_1c = if controller.is_null() {
        0xff
    } else {
        mixer_volume_probe_value(controller, 0x1c, knob)
    };
    if !state.is_null() {
        let flag = if path == VTABLE_PATH { 1 } else { 2 };
        write_u8(
            state,
            STATE_MIXER_PROBE_FLAGS,
            read_u8(state, STATE_MIXER_PROBE_FLAGS) | flag,
        );
        write_u8(state, STATE_MIXER_PROBE_PATH, path);
        write_u8(state, STATE_MIXER_PROBE_MODE, mode_value);
        write_u8(state, STATE_MIXER_PROBE_KNOB, knob as u8);
        write_u8(
            state,
            STATE_MIXER_PROBE_DELTA,
            if delta < 0 { 1 } else if delta > 0 { 2 } else { 0 },
        );
        write_u8(state, STATE_MIXER_PROBE_18_BEFORE, before_18);
        write_u8(state, STATE_MIXER_PROBE_1C_BEFORE, before_1c);
        let count_offset = if path == VTABLE_PATH {
            STATE_MIXER_PROBE_VTABLE_COUNT
        } else {
            STATE_MIXER_PROBE_BRANCH_COUNT
        };
        write_u8(state, count_offset, read_u8(state, count_offset).wrapping_add(1));
    }

    let result = function::<MixerKnobInput>(STOCK_MIXER_KNOB)(
        controller,
        knob,
        delta,
        context,
    );
    if !state.is_null() && !controller.is_null() {
        write_u8(
            state,
            STATE_MIXER_PROBE_18_AFTER,
            mixer_volume_probe_value(controller, 0x18, knob),
        );
        write_u8(
            state,
            STATE_MIXER_PROBE_1C_AFTER,
            mixer_volume_probe_value(controller, 0x1c, knob),
        );
    }
    result
}

#[cfg(mixer_volume_probe)]
#[no_mangle]
pub unsafe extern "C" fn mixer_volume_vtable_probe(
    controller: *mut u8,
    knob: u32,
    delta: i32,
    context: u32,
) -> u32 {
    mixer_volume_probe_entry(controller, knob, delta, context, 1)
}

#[cfg(mixer_volume_probe)]
#[no_mangle]
pub unsafe extern "C" fn mixer_volume_branch_probe(
    controller: *mut u8,
    knob: u32,
    delta: i32,
    context: u32,
) -> u32 {
    mixer_volume_probe_entry(controller, knob, delta, context, 2)
}

#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn fx_send_mixer_knob(
    controller: *mut u8,
    knob: u32,
    delta: i32,
    context: u32,
) -> u32 {
    const STOCK_FX_SEND_KNOB: usize = 0x0805_8c6b;
    const SEND_VALUES: usize = 0x14;

    let stock = function::<MixerKnobInput>(STOCK_FX_SEND_KNOB);
    if controller.is_null() || knob >= 8 {
        return stock(controller, knob, delta, context);
    }
    latch_send_controller(controller);
    let Some(leader) = paired_track_leader_index(knob) else {
        return stock(controller, knob, delta, context);
    };

    let result = stock(controller, leader, delta, context);
    let values = read_u32(controller, SEND_VALUES);
    if values != 0 {
        let read = function::<ObservableValue>(0x0803_fc59);
        let leader_value = read(values, leader);
        let follower_value = read(values, leader + 1);
        if !leader_value.is_null() && !follower_value.is_null() {
            let follower_delta = *leader_value as i32 - *follower_value as i32;
            if follower_delta != 0 {
                stock(controller, leader + 1, follower_delta, context);
            }
        }
    }
    result
}

/// Bitmask of leader indices 0..6 whose pair is currently enabled.
#[cfg(stereo_position_poc)]
unsafe fn current_pair_mask() -> u8 {
    let mut mask = 0u8;
    let mut leader = 0u32;
    while leader < 7 {
        if paired_pair_enabled(PLAYBACK_LEADER_TRACK + leader) {
            mask |= 1 << leader;
        }
        leader += 1;
    }
    mask
}

/// Records a send controller so its sends can be reconciled later.
///
/// The FX page render is a different class from the send knob handler: the
/// render object at `0x0804F7E3` works through `+0x0c`, `+0x10` and `+0x20`
/// and has neither the `+0x14` value observable nor the `+0x40` parameter base
/// the knob handler uses. Reading those offsets off the render object found
/// nothing, which is why the first attempt at this silently did nothing.
///
/// The knob handler's own argument is the only pointer proven to be a send
/// controller, so it is latched here rather than inferred from the render
/// object's sub-components. The page instantiates one controller per send
/// destination, so each is recorded separately with the pairing it last
/// reconciled; a newly seen controller adopts the current pairing without
/// rewriting anything.
#[cfg(stereo_position_poc)]
unsafe fn latch_send_controller(controller: *mut u8) {
    let state = pair_state();
    if state.is_null() {
        return;
    }
    let mut index = 0;
    while index < PAIR_SEND_SLOTS {
        let base = STATE_PAIR_SEND_SLOTS + index * PAIR_SEND_SLOT_STRIDE;
        let owner = read_u32(state, base);
        if owner == controller as u32 {
            return;
        }
        if owner == 0 {
            write_u32(state, base, controller as u32);
            write_u8(state, base + 4, current_pair_mask());
            return;
        }
        index += 1;
    }
}

/// Returns a follower's send to zero when its pair dissolves, and to the
/// leader's level when one forms, for every latched send controller.
///
/// Driven from the FX page render so that writes land while the page is up,
/// but through the latched controllers rather than the render's own object.
#[cfg(stereo_position_poc)]
unsafe fn service_latched_sends() {
    const STOCK_FX_SEND_KNOB: usize = 0x0805_8c6b;
    const SEND_VALUES: usize = 0x14;
    const SEND_MODE: usize = 0x24;

    let state = pair_state();
    if state.is_null() {
        return;
    }
    let read = function::<ObservableValue>(0x0803_fc59);
    let stock = function::<MixerKnobInput>(STOCK_FX_SEND_KNOB);
    let current = current_pair_mask();

    let mut index = 0;
    while index < PAIR_SEND_SLOTS {
        let slot = STATE_PAIR_SEND_SLOTS + index * PAIR_SEND_SLOT_STRIDE;
        index += 1;
        let controller = read_u32(state, slot) as *mut u8;
        if controller.is_null() {
            continue;
        }
        let previous = read_u8(state, slot + 4);
        if previous == current {
            continue;
        }
        // Stock only edits send levels while bit 0 of the mode observable is
        // clear; in the other mode the same knob edits an unrelated field.
        let mode = read_u32(controller, SEND_MODE);
        if mode == 0 {
            continue;
        }
        let mode_value = read(mode, 0);
        if mode_value.is_null() || *mode_value & 1 != 0 {
            continue;
        }
        let values = read_u32(controller, SEND_VALUES);
        if values == 0 {
            continue;
        }
        write_u8(state, slot + 4, current);

        let mut leader = 0u32;
        while leader < 7 {
            let bit = 1u8 << leader;
            let dissolved = previous & bit != 0 && current & bit == 0;
            let formed = previous & bit == 0 && current & bit != 0;
            if dissolved || formed {
                let follower = read(values, leader + 1);
                if !follower.is_null() {
                    let mut target = 0i32;
                    let mut resolved = true;
                    if formed {
                        let value = read(values, leader);
                        if value.is_null() {
                            resolved = false;
                        } else {
                            target = *value as i32;
                        }
                    }
                    if resolved {
                        let delta = target - *follower as i32;
                        if delta != 0 {
                            stock(controller, leader + 1, delta, 0);
                        }
                    }
                }
            }
            leader += 1;
        }
    }
}

#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn fx_send_pair_render(controller: *mut u8) -> u32 {
    const STOCK_FX_SEND_RENDER: usize = 0x0804_f7e3;
    const ENCODER_LED_BASE: usize = 0x44;

    service_latched_sends();
    let pre_colors = control_color_buffer(controller);    if !pre_colors.is_null() {
        paired_led_restore(pre_colors, ENCODER_LED_BASE, PAIR_LED_GROUP_FX_SEND);
    }
    let result = function::<FxRender>(STOCK_FX_SEND_RENDER)(controller);
    let colors = control_color_buffer(controller);
    if !colors.is_null() {
        paired_led_mirror(colors, ENCODER_LED_BASE, PAIR_LED_GROUP_FX_SEND);
    }
    result
}

#[cfg(stereo_position_poc)]
unsafe fn canonical_sidechain_source(source: u8) -> u8 {
    if source < 8 {
        if let Some(leader) = paired_track_leader_index(source as u32) {
            return leader as u8;
        }
    }
    source
}

#[cfg(stereo_position_poc)]
unsafe fn sync_sidechain_pair(controller: *mut u8, leader: u32) {
    const STOCK_PRESET_PRESS: usize = 0x0805_bf11;
    const STOCK_SOURCE_PRESS: usize = 0x0805_c015;
    const PRESET_CHILDREN: usize = 0x0c;
    const SOURCE_CONTROLLER: usize = 0x2c;
    const PRESET_VALUES: usize = 0x0c;
    const SOURCE_VALUES: usize = 0x0c;
    const ACTIVE_TARGET: usize = 0x14;

    let read = function::<ObservableValue>(0x0803_fc59);
    let leader_child =
        read_u32(controller, PRESET_CHILDREN + leader as usize * 4) as *mut u8;
    let follower_child =
        read_u32(controller, PRESET_CHILDREN + (leader as usize + 1) * 4) as *mut u8;
    if !leader_child.is_null() && !follower_child.is_null() {
        let leader_values = read_u32(leader_child, PRESET_VALUES);
        let follower_values = read_u32(follower_child, PRESET_VALUES);
        if leader_values != 0 && follower_values != 0 {
            let leader_value = read(leader_values, 0);
            let follower_value = read(follower_values, 0);
            if !leader_value.is_null()
                && !follower_value.is_null()
                && *leader_value != *follower_value
            {
                function::<SidechainPadInput>(STOCK_PRESET_PRESS)(
                    follower_child,
                    *leader_value as u32,
                    0,
                    0,
                );
            }
        }
    }

    let source_controller = read_u32(controller, SOURCE_CONTROLLER) as *mut u8;
    if source_controller.is_null() {
        return;
    }
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, 1 << 2);
    let source_values = read_u32(source_controller, SOURCE_VALUES);
    if source_values == 0 {
        return;
    }
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, 1 << 3);
    let leader_source = read(source_values, leader);
    if leader_source.is_null() {
        return;
    }
    let source = canonical_sidechain_source(*leader_source);
    let source_press = function::<SidechainPadInput>(STOCK_SOURCE_PRESS);
    write_u8(source_controller, ACTIVE_TARGET, leader as u8);
    source_press(source_controller, source as u32, 0, 0);
    write_u8(source_controller, ACTIVE_TARGET, leader as u8 + 1);
    source_press(source_controller, source as u32, 0, 0);
    write_u8(source_controller, ACTIVE_TARGET, leader as u8);
}

#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn sidechain_pair_press(
    controller: *mut u8,
    pad: u32,
    event: u32,
    context: u32,
) -> u32 {
    const STOCK_SIDECHAIN_PRESS: usize = 0x0805_574b;
    const PAGE_SOURCE: usize = 0x30;
    const SOURCE_CONTROLLER: usize = 0x2c;
    const ACTIVE_TARGET: usize = 0x14;

    let stock = function::<SidechainPadInput>(STOCK_SIDECHAIN_PRESS);
    if controller.is_null() || pad >= 32 {
        return stock(controller, pad, event, context);
    }
    let page_source = read_u32(controller, PAGE_SOURCE);
    if page_source == 0 {
        return stock(controller, pad, event, context);
    }
    let page_value = function::<ObservableValue>(0x0803_fc59)(page_source, 0);
    if page_value.is_null() || *page_value >= 4 {
        return stock(controller, pad, event, context);
    }
    // The page displays target tracks 2p and 2p+1, but both the source target
    // selector and the preset children are addressed by absolute track index,
    // so a pair straddling a page boundary can still be coupled by reaching its
    // off-page half directly.
    let first_track = *page_value as u32 * 2;

    if pad < 8 {
        let source = canonical_sidechain_source(pad as u8) as u32;
        let source_controller = read_u32(controller, SOURCE_CONTROLLER) as *mut u8;
        if source_controller.is_null() {
            return stock(controller, source, event, context);
        }
        let target = read_u8(source_controller, ACTIVE_TARGET) as u32;
        if target >= 8 {
            return stock(controller, source, event, context);
        }
        let Some(leader) = paired_track_leader_index(target) else {
            return stock(controller, source, event, context);
        };
        write_u8(source_controller, ACTIVE_TARGET, leader as u8);
        let result = stock(controller, source, event, context);
        write_u8(source_controller, ACTIVE_TARGET, leader as u8 + 1);
        stock(controller, source, event, context);
        // Restore the target the user is actually editing, which for a pair
        // spanning two pages is not necessarily the leader.
        write_u8(source_controller, ACTIVE_TARGET, target as u8);
        return result;
    }

    if pad >= 16 {
        let row = (pad - 16) >> 3;
        let column = (pad - 16) & 7;
        let track = first_track + row;
        let result = stock(controller, pad, event, context);
        if let Some(leader) = paired_track_leader_index(track) {
            let partner = if track == leader { leader + 1 } else { leader };
            if partner >= first_track && partner < first_track + 2 {
                stock(controller, 16 + (partner - first_track) * 8 + column, event, context);
            } else {
                sidechain_preset_press(controller, partner, column);
            }
        }
        return result;
    }
    stock(controller, pad, event, context)
}

/// Applies a preset selection to a target track's own preset child, which works
/// for a track that is not currently displayed on the sidechain page.
#[cfg(stereo_position_poc)]
unsafe fn sidechain_preset_press(controller: *mut u8, track: u32, column: u32) {
    const STOCK_PRESET_PRESS: usize = 0x0805_bf11;
    const PRESET_CHILDREN: usize = 0x0c;

    if track >= 8 {
        return;
    }
    let child = read_u32(controller, PRESET_CHILDREN + track as usize * 4) as *mut u8;
    if child.is_null() {
        return;
    }
    function::<SidechainPadInput>(STOCK_PRESET_PRESS)(child, column, 0, 0);
}

/// Records the sidechain page controller so dissolutions can be reconciled
/// from the main loop. A newly seen controller adopts the current pairing.
#[cfg(stereo_position_poc)]
unsafe fn latch_sidechain_controller(controller: *mut u8) {
    let state = pair_state();
    if state.is_null() || controller.is_null() {
        return;
    }
    if read_u32(state, STATE_PAIR_SIDECHAIN_CONTROLLER) != controller as u32 {
        write_u32(state, STATE_PAIR_SIDECHAIN_CONTROLLER, controller as u32);
        write_u8(state, STATE_PAIR_SIDECHAIN_MASK, current_pair_mask());
    }
}

/// Finds the sidechain page without it ever having been displayed.
///
/// The app init (`FUN_08027980`) registers the UI root as the context of the
/// callback at `0x20006EA0` (`FUN_0803FEDC`). The root's `+0x3c` is the page
/// group built by `FUN_0804C3C4`, whose `+0x10` is the sidechain page from
/// `FUN_080555F0`. The result is accepted only if it carries that page's
/// vtable, so a wrong path yields nothing rather than a stray object.
#[cfg(stereo_position_poc)]
unsafe fn resolve_sidechain_page() -> *mut u8 {
    const UI_REGISTRATION: *const u32 = 0x2000_6ea0 as *const u32;
    const PAGE_GROUP: usize = 0x3c;
    const SIDECHAIN_PAGE: usize = 0x10;
    const SIDECHAIN_PAGE_VTABLE: u32 = 0x0806_7f14;

    let root = read_volatile(UI_REGISTRATION.add(1)) as *const u8;
    if root.is_null() {
        return core::ptr::null_mut();
    }
    let group = read_u32(root, PAGE_GROUP) as *const u8;
    if group.is_null() {
        return core::ptr::null_mut();
    }
    let page = read_u32(group, SIDECHAIN_PAGE) as *mut u8;
    if page.is_null() || read_u32(page, 0) != SIDECHAIN_PAGE_VTABLE {
        return core::ptr::null_mut();
    }
    page
}

/// Pairing gesture: hold one track button and press an adjacent one.
///
/// Wraps the UI root's button-press method (`0x0803D8B8`, root vtable slot
/// `0x080672EC`). Button IDs 0..7 are the track buttons (its `tbh` sends them
/// to `FUN_0804D662`, which selects the track), and the root records held
/// buttons in the bitmap at `+0xa0`, setting the bit on press. The gesture
/// applies only when stock would honour a track button: stock ignores them
/// while the `+0x0c` observable is non-zero (mask `0x07DFFF00`).
///
/// Pairing is derived from the sample assignment, so the gesture performs the
/// assignment: if the held track's sample (else the pressed track's) is half of
/// an identified stereo pair, the left half goes to the lower track and the
/// right half to the upper one. The gesture on an already formed pair unpairs
/// it (`unpair_track_pair`). A gesture that pairs nothing is swallowed without
/// effect. Held buttons are judged physically (`physical_track_buttons_held`),
/// since a follower press is redirected to its leader. The swallowed press
/// never sets its held bit, so stock's release (`0x0803DCC8`) ignores it too.
#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn track_pair_button_press(
    root: *mut u8,
    button: u32,
    event: u32,
    context: u32,
) -> u32 {
    const STOCK_ROOT_PRESS: usize = 0x0803_d8b9;
    const HELD_BUTTONS: usize = 0xa0;
    const TRACK_BUTTONS_BLOCKED: usize = 0x0c;

    let stock = function::<SidechainPadInput>(STOCK_ROOT_PRESS);
    if root.is_null() || button >= 8 {
        return stock(root, button, event, context);
    }
    let held = physical_track_buttons_held(root);
    if held & (1 << button) != 0 {
        return stock(root, button, event, context);
    }
    let partner = if button > 0 && held & (1 << (button - 1)) != 0 {
        button - 1
    } else if button < 7 && held & (1 << (button + 1)) != 0 {
        button + 1
    } else {
        // Shared views: a pair acts as one track, so the follower's button
        // presses the leader's. The redirect is remembered so the follower's
        // release clears the leader's held bit (see `track_pair_button_release`).
        if let Some(leader) = paired_track_leader_index(button) {
            if leader != button {
                write_volatile(PAIR_BUTTON_REDIRECT.add(button as usize), leader as u8 + 1);
                return stock(root, leader, event, context);
            }
        }
        return stock(root, button, event, context);
    };
    let blocked = read_u32(root, TRACK_BUTTONS_BLOCKED);
    if blocked != 0 {
        let value = function::<ObservableValue>(0x0803_fc59)(blocked, 0);
        if !value.is_null() && *value != 0 {
            return stock(root, button, event, context);
        }
    }
    let lower = partner.min(button);
    if paired_track_leader_index(lower) == Some(lower) {
        unpair_track_pair(lower);
    } else {
        assign_track_pair(partner, button);
    }
    context
}

/// Per physical track button: 1 + the leader button its press was redirected
/// to, or 0. Lives after `PAIR_TRACK_BUTTON_LIT`; cleared at boot.
#[cfg(stereo_position_poc)]
const PAIR_BUTTON_REDIRECT: *mut u8 = 0x2004_fc34 as *mut u8;

/// The root's held track buttons (`+0xa0`) as physically held: a redirected
/// follower press holds the leader's bit, so it is moved back to the follower.
#[cfg(stereo_position_poc)]
unsafe fn physical_track_buttons_held(root: *const u8) -> u32 {
    let stock_held = read_u8(root, 0xa0) as u32;
    let mut held = stock_held;
    for button in 0..8usize {
        let target = read_volatile(PAIR_BUTTON_REDIRECT.add(button)) as u32;
        if (1..=8).contains(&target) {
            held &= !(1 << (target - 1));
        }
    }
    for button in 0..8usize {
        let target = read_volatile(PAIR_BUTTON_REDIRECT.add(button)) as u32;
        if (1..=8).contains(&target) && stock_held & (1 << (target - 1)) != 0 {
            held |= 1 << button;
        }
    }
    held
}

/// Wraps the root's button-release method (`0x0803DDB0`, root vtable slot
/// `0x080672F0`, which tails into `FUN_0803DCC8`). A follower release whose
/// press was redirected releases the leader button instead.
#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn track_pair_button_release(
    root: *mut u8,
    button: u32,
    event: u32,
    context: u32,
) -> u32 {
    const STOCK_ROOT_RELEASE: usize = 0x0803_ddb1;
    let mut button = button;
    if button < 8 {
        let slot = PAIR_BUTTON_REDIRECT.add(button as usize);
        let target = read_volatile(slot) as u32;
        if target != 0 {
            write_volatile(slot, 0);
            if target <= 8 {
                button = target - 1;
            }
        }
    }
    function::<SidechainPadInput>(STOCK_ROOT_RELEASE)(root, button, event, context)
}

/// Keeps a pair's follower from being the selected track: if it is (the pair
/// formed while it was selected, or a project loaded that way), taps the
/// leader's button through stock, exactly as the user would. Waits until no
/// track button is held and stock honours track buttons.
#[cfg(stereo_position_poc)]
unsafe fn service_pair_selection() {
    const UI_REGISTRATION: *const u32 = 0x2000_6ea0 as *const u32;
    const PAGE_GROUP: usize = 0x3c;
    const SELECTED_TRACK: usize = 0x25c;
    const TRACK_BUTTONS_BLOCKED: usize = 0x0c;
    const OBSERVABLE_READ: usize = 0x0803_fc59;

    let root = read_volatile(UI_REGISTRATION.add(1)) as *mut u8;
    if root.is_null() || read_u8(root, 0xa0) != 0 {
        return;
    }
    let group = read_u32(root, PAGE_GROUP) as *const u8;
    if group.is_null() {
        return;
    }
    let observable = read_u32(group, SELECTED_TRACK);
    if observable == 0 {
        return;
    }
    let value = function::<ObservableValue>(OBSERVABLE_READ)(observable, 0);
    if value.is_null() {
        return;
    }
    let selected = read_volatile(value) as u32;
    let Some(leader) = paired_track_leader_index(selected) else {
        return;
    };
    if leader == selected {
        return;
    }
    let blocked = read_u32(root, TRACK_BUTTONS_BLOCKED);
    if blocked != 0 {
        let value = function::<ObservableValue>(OBSERVABLE_READ)(blocked, 0);
        if value.is_null() || *value != 0 {
            return;
        }
    }
    function::<SidechainPadInput>(0x0803_d8b9)(root, leader, 0, 0);
    function::<SidechainPadInput>(0x0803_ddb1)(root, leader, 0, 0);
}

/// Per track: the sample it held before a pairing gesture made it a follower
/// (0xFF unknown). RAM only, so a reboot falls back to the default sample.
#[cfg(stereo_position_poc)]
const PAIR_PRIOR_SAMPLE: *mut u8 = 0x2004_fc3c as *mut u8;

/// Reads a track's sample from its session observable (0xFF if unavailable).
#[cfg(stereo_position_poc)]
unsafe fn session_track_sample(session: *const u8, track: u32) -> u32 {
    let obs = read_u32(session, 0x9c + track as usize * 4);
    if obs == 0 {
        return 0xff;
    }
    let value = function::<ObservableValue>(0x0803_fc59)(obs, 0);
    if value.is_null() {
        0xff
    } else {
        read_volatile(value) as u32
    }
}

/// Assigns `sample` to `track` in both the session observable and the
/// track's parameter block (see `assign_track_pair`).
#[cfg(stereo_position_poc)]
unsafe fn write_track_sample(session: *const u8, track: u32, sample: u8) {
    const TRACK_PARAMETER_GET: usize = 0x0804_6871;
    const OBSERVABLE_VALUE: usize = 0x0803_fd27;
    const OBSERVABLE_PUBLISH: usize = 0x0803_fbed;
    const STOCK_OBSERVABLE_SET: usize = 0x0804_65dd;
    const SAMPLE_PARAMETER: u32 = 0;

    let obs = read_u32(session, 0x9c + track as usize * 4);
    if obs != 0 && session_track_sample(session, track) != sample as u32 {
        let value = function::<ObservableValue>(OBSERVABLE_VALUE)(obs, 0);
        if !value.is_null() {
            write_volatile(value, sample);
            function::<FirmwareMethod4>(OBSERVABLE_PUBLISH)(obs, 0, 0, 0);
        }
    }
    if function::<TrackParameterGet>(TRACK_PARAMETER_GET)(PLAYBACK_LEADER_TRACK + track, SAMPLE_PARAMETER)
        != sample as u32
    {
        let object = track_parameter_object(track);
        if !object.is_null() {
            function::<ObservableSet>(STOCK_OBSERVABLE_SET)(object, SAMPLE_PARAMETER, sample as u32);
        }
    }
}

/// Unpairs `leader` and the track above it (the gesture repeated on a formed
/// pair). The follower gets back the sample it had before the gesture paired
/// it, else its blank default `2 * track`, and never a half of this
/// recording: candidates that are any identified stereo half are skipped.
#[cfg(stereo_position_poc)]
unsafe fn unpair_track_pair(leader: u32) {
    let follower = leader + 1;
    if follower >= 8 {
        return;
    }
    let state = pair_state();
    if state.is_null() {
        return;
    }
    let pack = registry_pack(state);
    if pack >= 0x20 {
        return;
    }
    let session = find_session_object();
    if session.is_null() {
        return;
    }
    let slot = PAIR_PRIOR_SAMPLE.add(follower as usize);
    let prior = read_volatile(slot) as u32;
    write_volatile(slot, 0xff);
    let is_half = |sample: u32| identified_pair_left(state, pack, sample as u8).is_some();
    let mut sample = prior;
    if sample >= 128 || is_half(sample) {
        sample = 0xff;
        let mut step = 0;
        while step < 128 {
            let candidate = (2 * follower + step) % 128;
            if !is_half(candidate) {
                sample = candidate;
                break;
            }
            step += 1;
        }
    }
    if sample < 128 {
        write_track_sample(session, follower, sample as u8);
    }
}

/// Assigns the stereo pair owning the held track's sample, else the pressed
/// track's, across the two adjacent tracks, left half on the lower one.
///
/// The track's sample lives in the session object (ctor `FUN_08036FA8`), one
/// observable per track at `+0x9c + track*4`, persisted to project `+0x28CEC`
/// (blank defaults 0,2,..,14) and copied on load into each track's 0x29-byte
/// parameter block, which track parameter 0 reads and playback and pairing
/// follow. Neither copy refreshes the other at runtime: writing only the block
/// was lost, writing only the observable updated the Sample view but not
/// playback. So both are written.
#[cfg(stereo_position_poc)]
unsafe fn assign_track_pair(held: u32, pressed: u32) {
    let state = pair_state();
    if state.is_null() {
        return;
    }
    let pack = registry_pack(state);
    if pack >= 0x20 {
        return;
    }
    let session = find_session_object();
    if session.is_null() {
        return;
    }
    let mut left = None;
    for track in [held, pressed] {
        let sample = session_track_sample(session, track);
        if sample < 128 {
            left = identified_pair_left(state, pack, sample as u8);
            if left.is_some() {
                break;
            }
        }
    }
    let Some(left) = left else {
        return;
    };
    let lower = held.min(pressed);
    let upper = held.max(pressed);
    let prior = session_track_sample(session, upper);
    if prior != (left + 1) as u32 {
        write_volatile(PAIR_PRIOR_SAMPLE.add(upper as usize), prior as u8);
    }
    write_track_sample(session, lower, left);
    write_track_sample(session, upper, left + 1);
}

/// Finds the session object by its five vtable pointers (`FUN_08036FA8`
/// installs `0x08066F5C + {8, 0x24, 0x38, 0x44, 0x50}`) in the F745's single
/// contiguous 320 KiB SRAM. It is allocated once at boot and never freed.
#[cfg(stereo_position_poc)]
unsafe fn find_session_object() -> *const u8 {
    const SRAM_START: usize = 0x2000_0000;
    const SRAM_END: usize = 0x2005_0000;
    const SIGNATURE: [u32; 5] = [0x0806_6f64, 0x0806_6f80, 0x0806_6f94, 0x0806_6fa0, 0x0806_6fac];

    let mut address = SRAM_START;
    while address + 0x100 <= SRAM_END {
        let words = address as *const u32;
        if read_volatile(words) == SIGNATURE[0] {
            let mut index = 1;
            while index < SIGNATURE.len() && read_volatile(words.add(index)) == SIGNATURE[index] {
                index += 1;
            }
            if index == SIGNATURE.len() {
                return address as *const u8;
            }
        }
        address += 4;
    }
    core::ptr::null()
}

/// Keeps every pair's sidechain settings coupled from the main loop, so it
/// does not depend on the sidechain page ever being opened.
///
/// A newly formed pair has its follower converged onto the leader. A dissolved
/// pair's follower returns to the blank-project sidechain defaults: preset 0
/// (`FUN_08032FBE` zeroes the per-track preset array) and source track 1 (track
/// parameter 20, template byte 32 = 0). While paired the follower was slaved to
/// its leader, so otherwise it would keep the leader's settings.
#[cfg(stereo_position_poc)]
unsafe fn service_sidechain_residue() {
    const STOCK_PRESET_PRESS: usize = 0x0805_bf11;
    const STOCK_SOURCE_PRESS: usize = 0x0805_c015;
    const PRESET_CHILDREN: usize = 0x0c;
    const PRESET_VALUES: usize = 0x0c;
    const SOURCE_CONTROLLER: usize = 0x2c;
    const SOURCE_VALUES: usize = 0x0c;
    const ACTIVE_TARGET: usize = 0x14;
    const DEFAULT_PRESET: u8 = 0;
    const DEFAULT_SOURCE: u8 = 0;

    let state = pair_state();
    if state.is_null() {
        return;
    }
    let mut controller = read_u32(state, STATE_PAIR_SIDECHAIN_CONTROLLER) as *mut u8;
    if controller.is_null() {
        controller = resolve_sidechain_page();
        if controller.is_null() {
            return;
        }
        // Every pair that already exists is treated as newly formed, which
        // converges it; converging an already coupled pair changes nothing.
        write_u32(state, STATE_PAIR_SIDECHAIN_CONTROLLER, controller as u32);
        write_u8(state, STATE_PAIR_SIDECHAIN_MASK, 0);
    }
    let previous = read_u8(state, STATE_PAIR_SIDECHAIN_MASK);
    let current = current_pair_mask();
    if previous == current {
        return;
    }
    write_u8(state, STATE_PAIR_SIDECHAIN_MASK, current);

    let read = function::<ObservableValue>(0x0803_fc59);
    let press = function::<SidechainPadInput>(STOCK_PRESET_PRESS);
    let source_press = function::<SidechainPadInput>(STOCK_SOURCE_PRESS);
    let source_controller = read_u32(controller, SOURCE_CONTROLLER) as *mut u8;
    // Dissolutions first, so a track moving straight from follower to leader is
    // reset before its new follower copies it.
    let mut leader = 0u32;
    while leader < 7 {
        let bit = 1u8 << leader;
        leader += 1;
        if previous & bit == 0 || current & bit != 0 {
            continue;
        }
        let follower = leader;
        let child = read_u32(controller, PRESET_CHILDREN + follower as usize * 4) as *mut u8;
        if !child.is_null() {
            let values = read_u32(child, PRESET_VALUES);
            if values != 0 {
                let value = read(values, 0);
                if !value.is_null() && *value != DEFAULT_PRESET {
                    press(child, DEFAULT_PRESET as u32, 0, 0);
                }
            }
        }
        if !source_controller.is_null() {
            let values = read_u32(source_controller, SOURCE_VALUES);
            if values != 0 {
                let value = read(values, follower);
                if !value.is_null() && *value != DEFAULT_SOURCE {
                    let target = read_u8(source_controller, ACTIVE_TARGET);
                    write_u8(source_controller, ACTIVE_TARGET, follower as u8);
                    source_press(source_controller, DEFAULT_SOURCE as u32, 0, 0);
                    write_u8(source_controller, ACTIVE_TARGET, target);
                }
            }
        }
    }
    let mut leader = 0u32;
    while leader < 7 {
        let bit = 1u8 << leader;
        if current & bit != 0 && previous & bit == 0 {
            sync_sidechain_pair(controller, leader);
        }
        leader += 1;
    }
}

/// The firmware's track colour, from the global colour table at `0x08062AB0`.
///
/// Every LED colour is `g_colours[enum]`, written through the setter at
/// `0x080555C0`. For tracks the convention is `enum = track + 1` full and
/// `track + 9` dim; there is no brightness function, the dim form is simply a
/// second static entry eight places along.
#[cfg(stereo_position_poc)]
unsafe fn track_color(track: u32, dim: bool) -> u16 {
    const TRACK_COLOR_TABLE: usize = 0x0806_2ab0;
    let index = track + if dim { 9 } else { 1 };
    read_u16(TRACK_COLOR_TABLE as *const u8, index as usize * 2)
}

/// Repaints `from`'s track colour as `to`'s across one eight-column row.
///
/// Only words that are exactly `from`'s full or dim colour are rewritten, so
/// any cell the page coloured for some other reason is left alone. That makes
/// the pass safe to run unconditionally and idempotent: once a row has been
/// repainted nothing in it still matches `from`.
#[cfg(stereo_position_poc)]
unsafe fn recolor_track_row(colors: *mut u8, base: usize, from: u32, to: u32) {
    // Every track's dim colour is exactly its full colour plus one across all
    // eight entries, so a single offset converts either form.
    let from_full = track_color(from, false);
    let shift = track_color(to, false).wrapping_sub(from_full);
    let from_dim = from_full.wrapping_add(1);
    let mut column = 0;
    while column < 8 {
        let offset = base + column * 2;
        let word = read_u16(colors, offset);
        if word == from_full || word == from_dim {
            write_u16(colors, offset, word.wrapping_add(shift));
        }
        column += 1;
    }
}

/// RAM copy of the colour-enum table, which every stock colour lookup reads.
///
/// The builder redirects all 47 stock literals naming `0x08062AB0` (and the six
/// naming entries inside it) here, so rewriting a follower's entries recolours
/// it in every view at once. The address is the unused gap between the bump
/// arena's limit (`[0x20002B54]` = `0x2004F760`) and the main stack, which the
/// stock memory report bounds at `0x2004FE00`; 0x470 bytes end at `0x2004FBD0`.
#[cfg(stereo_position_poc)]
const PAIR_COLOR_TABLE_RAM: usize = 0x2004_f760;
#[cfg(stereo_position_poc)]
const PAIR_COLOR_TABLE_FLASH: usize = 0x0806_2ab0;
#[cfg(stereo_position_poc)]
const PAIR_COLOR_TABLE_ENTRIES: usize = 568;

/// Per-track colour runs in the enum table as `(first enum, stride, length)`.
/// Track `t` owns enums `first + t * stride .. + length`.
#[cfg(stereo_position_poc)]
const PAIR_COLOR_RUNS: [(usize, usize, usize); 8] = [
    (1, 1, 1),    // full
    (9, 1, 1),    // dim
    (71, 4, 4),   // four-shade block
    (127, 12, 12), // per-track record (dim, full and two further shades)
    (223, 1, 1),  // full
    (279, 1, 1),  // full (palette 0xE1 + t)
    (304, 3, 3),  // full, dim, off
    (360, 1, 1),  // full
];

/// The live RGB palette (`.data`), indexed by palette index; the LED output
/// stage `FUN_0803E8E6` reads it every frame.
#[cfg(stereo_position_poc)]
const PAIR_PALETTE_RAM: usize = 0x2000_3974;

/// Per-track palette slots whose RGB itself is the track's colour, as
/// `(first slot)` with track `t` at `first + t`. Encoder LEDs on these slots are
/// brightness-modulated from a per-slot parameter (`[0x20003794]`), so the
/// follower keeps its own slot and only its RGB is replaced.
/// 0x129/0x131: pastel shades (mixer encoders, sample pads); 0x149: full.
#[cfg(stereo_position_poc)]
const PAIR_PALETTE_FAMILIES: [usize; 3] = [0x129, 0x131, 0x149];

/// Boot-time copies of the family RGBs, after the colour table (ends
/// `0x2004FBD0`); 24 words end at `0x2004FC30`.
#[cfg(stereo_position_poc)]
const PAIR_PALETTE_SAVED: usize = 0x2004_fbd0;

/// Per-track encoder slots `0xE9 + 8t + k`. `FUN_080585DA` writes them from
/// enum `360 + t` (or a red/orange override), so they follow the remapped
/// table only when stock happens to rewrite them.
#[cfg(stereo_position_poc)]
const PAIR_ENCODER_SLOTS: usize = 0xe9;

/// Copies the stock colour table into RAM. Must run before any colour lookup,
/// so it is called ahead of `main` and again at Main-task entry (the
/// pre-scheduler stack may reach the region; the task stacks never do).
#[cfg(stereo_position_poc)]
unsafe fn pair_color_table_init() {
    let source = PAIR_COLOR_TABLE_FLASH as *const u32;
    let target = PAIR_COLOR_TABLE_RAM as *mut u32;
    let mut word = 0;
    while word < PAIR_COLOR_TABLE_ENTRIES / 2 {
        write_volatile(target.add(word), read_volatile(source.add(word)));
        word += 1;
    }
    let palette = PAIR_PALETTE_RAM as *const u32;
    let saved = PAIR_PALETTE_SAVED as *mut u32;
    let mut family = 0;
    while family < PAIR_PALETTE_FAMILIES.len() {
        let mut track = 0;
        while track < 8 {
            write_volatile(
                saved.add(family * 8 + track),
                read_volatile(palette.add(PAIR_PALETTE_FAMILIES[family] + track)),
            );
            track += 1;
        }
        family += 1;
    }
}

/// Points every follower's colour runs at its leader's, and every other
/// track's at its own. Each entry is written straight to its final value, so
/// a lookup can never observe an intermediate colour.
#[cfg(stereo_position_poc)]
unsafe fn service_pair_colors() {
    let source = PAIR_COLOR_TABLE_FLASH as *const u16;
    let target = PAIR_COLOR_TABLE_RAM as *mut u16;
    let mut track = 1usize;
    while track < 8 {
        let owner = match paired_track_leader_index(track as u32) {
            Some(leader) => leader as usize,
            None => track,
        };
        let mut run = 0;
        while run < PAIR_COLOR_RUNS.len() {
            let (first, stride, length) = PAIR_COLOR_RUNS[run];
            let mut entry = 0;
            while entry < length {
                let value = read_volatile(source.add(first + owner * stride + entry));
                let slot = target.add(first + track * stride + entry);
                if read_volatile(slot) != value {
                    write_volatile(slot, value);
                }
                entry += 1;
            }
            run += 1;
        }

        let palette = PAIR_PALETTE_RAM as *mut u32;
        let saved = PAIR_PALETTE_SAVED as *const u32;
        let mut family = 0;
        while family < PAIR_PALETTE_FAMILIES.len() {
            let value = read_volatile(saved.add(family * 8 + owner));
            let slot = palette.add(PAIR_PALETTE_FAMILIES[family] + track);
            if read_volatile(slot) != value {
                write_volatile(slot, value);
            }
            family += 1;
        }

        // Encoder slots: replace only a track colour, leaving stock's
        // override colours alone; stock restores from the remapped enum.
        let full = saved.add((PAIR_PALETTE_FAMILIES.len() - 1) * 8);
        let wanted = read_volatile(full.add(owner));
        let mut knob = 0;
        while knob < 8 {
            let slot = palette.add(PAIR_ENCODER_SLOTS + track * 8 + knob);
            let current = read_volatile(slot);
            if current != wanted {
                let mut other = 0;
                while other < 8 {
                    if read_volatile(full.add(other)) == current {
                        write_volatile(slot, wanted);
                        break;
                    }
                    other += 1;
                }
            }
            knob += 1;
        }
        track += 1;
    }
}

/// Replaces `bl main` in the C runtime start-up.
#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn pair_color_boot_main() -> u32 {
    write_volatile(PAIR_TRACK_BUTTON_LIT as *mut u8, 0xff);
    for button in 0..8 {
        write_volatile(PAIR_BUTTON_REDIRECT.add(button), 0);
        write_volatile(PAIR_PRIOR_SAMPLE.add(button), 0xff);
    }
    write_volatile(OCCUPANCY_MODEL_CACHE, 0);
    pair_color_table_init();
    function::<unsafe extern "C" fn() -> u32>(0x0802_5689)()
}

/// Replaces the Main task's entry point (registered by `FUN_08026090`).
#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn pair_color_main_task(argument: *mut u8) {
    pair_color_table_init();
    function::<unsafe extern "C" fn(*mut u8)>(0x0802_5771)(argument)
}

/// Track-button LED layer: the UI root's `+0x38` (`FUN_0804BF28`, vtable
/// `0x080671F8`, 13 cells). The root's LED composite `FUN_0803E80C` maps
/// LEDs `0x20..0x27` to its cells 0..7. Its render `FUN_0804BFEC` paints them
/// enum 19 and lights only the selected track (byte `+0x20`) with `1 + sel`.
#[cfg(stereo_position_poc)]
const TRACK_BUTTONS_VTABLE: u32 = 0x0806_71f8;
/// Partner cell lit by the overlay (0xFF none); after the saved palette copies.
#[cfg(stereo_position_poc)]
const PAIR_TRACK_BUTTON_LIT: usize = 0x2004_fc30;

#[cfg(stereo_position_poc)]
unsafe fn track_buttons_layer() -> *mut u8 {
    const UI_REGISTRATION: *const u32 = 0x2000_6ea0 as *const u32;
    const TRACK_BUTTONS: usize = 0x38;
    let root = read_volatile(UI_REGISTRATION.add(1)) as *const u8;
    if root.is_null() {
        return core::ptr::null_mut();
    }
    let layer = read_u32(root, TRACK_BUTTONS) as *mut u8;
    if layer.is_null() || read_u32(layer, 0) != TRACK_BUTTONS_VTABLE {
        return core::ptr::null_mut();
    }
    layer
}

/// Lights both buttons of a pair whenever stock lights either one. Through the
/// shared colour table the follower's enum already gives the leader's colour.
#[cfg(stereo_position_poc)]
unsafe fn pair_track_buttons_overlay(layer: *mut u8) {
    const SELECTED_TRACK: usize = 0x20;
    const UNLIT: usize = 19;

    if layer.is_null() {
        return;
    }
    let buffer = read_u32(layer, 4) as *const u8;
    if buffer.is_null() {
        return;
    }
    let colors = read_u32(buffer, 0) as *mut u16;
    if colors.is_null() {
        return;
    }
    let table = PAIR_COLOR_TABLE_RAM as *const u16;
    let lit_of = |track: u32| read_volatile(table.add(1 + track as usize));
    let selected = read_u8(layer, SELECTED_TRACK) as u32;
    let mut partner = 0xffu32;
    if selected < 8 && read_volatile(colors.add(selected as usize)) == lit_of(selected) {
        if let Some(leader) = paired_track_leader_index(selected) {
            partner = if leader == selected { leader + 1 } else { leader };
        }
    }
    let previous = read_volatile(PAIR_TRACK_BUTTON_LIT as *const u8) as u32;
    // Stock lights only the selected cell, so a former partner is always
    // unlit. Not compared with `lit_of`: unpairing restores the follower's own
    // colour while its cell still shows the leader's.
    if previous < 8 && previous != partner && previous != selected {
        let cell = colors.add(previous as usize);
        let unlit = read_volatile(table.add(UNLIT));
        if read_volatile(cell) != unlit {
            write_volatile(cell, unlit);
        }
    }
    if partner < 8 {
        let cell = colors.add(partner as usize);
        if read_volatile(cell) != lit_of(partner) {
            write_volatile(cell, lit_of(partner));
        }
    }
    write_volatile(PAIR_TRACK_BUTTON_LIT as *mut u8, partner as u8);
}

/// Sample bank: the bottom two rows of a track's Sample page. Each track owns
/// one bank object (`FUN_0805C060`, vtable `0x08068294`, 32 cells) at page
/// group `+0x14 + 4t`; the group's compose (`FUN_0804DD1C`) shows 16 of them.
/// Cell `i` is sample `32 * page + i` (page observable `+0x1c`), the selected
/// sample observable is `+0x28`, and stock paints colour enum `base + 2` (dim)
/// or `base + 1` (selected), `base` at `+0x24` (`0x12F + 3t`).
#[cfg(stereo_position_poc)]
const SAMPLE_BANK_VTABLE: u32 = 0x0806_8294;
/// Page group vtable (`FUN_0804C3C4`), primary at base + 8.
#[cfg(stereo_position_poc)]
const PAGE_GROUP_VTABLE: u32 = 0x0806_78dc;
/// The sample-occupancy model (ctor `0x08039EBC`), found once from the main
/// loop: 0 not searched yet, 1 not found, else its address.
#[cfg(stereo_position_poc)]
const OCCUPANCY_MODEL_CACHE: *mut u32 = 0x2004_fc44 as *mut u32;

#[cfg(stereo_position_poc)]
unsafe fn service_occupancy_model_cache() {
    const SRAM_START: usize = 0x2000_0000;
    const SRAM_END: usize = 0x2005_0000;
    const SIGNATURE: [u32; 4] = [0x0806_7570, 0x0806_7588, 0x0806_7594, 0x0806_75a8];

    if read_volatile(OCCUPANCY_MODEL_CACHE) != 0 {
        return;
    }
    let mut found = 1u32;
    let mut address = SRAM_START;
    while address + 0x40 <= SRAM_END {
        let words = address as *const u32;
        if read_volatile(words) == SIGNATURE[0] {
            let mut index = 1;
            while index < SIGNATURE.len() && read_volatile(words.add(index)) == SIGNATURE[index] {
                index += 1;
            }
            if index == SIGNATURE.len() {
                found = address as u32;
                break;
            }
        }
        address += 4;
    }
    write_volatile(OCCUPANCY_MODEL_CACHE, found);
}

/// The 128-bit sample occupancy observable (model `+0x14`), or 0.
#[cfg(stereo_position_poc)]
unsafe fn sample_occupancy_observable() -> u32 {
    let model = read_volatile(OCCUPANCY_MODEL_CACHE);
    if model <= 1 || read_volatile(model as *const u32) != 0x0806_7570 {
        return 0;
    }
    read_u32(model as *const u8, 0x14)
}

/// Recolours the selected track's sample bank: empty slots off, mono samples
/// dim white, stereo halves dim teal (as on the Sample Rec page), and the
/// selected sample bright in the same colour; on a paired track both halves
/// are bright. A selected empty slot is dim red (Sample Rec's empty colour).
/// Only cells showing stock's dim/selected colour or one of these are
/// touched, so stock's transient colours (e.g. red while recording) remain.
#[cfg(stereo_position_poc)]
unsafe fn sample_bank_overlay(group: *const u8) {
    const OBSERVABLE_READ: usize = 0x0803_fc59;
    const SELECTED_TRACK: usize = 0x25c;
    const BANKS: usize = 0x14;
    const PAGE: usize = 0x1c;
    const SELECTED_SAMPLE: usize = 0x28;
    const COLOR_BASE: usize = 0x24;
    const OFF: u16 = 0x00;
    const WHITE: u16 = 0x01;
    const DIM_WHITE: u16 = 0x09;
    const TEAL: u16 = 0x25;
    const DIM_TEAL: u16 = 0x26;
    const DIM_RED: u16 = 0x56;

    let read = function::<ObservableValue>(OBSERVABLE_READ);
    let observed = |observable: u32, index: u32| -> u32 {
        if observable == 0 {
            return 0xff;
        }
        let value = read(observable, index);
        if value.is_null() {
            0xff
        } else {
            read_volatile(value) as u32
        }
    };
    let track = observed(read_u32(group, SELECTED_TRACK), 0);
    if track >= 8 {
        return;
    }
    let bank = read_u32(group, BANKS + track as usize * 4) as *const u8;
    if bank.is_null() || read_u32(bank, 0) != SAMPLE_BANK_VTABLE {
        return;
    }
    let buffer = read_u32(bank, 4) as *const u8;
    if buffer.is_null() {
        return;
    }
    let cells = read_u32(buffer, 0) as *mut u16;
    if cells.is_null() {
        return;
    }
    let occupancy = sample_occupancy_observable();
    if occupancy == 0 {
        return;
    }
    let page = observed(read_u32(bank, PAGE), 0);
    if page >= 4 {
        return;
    }
    let selected = observed(read_u32(bank, SELECTED_SAMPLE), 0);
    let state = pair_state();
    let pack = if state.is_null() { 0xff } else { registry_pack(state) };
    let pair_left = |sample: u32| -> Option<u8> {
        if pack >= 0x20 || sample >= 128 {
            None
        } else {
            identified_pair_left(state, pack, sample as u8)
        }
    };
    let (bright_low, bright_high) = match pair_left(selected) {
        Some(left) if paired_track_leader_index(track).is_some() => (left as u32, left as u32 + 1),
        _ => (selected, selected),
    };
    let table = PAIR_COLOR_TABLE_RAM as *const u16;
    let base = read_volatile(bank.add(COLOR_BASE) as *const u16) as usize;
    let stock_selected = read_volatile(table.add(base + 1));
    let stock_dim = read_volatile(table.add(base + 2));
    let mut occupied_word = 0u32;
    let mut cell = 0u32;
    while cell < 32 {
        let sample = page * 32 + cell;
        if cell == 0 {
            let word = read(occupancy, sample >> 5).cast::<u32>();
            if word.is_null() {
                return;
            }
            occupied_word = read_volatile(word);
        }
        let slot = cells.add(cell as usize);
        let current = read_volatile(slot);
        cell += 1;
        if !matches!(current, OFF | WHITE | DIM_WHITE | TEAL | DIM_TEAL | DIM_RED)
            && current != stock_selected
            && current != stock_dim
        {
            continue;
        }
        let occupied = occupied_word >> (sample & 31) & 1 != 0;
        let stereo = occupied && pair_left(sample).is_some();
        let lit = sample == bright_low || sample == bright_high;
        let desired = match (lit, occupied, stereo) {
            (true, _, true) => TEAL,
            (true, true, false) => WHITE,
            (true, false, _) => DIM_RED,
            (false, _, true) => DIM_TEAL,
            (false, true, false) => DIM_WHITE,
            (false, false, _) => OFF,
        };
        if current != desired {
            write_volatile(slot, desired);
        }
    }
}

/// Wraps the page group's cell getter (vtable slot `0x080678F8`), which
/// composes the 32 pads from its children each time the LEDs are read, so the
/// sample bank is recoloured before stock copies it.
#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn sample_bank_compose(group: *mut u8, a: u32, b: u32, c: u32) -> u64 {
    if !group.is_null() && read_u32(group, 0) == PAGE_GROUP_VTABLE {
        sample_bank_overlay(group);
    }
    function::<unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u64>(0x0804_dd1d)(group, a, b, c)
}

/// Wraps the layer's observer handler (primary vtable slot `0x08067218`),
/// which ends in the render.
#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn pair_track_buttons_notify(
    layer: *mut u8,
    source: u32,
    a: u32,
    b: u32,
) -> u32 {
    let result =
        function::<unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u32>(0x0804_c2df)(layer, source, a, b);
    if !layer.is_null() && read_u32(layer, 0) == TRACK_BUTTONS_VTABLE {
        pair_track_buttons_overlay(layer);
    }
    result
}

/// Same, for the observer-interface thunk (slot `0x08067228`, `this` = layer + 8).
#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn pair_track_buttons_notify_thunk(
    observer: *mut u8,
    source: u32,
    a: u32,
    b: u32,
) -> u32 {
    let result = function::<unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u32>(0x0804_c2dd)(
        observer, source, a, b,
    );
    if !observer.is_null() {
        let layer = observer.sub(8);
        if read_u32(layer, 0) == TRACK_BUTTONS_VTABLE {
            pair_track_buttons_overlay(layer);
        }
    }
    result
}

/// Re-applies the pair lighting from the main loop, so pairing or unpairing
/// with the leader selected shows at once.
#[cfg(stereo_position_poc)]
unsafe fn service_pair_track_buttons() {
    pair_track_buttons_overlay(track_buttons_layer());
}

#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn sidechain_pair_render(controller: *mut u8) -> u32 {
    const STOCK_SIDECHAIN_RENDER: usize = 0x0805_57b1;
    const OUTPUT_OWNER: usize = 0x04;
    const PAGE_SOURCE: usize = 0x30;
    const SOURCE_CONTROLLER: usize = 0x2c;
    const SOURCE_VALUES: usize = 0x0c;
    const ACTIVE_TARGET: usize = 0x14;
    const LEADER_PRESETS: usize = 0x20;
    const FOLLOWER_PRESETS: usize = 0x30;

    // Every pair is kept converged, not just one whose halves both happen to be
    // displayed, so a pair spanning two pages stays coupled.
    if !controller.is_null() {
        latch_sidechain_controller(controller);
        service_sidechain_residue();
        let mut leader = 0u32;
        while leader < 7 {
            if paired_pair_enabled(PLAYBACK_LEADER_TRACK + leader) {
                sync_sidechain_pair(controller, leader);
            }
            leader += 1;
        }
    }

    let result = function::<FxRender>(STOCK_SIDECHAIN_RENDER)(controller);
    if controller.is_null() {
        return result;
    }
    let output_owner = read_u32(controller, OUTPUT_OWNER) as *const u8;
    let page_source = read_u32(controller, PAGE_SOURCE);
    if output_owner.is_null() || page_source == 0 {
        return result;
    }
    let colors = read_u32(output_owner, 0) as *mut u8;
    let page_value = function::<ObservableValue>(0x0803_fc59)(page_source, 0);
    if colors.is_null() || page_value.is_null() || *page_value >= 4 {
        return result;
    }
    let leader = *page_value as u32 * 2;
    if paired_pair_enabled(PLAYBACK_LEADER_TRACK + leader) {
        let mut column = 0;
        while column < 8 {
            write_u16(
                colors,
                FOLLOWER_PRESETS + column * 2,
                read_u16(colors, LEADER_PRESETS + column * 2),
            );
            column += 1;
        }
    }

    // The copy above only reaches a pair whose halves share a page. A pair
    // split across two pages has no leader row in this buffer to copy from, so
    // the follower's own track colour is repainted as the leader's instead.
    // After the copy has run nothing still matches the follower's colour, so
    // this is a no-op in the same-page case.
    let mut row = 0u32;
    while row < 2 {
        let track = leader + row;
        if let Some(pair_leader) = paired_track_leader_index(track) {
            if pair_leader != track {
                let base = LEADER_PRESETS + row as usize * (FOLLOWER_PRESETS - LEADER_PRESETS);
                recolor_track_row(colors, base, track, pair_leader);
            }
        }
        row += 1;
    }

    let source_controller = read_u32(controller, SOURCE_CONTROLLER) as *const u8;
    if source_controller.is_null() {
        return result;
    }
    let source_values = read_u32(source_controller, SOURCE_VALUES);
    let active_target = read_u8(source_controller, ACTIVE_TARGET) as u32;
    if source_values == 0 || active_target >= 8 {
        return result;
    }
    let source_value = function::<ObservableValue>(0x0803_fc59)(source_values, active_target);
    if source_value.is_null() || *source_value >= 8 {
        return result;
    }
    let source = *source_value as usize;
    if let Some(source_leader) = paired_track_leader_index(source as u32) {
        // Take the colour from the table rather than from whichever member's
        // word happens to be in the buffer, so the pair always shows the
        // leader's hue even when the follower was the selected source.
        let selected_color = track_color(source_leader, false);
        let source_leader = source_leader as usize;
        write_u16(colors, source_leader * 2, selected_color);
        write_u16(colors, (source_leader + 1) * 2, selected_color);
    }
    result
}

#[cfg(mixer_volume_probe)]
#[no_mangle]
pub unsafe extern "C" fn fx_send_outer_probe(
    controller: *mut u8,
    knob: u32,
    delta: i32,
) {
    const STOCK_OUTER_KNOB: usize = 0x0805_a297;
    const DESCRIPTOR_BASE: usize = 0x2c;
    const DESCRIPTOR_SIZE: usize = 8;

    let state = pair_state();
    let mut page = 0xff;
    let mut selection = 0xff;
    let mut source_before = 0xff;
    let mut accumulator_before = 0xff;
    if !controller.is_null() {
        let read = function::<ObservableValue>(0x0803_fc59);
        let page_source = read_u32(controller, 0x0c);
        if page_source != 0 {
            let value = read(page_source, 0);
            if !value.is_null() {
                page = *value;
            }
        }
        let selection_source = read_u32(controller, 0x10);
        if selection_source != 0 {
            let value = read(selection_source, 0);
            if !value.is_null() {
                selection = *value;
            }
        }
        if knob < 8 && selection != 0xff {
            let descriptor = controller.add(DESCRIPTOR_BASE + knob as usize * DESCRIPTOR_SIZE);
            let source = read_u32(controller, 0x20);
            if read_u8(descriptor, 0) != 0xff && source != 0 {
                let value = read(source, read_u8(descriptor, 1) as u32 + 7);
                if !value.is_null() {
                    source_before = *value;
                }
                accumulator_before = read_u8(descriptor, 4);
            }
        }
    }
    if !state.is_null() {
        write_u8(
            state,
            STATE_MIXER_PROBE_FLAGS,
            read_u8(state, STATE_MIXER_PROBE_FLAGS) | 4,
        );
        write_u8(state, STATE_MIXER_PROBE_PATH, 3);
        write_u8(state, STATE_MIXER_PROBE_MODE, page);
        write_u8(state, STATE_MIXER_PROBE_KNOB, knob as u8);
        write_u8(
            state,
            STATE_MIXER_PROBE_DELTA,
            if delta < 0 { 1 } else if delta > 0 { 2 } else { 0 },
        );
        write_u8(state, STATE_MIXER_PROBE_18_BEFORE, source_before);
        write_u8(state, STATE_MIXER_PROBE_1C_BEFORE, accumulator_before);
        write_u8(state, STATE_MIXER_PROBE_VTABLE_COUNT, selection);
    }

    function::<FxKnobInput>(STOCK_OUTER_KNOB)(controller, knob, delta);
    if !state.is_null() && !controller.is_null() && knob < 8 && selection != 0xff {
        let descriptor = controller.add(DESCRIPTOR_BASE + knob as usize * DESCRIPTOR_SIZE);
        let source = read_u32(controller, 0x20);
        let mut source_after = 0xff;
        if read_u8(descriptor, 0) != 0xff && source != 0 {
            let value = function::<ObservableValue>(0x0803_fc59)(
                source,
                read_u8(descriptor, 1) as u32 + 7,
            );
            if !value.is_null() {
                source_after = *value;
            }
        }
        write_u8(state, STATE_MIXER_PROBE_18_AFTER, source_after);
        write_u8(
            state,
            STATE_MIXER_PROBE_1C_AFTER,
            read_u8(descriptor, 4),
        );
    }
    fx_send_probe_overlay(controller);
}

#[cfg(mixer_volume_probe)]
#[no_mangle]
pub unsafe extern "C" fn fx_send_render_probe(controller: *mut u8) -> u32 {
    const STOCK_FX_RENDER: usize = 0x0805_a2f1;

    let result = function::<FxRender>(STOCK_FX_RENDER)(controller);
    fx_send_probe_overlay(controller);
    result
}

#[cfg(mixer_volume_probe)]
#[no_mangle]
pub unsafe extern "C" fn root_knob_probe(root: *mut u8, knob: u32, delta: i32) {
    const STOCK_ROOT_KNOB: usize = 0x0803_dde1;

    let state = pair_state();
    if !state.is_null() {
        let mut child = if root.is_null() {
            0
        } else {
            read_u32(root, 0x7c)
        };
        if child == 0 && !root.is_null() {
            child = read_u32(root, 0x78);
        }
        let vtable = if child == 0 {
            0
        } else {
            read_u32(child as *const u8, 0)
        };
        let method = if vtable == 0 {
            0
        } else {
            read_u32(vtable as *const u8, 0x10)
        };
        write_u8(
            state,
            STATE_MIXER_PROBE_FLAGS,
            read_u8(state, STATE_MIXER_PROBE_FLAGS) | 8,
        );
        write_u32(state, STATE_MIXER_PROBE_18_BEFORE, method);
        write_u8(state, STATE_MIXER_PROBE_KNOB, knob as u8);
        write_u8(
            state,
            STATE_MIXER_PROBE_DELTA,
            if delta < 0 { 1 } else if delta > 0 { 2 } else { 0 },
        );
    }
    function::<RootKnobInput>(STOCK_ROOT_KNOB)(root, knob, delta);
}

#[cfg(mixer_volume_probe)]
unsafe fn fx_send_probe_overlay(controller: *mut u8) {
    const PROBE_COLOR: u16 = 0x00a1;

    let state = pair_state();
    if controller.is_null()
        || state.is_null()
        || read_u8(state, STATE_MIXER_PROBE_FLAGS) & 4 == 0
    {
        return;
    }
    let output_owner = read_u32(controller, 0x04) as *const u8;
    if output_owner.is_null() {
        return;
    }
    let colors = read_u32(output_owner, 0) as *mut u8;
    if colors.is_null() {
        return;
    }
    let indicators = mixer_volume_probe_indicators(state);
    let mut index = 0;
    while index < 8 {
        write_u16(
            colors,
            index * 2,
            if indicators[index] { PROBE_COLOR } else { 0 },
        );
        index += 1;
    }
}

#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn stereo_position_grid_render(
    mixer: *mut u8,
    output: u32,
    event: u32,
    context: u32,
) -> u64 {
    const STOCK_MIXER_RENDER: usize = 0x0805_0edb;
    const MUTE_LED_BASE: usize = 0x00;
    const MACRO_LED_BASE: usize = 0x44;

    // Undo the previous frame's mirror first, so the stock renderer sees the
    // persistent buffer exactly as it left it and the words read back below are
    // genuinely stock's, never our own residue.
    let pre_colors = control_color_buffer(mixer);
    if !pre_colors.is_null() {
        paired_led_restore(pre_colors, MUTE_LED_BASE, PAIR_LED_GROUP_MIXER_MUTE);
        paired_led_restore(pre_colors, MACRO_LED_BASE, PAIR_LED_GROUP_MIXER_MACRO);
    }

    let result = function::<GridRender>(STOCK_MIXER_RENDER)(mixer, output, event, context);
    if mixer.is_null() {
        return result;
    }
    let colors = control_color_buffer(mixer);
    if colors.is_null() {
        return result;
    }
    paired_led_mirror(colors, MUTE_LED_BASE, PAIR_LED_GROUP_MIXER_MUTE);
    paired_led_mirror(colors, MACRO_LED_BASE, PAIR_LED_GROUP_MIXER_MACRO);
    #[cfg(mixer_volume_probe)]
    mixer_volume_probe_overlay(colors);
    result
}

/// Resolves a control's output colour buffer: `controller[4]` is the output
/// owner and its first word is the buffer.
#[cfg(stereo_position_poc)]
unsafe fn control_color_buffer(controller: *const u8) -> *mut u8 {
    const OUTPUT_OWNER: usize = 0x04;

    if controller.is_null() {
        return core::ptr::null_mut();
    }
    let output_owner = read_u32(controller, OUTPUT_OWNER) as *const u8;
    if output_owner.is_null() {
        return core::ptr::null_mut();
    }
    read_u32(output_owner, 0) as *mut u8
}

/// Restores the original words this mirror site overwrote on the previous
/// frame. Must run before the stock renderer.
#[cfg(stereo_position_poc)]
unsafe fn paired_led_restore(colors: *mut u8, base: usize, group: usize) {
    let state = pair_state();
    if colors.is_null() || state.is_null() {
        return;
    }
    let slot = STATE_PAIR_LED_SAVE + group * PAIR_LED_GROUP_STRIDE;
    let mask = read_u8(state, slot);
    if mask == 0 {
        return;
    }
    let mut track = 0usize;
    while track < 8 {
        if mask & (1 << track) != 0 {
            write_u16(
                colors,
                base + track * 2,
                read_u16(state, slot + 4 + track * 2),
            );
        }
        track += 1;
    }
    write_u8(state, slot, 0);
}

/// Copies each leader's word onto its follower, saving what was there so the
/// next frame can put it back.
///
/// Any adjacent pair is permitted, so this walks leader indices rather than
/// even-aligned pairs. Pairs can never overlap, so a follower is never itself a
/// leader and an ascending walk cannot chain one pair's colour into the next.
#[cfg(stereo_position_poc)]
unsafe fn paired_led_mirror(colors: *mut u8, base: usize, group: usize) {
    let state = pair_state();
    if colors.is_null() || state.is_null() {
        return;
    }
    let slot = STATE_PAIR_LED_SAVE + group * PAIR_LED_GROUP_STRIDE;
    let mut mask = 0u8;
    let mut leader = 0usize;
    while leader < 7 {
        if paired_pair_enabled(PLAYBACK_LEADER_TRACK + leader as u32) {
            let follower = leader + 1;
            write_u16(
                state,
                slot + 4 + follower * 2,
                read_u16(colors, base + follower * 2),
            );
            mask |= 1 << follower;
            write_u16(
                colors,
                base + follower * 2,
                read_u16(colors, base + leader * 2),
            );
        }
        leader += 1;
    }
    write_u8(state, slot, mask);
}

#[cfg(mixer_volume_probe)]
unsafe fn mixer_volume_probe_overlay(colors: *mut u8) {
    const PROBE_COLOR: u16 = 0x00a1;
    const LOWER_PAD_BASE: usize = 0x20;
    const ENCODER_LED_BASE: usize = 0x44;

    let state = pair_state();
    if state.is_null() || read_u8(state, STATE_MIXER_PROBE_FLAGS) == 0 {
        return;
    }
    if read_u8(state, STATE_MIXER_PROBE_FLAGS) & 8 != 0 {
        let method = read_u32(state, STATE_MIXER_PROBE_18_BEFORE);
        let mut index = 0;
        while index < 32 {
            write_u16(
                colors,
                index * 2,
                if method & (1 << index) != 0 {
                    PROBE_COLOR
                } else {
                    0
                },
            );
            index += 1;
        }
        return;
    }
    let indicators = mixer_volume_probe_indicators(state);
    let mut index = 0;
    while index < 8 {
        write_u16(
            colors,
            index * 2,
            if indicators[index] { PROBE_COLOR } else { 0 },
        );
        write_u16(
            colors,
            ENCODER_LED_BASE + index * 2,
            if indicators[index] { PROBE_COLOR } else { 0 },
        );
        write_u16(colors, LOWER_PAD_BASE + index * 2, 0);
        index += 1;
    }
    let knob = read_u8(state, STATE_MIXER_PROBE_KNOB) as usize;
    if knob < 8 {
        write_u16(colors, LOWER_PAD_BASE + knob * 2, PROBE_COLOR);
    }
}

#[cfg(mixer_volume_probe)]
unsafe fn mixer_volume_probe_indicators(state: *const u8) -> [bool; 8] {
    let flags = read_u8(state, STATE_MIXER_PROBE_FLAGS);
    let mode = read_u8(state, STATE_MIXER_PROBE_MODE);
    let delta = read_u8(state, STATE_MIXER_PROBE_DELTA);
    if flags & 4 != 0 {
        let selection = read_u8(state, STATE_MIXER_PROBE_VTABLE_COUNT);
        [
            true,
            mode == 7,
            selection == 0xff,
            selection != 0xff,
            delta == 1,
            delta == 2,
            read_u8(state, STATE_MIXER_PROBE_18_BEFORE)
                != read_u8(state, STATE_MIXER_PROBE_18_AFTER),
            read_u8(state, STATE_MIXER_PROBE_1C_BEFORE)
                != read_u8(state, STATE_MIXER_PROBE_1C_AFTER),
        ]
    } else {
        [
            flags & 1 != 0,
            flags & 2 != 0,
            mode == 0,
            mode != 0 && mode != 0xff,
            delta == 1,
            delta == 2,
            read_u8(state, STATE_MIXER_PROBE_18_BEFORE)
                != read_u8(state, STATE_MIXER_PROBE_18_AFTER),
            read_u8(state, STATE_MIXER_PROBE_1C_BEFORE)
                != read_u8(state, STATE_MIXER_PROBE_1C_AFTER),
        ]
    }
}
#[cfg(stereo_position_poc)]
#[no_mangle]
pub unsafe extern "C" fn stereo_position_mixer_press(
    mixer: *mut u8,
    pad: u32,
    event: u32,
    context: u32,
) -> u32 {
    const STOCK_MIXER_PRESS: usize = 0x0805_0c39;

    let stock = function::<MixerPadPress>(STOCK_MIXER_PRESS);
    if mixer.is_null() || pad >= 8 {
        return stock(mixer, pad, event, context);
    }
    let Some(leader_index) = paired_track_leader_index(pad) else {
        return stock(mixer, pad, event, context);
    };

    let result = stock(mixer, leader_index, event, context);
    sync_mixer_mute(mixer, leader_index as usize);
    result
}
#[cfg(stereo_position_poc)]
unsafe fn sync_mixer_mute(mixer: *mut u8, leader_index: usize) {
    const OBSERVABLE_READ: usize = 0x0803_fc59;
    const OBSERVABLE_VALUE: usize = 0x0803_fd27;
    const OBSERVABLE_PUBLISH: usize = 0x0803_fbed;
    const MUTE_STRIDE: usize = 0x1d0;
    const MUTE_TABLE: usize = 0x1d8;

    let stride = read_u32(mixer, MUTE_STRIDE) as usize;
    let table = read_u32(mixer, MUTE_TABLE) as *const u8;
    if stride == 0 || table.is_null() {
        return;
    }
    let leader_observable = read_u32(table, leader_index * stride);
    let follower_observable = read_u32(table, (leader_index + 1) * stride);
    if leader_observable == 0 || follower_observable == 0 {
        return;
    }
    let leader_value =
        function::<ObservableValue>(OBSERVABLE_READ)(leader_observable, 0);
    let follower_value =
        function::<ObservableValue>(OBSERVABLE_VALUE)(follower_observable, 0);
    if leader_value.is_null() || follower_value.is_null() {
        return;
    }
    let value = read_volatile(leader_value);
    if read_volatile(follower_value) != value {
        write_volatile(follower_value, value);
        function::<FirmwareMethod4>(OBSERVABLE_PUBLISH)(follower_observable, 0, 0, 0);
    }
}

#[cfg(stereo_position_poc)]
#[inline(always)]
fn stereo_left_pan(position: u32) -> u32 {
    if position <= 64 {
        0
    } else {
        ((position - 64) * 127 + 31) / 63
    }
}

#[cfg(stereo_position_poc)]
#[inline(always)]
fn stereo_right_pan(position: u32) -> u32 {
    if position >= 64 {
        127
    } else {
        (position * 127 + 32) / 64
    }
}

#[cfg(auto_pan_poc)]
unsafe fn enforce_stereo_pan(receiver: *mut u8, leader_track: u32) {
    const TRACK_PARAMETER_GET: usize = 0x0804_6871;
    const TRACK_PARAMETER_SET: usize = 0x0803_1a3b;
    const PAN_PARAMETER: u32 = 17;
    const HARD_LEFT: u32 = 0;
    const HARD_RIGHT: u32 = 127;

    let get = function::<TrackParameterGet>(TRACK_PARAMETER_GET);
    let set = function::<TrackParameterSet>(TRACK_PARAMETER_SET);
    if get(leader_track, PAN_PARAMETER) != HARD_LEFT {
        set(receiver, leader_track, PAN_PARAMETER, HARD_LEFT);
    }
    if get(leader_track + 1, PAN_PARAMETER) != HARD_RIGHT {
        set(receiver, leader_track + 1, PAN_PARAMETER, HARD_RIGHT);
    }
}

#[cfg(paired_playback_poc)]
#[no_mangle]
pub unsafe extern "C" fn paired_playback_note_off(
    receiver: *mut u8,
    track: u32,
    note: u32,
) {
    let stock = function::<TrackNoteOffAdapter>(0x0803_1a23);
    if paired_follower_track(track) {
        return;
    }
    stock(receiver, track, note);
    let Some(pair) = playback_pair_index(track) else {
        return
    };
    let state = pair_state();
    if state.is_null() {
        return;
    }
    let latch = read_u8(state, STATE_PLAYBACK_LATCHES + pair);
    if latch & PLAYBACK_LATCH_ACTIVE != 0
        && (latch & PLAYBACK_LATCH_NOTE_MASK) == (note as u8 & PLAYBACK_LATCH_NOTE_MASK)
    {
        stock(receiver, track + 1, note);
        write_u8(state, STATE_PLAYBACK_LATCHES + pair, 0);
    }
}

#[cfg(sample_pair_led_poc)]
core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb
    .section .text.sample_pair_render,"ax",%progbits
    .global sample_pair_render
    .type sample_pair_render,%function
    .thumb_func
sample_pair_render:
    push {{r4, r5, r6, lr}}
    mov r6, r0
    ldr r4, 1f
    blx r4
    mov r4, r0
    mov r5, r1
    mov r0, r6
    mov r1, r4
    bl sample_pair_led_overlay
    mov r0, r4
    mov r1, r5
    pop {{r4, r5, r6, pc}}
    .align 2
1:
    .word 0x0804fe9b
    .size sample_pair_render, .-sample_pair_render
"#
);

#[cfg(sample_pair_identity_poc)]
core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb
    .section .text.sample_pair_wav_validate,"ax",%progbits
    .global sample_pair_wav_validate
    .type sample_pair_wav_validate,%function
    .thumb_func
sample_pair_wav_validate:
    push {{r4-r6, lr}}
    mov r4, r0
    ldr r3, 1f
    blx r3
    mov r5, r0
    mov r6, r1
    cbz r5, 2f
    mov r0, r4
    mov r1, r7
    bl sample_pair_note_wav_header
2:
    mov r0, r5
    mov r1, r6
    pop {{r4-r6, pc}}
    .align 2
1:
    .word 0x0805b2b5
    .size sample_pair_wav_validate, .-sample_pair_wav_validate
"#
);

#[cfg(sample_pair_gesture_poc)]
core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb

    .section .text.sample_pair_view_init_epilogue,"ax",%progbits
    .global sample_pair_view_init_epilogue
    .type sample_pair_view_init_epilogue,%function
    .thumb_func
sample_pair_view_init_epilogue:
    mov r0, r4
    movs r1, #0
    strh r1, [r4, #0x60]
    movs r1, #0x20
    strb r1, [r4, #0x62]
    movs r1, #0xff
    strb r1, [r4, #0x63]
    ldr r1, 1f
    str r1, [r4, #0x64]
    add sp, #0x0c
    pop.w {{r4-r11, pc}}
    .align 2
1:
    .word 0x50414952
    .size sample_pair_view_init_epilogue, .-sample_pair_view_init_epilogue

    .section .text.sample_pair_press,"ax",%progbits
    .global sample_pair_press
    .type sample_pair_press,%function
    .thumb_func
sample_pair_press:
    push {{r0-r6, lr}}
    bl sample_pair_clear_canonical_pad
    str r0, [sp, #4]
    ldr r0, [sp]
    ldr r1, [sp, #4]
    bl sample_pair_gesture_press_before_stock
    mov r6, r0
    ldr r0, [sp]
    ldrb r5, [r0, #0x0c]
    ldmia sp, {{r0-r3}}
    ldr r4, .Lsample_pair_press_stock
    blx r4
    str r0, [sp, #8]
    str r1, [sp, #12]
    ldr r0, [sp]
    ldrb r1, [r0, #0x0c]
    cmp r1, r5
    bne .Lsample_pair_press_clear
    cbnz r6, .Lsample_pair_press_after_canonical
    ldr r1, [sp, #4]
    bl sample_pair_save_preview_canonical
    mov r6, r0
.Lsample_pair_press_after_canonical:
    cbz r6, 2f
    subs r1, r6, #1
    ldr r0, [sp]
    bl sample_pair_gesture_select_canonical
2:
    ldr r0, [sp, #8]
    ldr r1, [sp, #12]
    add sp, #16
    pop {{r4-r6, pc}}
.Lsample_pair_press_clear:
    bl sample_pair_gesture_clear
    b 2b
    .align 2
.Lsample_pair_press_stock:
    .word 0x0804f9d9
    .size sample_pair_press, .-sample_pair_press

    .section .text.sample_pair_release,"ax",%progbits
    .global sample_pair_release
    .type sample_pair_release,%function
    .thumb_func
sample_pair_release:
    push {{r0, r1, r4-r6, lr}}
    ldr r4, 1f
    blx r4
    mov r4, r0
    mov r5, r1
    ldr r0, [sp]
    ldr r1, [sp, #4]
    bl sample_pair_gesture_release_after_stock
    mov r0, r4
    mov r1, r5
    add sp, #8
    pop {{r4-r6, pc}}
    .align 2
1:
    .word 0x0804fb27
    .size sample_pair_release, .-sample_pair_release

    .section .text.sample_pair_page_event,"ax",%progbits
    .global sample_pair_page_event
    .type sample_pair_page_event,%function
    .thumb_func
sample_pair_page_event:
    push {{r0-r6, lr}}
    ldrb r6, [r0, #0x0c]
    ldr r4, 1f
    blx r4
    mov r4, r0
    mov r5, r1
    ldr r0, [sp]
    ldrb r1, [r0, #0x0c]
    cmp r1, r6
    beq 2f
    bl sample_pair_gesture_clear
2:
    mov r0, r4
    mov r1, r5
    add sp, #16
    pop {{r4-r6, pc}}
    .align 2
1:
    .word 0x0804fd35
    .size sample_pair_page_event, .-sample_pair_page_event
"#
);

#[cfg(safe_pair_guard)]
core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb
    .section .text.stock_prepare_destination_trampoline,"ax",%progbits
    .global stock_prepare_destination_trampoline
    .type stock_prepare_destination_trampoline,%function
    .thumb_func
stock_prepare_destination_trampoline:
    push {{r4, r5, r6, lr}}
    mov r4, r0
    ldr r3, 1f
    bx r3
    .align 2
1:
    .word 0x0804138d
    .size stock_prepare_destination_trampoline, .-stock_prepare_destination_trampoline
"#
);

#[cfg(safe_pair_guard)]
extern "C" {
    fn stock_prepare_destination_trampoline(descriptor: *mut u8) -> u32;
}

extern "C" {
    fn stock_start_request_trampoline(
        manager: *mut u8,
        capture_slot: u32,
        descriptor: *const u8,
        completion: usize,
        completion_context: u32,
    ) -> u32;
    fn stock_pump_trampoline(
        manager: *mut u8,
        state: *mut u8,
        arg3: u32,
        arg4: u32,
    ) -> u32;
    fn stock_file_callback_trampoline(
        arg1: u32,
        arg2: u32,
        status: u32,
        arg4: u32,
        bytes_written: u32,
        state: *mut u8,
    );
}

core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb

    .section .text.stock_start_request_trampoline,"ax",%progbits
    .global stock_start_request_trampoline
    .type stock_start_request_trampoline,%function
    .thumb_func
stock_start_request_trampoline:
    push.w {{r4-r8, lr}}
    ldr r12, 1f
    bx r12
    .align 2
1:
    .word 0x0802f359
    .size stock_start_request_trampoline, .-stock_start_request_trampoline

    .section .text.stock_pump_trampoline,"ax",%progbits
    .global stock_pump_trampoline
    .type stock_pump_trampoline,%function
    .thumb_func
stock_pump_trampoline:
    push {{r1-r7, lr}}
    mov r7, r0
    ldr r12, 2f
    bx r12
    .align 2
2:
    .word 0x0802f455
    .size stock_pump_trampoline, .-stock_pump_trampoline

    .section .text.stock_file_callback_trampoline,"ax",%progbits
    .global stock_file_callback_trampoline
    .type stock_file_callback_trampoline,%function
    .thumb_func
stock_file_callback_trampoline:
    push {{r4, lr}}
    ldr r4, [sp, #0x0c]
    ldr r12, 3f
    bx r12
    .align 2
3:
    .word 0x0802f5d1
    .size stock_file_callback_trampoline, .-stock_file_callback_trampoline
"#
);

#[cfg(writer_request_probe_diagnostic)]
#[no_mangle]
pub unsafe extern "C" fn writer_request_probe_entry(
    manager: *mut u8,
    capture_slot: u32,
    descriptor: *const u8,
    completion: usize,
    completion_context: u32,
) -> u32 {
    if !manager.is_null() {
        let state = read_u32(manager, 0x04) as *mut u8;
        let source = read_u32(manager, 0x08) as *const u8;
        if !state.is_null() && !source.is_null() {
            write_u32(
                state,
                STATE_WRITER_REQUEST_SOURCE,
                (capture_slot & 0xff)
                    | ((read_u8(source, 4) as u32) << 8)
                    | (((completion as u32) & 0xffff) << 16),
            );
        }
    }
    stereo_start_request(
        manager,
        capture_slot,
        descriptor,
        completion,
        completion_context,
    )
}

#[inline(always)]
unsafe fn read_u8(base: *const u8, offset: usize) -> u8 {
    read_volatile(base.add(offset))
}

#[inline(always)]
unsafe fn read_u16(base: *const u8, offset: usize) -> u16 {
    read_volatile(base.add(offset).cast::<u16>())
}

#[inline(always)]
unsafe fn read_u32(base: *const u8, offset: usize) -> u32 {
    read_volatile(base.add(offset).cast::<u32>())
}

#[inline(always)]
unsafe fn write_u8(base: *mut u8, offset: usize, value: u8) {
    write_volatile(base.add(offset), value);
}

#[inline(always)]
unsafe fn write_u16(base: *mut u8, offset: usize, value: u16) {
    write_volatile(base.add(offset).cast::<u16>(), value);
}

#[inline(always)]
unsafe fn write_u32(base: *mut u8, offset: usize, value: u32) {
    write_volatile(base.add(offset).cast::<u32>(), value);
}

#[cfg(sample_pair_identity_poc)]
#[no_mangle]
pub unsafe extern "C" fn writer_state_constructor(
    state: *mut u8,
    owner: *mut u8,
) -> *mut u8 {
    const STOCK_WRITER_STATE_CONSTRUCTOR: usize = 0x0804_5e55;

    // The writer state is constructed lazily, on the first save after boot, and
    // that construction lands *between* the capture start that sets the stereo
    // save latch and the right slot's Save completion that consumes it. It also
    // clears `STATE_ARM_TOKEN`, so the gesture-owned path is guaranteed to be
    // dead for the first recording of every session - which is exactly the
    // "first attempt after a reboot never links" symptom.
    //
    // Read the latch from the block the sampling manager still points at (the
    // pre-construction block, which is the one the capture start wrote to) and
    // carry it into the new state. This is correct whether the allocation is
    // reused in place or replaced, because the manager is only repointed once
    // the stock constructor returns.
    #[cfg(native_dual_capture_poc)]
    let (carried_pair_latch, carried_view_latch) = {
        let prior = pair_state();
        if prior.is_null() {
            (0, 0)
        } else {
            let pair = read_u32(prior, STATE_SAVE_PAIR_LATCH);
            if pair & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC {
                (pair, read_u32(prior, STATE_SAVE_VIEW_LATCH))
            } else {
                (0, 0)
            }
        }
    };

    // The registry is rebuilt only by a pack load, which happens once at boot.
    // Zeroing it below therefore discards every pair recovered from the WAV
    // sentinels, and does so lazily, mid-session - leaving reboot-loaded pairs
    // invisible to teal and pair-wide Clear while in-session recordings still
    // work, because those re-register through `registry_set_pair`. That is the
    // "association is lost after a power cycle" symptom. Carry the registry
    // across construction, exactly as the save latch above is carried.
    #[cfg(all(native_dual_capture_poc, sample_pair_identity_poc))]
    let carried_registry = {
        let prior = pair_state();
        if prior.is_null() || read_u8(prior, STATE_PAIR_VALID) == 0 {
            None
        } else {
            Some((
                prior,
                read_u8(prior, STATE_PAIR_ACTIVE_BANK),
                read_u8(prior, STATE_PAIR_ACTIVE_PACK),
                read_u32(prior, STATE_REGISTRY_GENERATION),
            ))
        }
    };

    // Same hazard as the registry above, for the deferred peer Clear. A
    // pair-wide Clear arms the partner here and lets the main loop delete it,
    // but the stock delete transaction in between can construct the writer
    // state - so the first Clear after a boot loses its partner while every
    // later one works, the block already being constructed. Gated on the
    // deliberate arming signature, never on an inferred one, because restoring
    // it causes a sample to be deleted.
    #[cfg(all(native_dual_capture_poc, sample_pair_identity_poc))]
    let carried_peer_clear = {
        let prior = pair_state();
        if prior.is_null() {
            None
        } else {
            let slot = read_u8(prior, STATE_PEER_CLEAR_SLOT);
            let ui = read_u32(prior, STATE_PEER_CLEAR_UI);
            if slot >= 128 || ui == 0 {
                None
            } else {
                Some((
                    ui,
                    slot,
                    read_u8(prior, STATE_PEER_CLEAR_ATTEMPTS),
                    read_u16(prior, STATE_PEER_CLEAR_TICKS),
                    read_u16(prior, STATE_PEER_CLEAR_BUSY_TICKS),
                ))
            }
        }
    };

    let state = function::<WriterStateConstructor>(STOCK_WRITER_STATE_CONSTRUCTOR)(state, owner);
    if !state.is_null() {
        write_u32(state, STATE_MAGIC, 0);
        #[cfg(sample_pair_writer_coupling_poc)]
        write_u32(state, STATE_ARM_TOKEN, 0);
        #[cfg(sample_pair_identity_poc)]
        {
            write_u32(state, STATE_REFRESH_TOKEN, 0);
            write_u32(state, STATE_HEADER_WRITE_MARKER, 0);
            write_u8(state, STATE_PAIR_ACTIVE_BANK, 0);
            write_u8(state, STATE_PAIR_ACTIVE_PACK, 0xff);
            write_u8(state, STATE_PAIR_VALID, 0);
            write_u8(state, STATE_SCAN_MODE, SCAN_MODE_IDLE);
            write_u8(state, STATE_SCAN_BANK, SCAN_BANK_NONE);
            write_u8(state, STATE_SCAN_PENDING_LEFT, 0xff);
            write_u32(state, STATE_SCAN_EPOCH, 0);
            write_u32(state, STATE_MUTATION_EPOCH, 0);
            write_u32(state, STATE_REGISTRY_GENERATION, 0);
            // Restored after the zeroing above, so the values survive whether
            // the stock constructor reuses the block in place or replaces it.
            #[cfg(all(native_dual_capture_poc, sample_pair_identity_poc))]
            if let Some((prior, active_bank, active_pack, generation)) = carried_registry {
                // Copied byte-by-byte straight from the prior block: a stack
                // buffer would need memcpy, which this payload cannot link.
                // Correct whether the stock constructor reused the allocation
                // in place (the copy is then a no-op) or replaced it.
                let mut offset = 0;
                while offset < PAIR_BANK_SIZE * 2 {
                    write_u8(
                        state,
                        STATE_PAIR_BANK_0 + offset,
                        read_u8(prior, STATE_PAIR_BANK_0 + offset),
                    );
                    offset += 1;
                }
                write_u8(state, STATE_PAIR_ACTIVE_BANK, active_bank);
                write_u8(state, STATE_PAIR_ACTIVE_PACK, active_pack);
                write_u8(state, STATE_PAIR_VALID, 1);
                write_u32(state, STATE_REGISTRY_GENERATION, generation);
            }
            write_u8(state, STATE_WRITER_MODE, WRITER_MODE_IDLE);
            write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
            write_u8(state, STATE_WRITER_WAIT_TICKS, 0);
            write_u8(state, STATE_CLEAR_PHASE, CLEAR_PHASE_IDLE);
            write_u8(state, STATE_CLEAR_PACK, 0xff);
            write_u8(state, STATE_CLEAR_LEFT, 0xff);
            write_u32(state, STATE_CLEAR_COORDINATOR, 0);
            write_u32(state, STATE_CLEAR_COMPLETION, 0);
            write_u32(state, STATE_CLEAR_FIRST_STATUS, 0);
            // Must be an explicit sentinel: a zero-initialised block would
            // otherwise read as a pending peer Clear for slot 0.
            write_u32(state, STATE_PEER_CLEAR_UI, 0);
            write_u8(state, STATE_PEER_CLEAR_SLOT, PEER_CLEAR_NONE);
            write_u8(state, STATE_PEER_CLEAR_ATTEMPTS, 0);
            write_u16(state, STATE_PEER_CLEAR_TICKS, 0);
            write_u16(state, STATE_PEER_CLEAR_BUSY_TICKS, 0);
            #[cfg(all(native_dual_capture_poc, sample_pair_identity_poc))]
            if let Some((ui, slot, attempts, ticks, busy_ticks)) = carried_peer_clear {
                write_u32(state, STATE_PEER_CLEAR_UI, ui);
                write_u8(state, STATE_PEER_CLEAR_SLOT, slot);
                write_u8(state, STATE_PEER_CLEAR_ATTEMPTS, attempts);
                write_u16(state, STATE_PEER_CLEAR_TICKS, ticks);
                write_u16(state, STATE_PEER_CLEAR_BUSY_TICKS, busy_ticks);
            }
            #[cfg(all(native_dual_capture_poc, voice_channel_probe))]
            write_u16(state, STATE_VOICE_PROBE, 0);
            #[cfg(all(native_dual_capture_poc, crop_io_trace))]
            {
                let mut field = 0;
                while field < 152 {
                    write_u32(state, STATE_CROP_IO + field, 0);
                    field += 4;
                }
            }
            #[cfg(paired_playback_poc)]
            {
                let mut pair = 0;
                while pair < PLAYBACK_PAIR_COUNT {
                    write_u8(state, STATE_PLAYBACK_LATCHES + pair, 0);
                    pair += 1;
                }
            }
            #[cfg(stereo_position_poc)]
            {
                let mut offset = 0;
                while offset < PAIR_LED_GROUP_STRIDE * PAIR_LED_GROUPS {
                    write_u8(state, STATE_PAIR_LED_SAVE + offset, 0);
                    offset += 1;
                }
                let mut offset = 0;
                while offset < PAIR_SEND_SLOTS * PAIR_SEND_SLOT_STRIDE {
                    write_u8(state, STATE_PAIR_SEND_SLOTS + offset, 0);
                    offset += 1;
                }
                // The stock constructor does not blank this region, so a stale
                // controller pointer or pairing mask would otherwise survive a
                // reboot.
                write_u32(state, STATE_PAIR_VOLUME_CONTROLLER, 0);
                write_u8(state, STATE_PAIR_VOLUME_PENDING, 0);
                write_u32(state, STATE_PAIR_SIDECHAIN_CONTROLLER, 0);
                write_u8(state, STATE_PAIR_SIDECHAIN_MASK, 0);
                write_u8(state, STATE_PAIR_POSITION_MASK, 0);
            }
            #[cfg(mixer_volume_probe)]
            {
                let mut offset = STATE_MIXER_PROBE_FLAGS;
                while offset <= STATE_MIXER_PROBE_BRANCH_COUNT {
                    write_u8(state, offset, 0);
                    offset += 1;
                }
                write_u8(state, STATE_MIXER_PROBE_MODE, 0xff);
                write_u8(state, STATE_MIXER_PROBE_KNOB, 0xff);
                write_u8(state, STATE_MIXER_PROBE_18_BEFORE, 0xff);
                write_u8(state, STATE_MIXER_PROBE_18_AFTER, 0xff);
                write_u8(state, STATE_MIXER_PROBE_1C_BEFORE, 0xff);
                write_u8(state, STATE_MIXER_PROBE_1C_AFTER, 0xff);
            }
        }
        let mut offset = 0;
        while offset < PAIR_BANK_SIZE {
            write_u8(state, STATE_PAIR_BANK_0 + offset, 0);
            write_u8(state, STATE_PAIR_BANK_1 + offset, 0);
            offset += 1;
        }
        #[cfg(any(
            writer_request_probe_diagnostic,
            native_slot_metadata_probe_diagnostic
        ))]
        write_u32(state, STATE_WRITER_REQUEST_SOURCE, 0);
        #[cfg(native_dual_capture_poc)]
        {
            write_u32(state, STATE_CAPTURE_UI_TOKEN, 0);
            write_u32(state, STATE_DEFERRED_REFRESH_VIEW, 0);
            write_u32(state, STATE_SAVE_PAIR_LATCH, carried_pair_latch);
            write_u32(state, STATE_SAVE_VIEW_LATCH, carried_view_latch);
        }
        #[cfg(native_capture_preflight_probe_diagnostic)]
        write_u32(
            state,
            STATE_CAPTURE_PREFLIGHT_PROBE,
            CAPTURE_PREFLIGHT_PROBE_MAGIC,
        );
        #[cfg(native_writer_readiness_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_WRITER_PROBE,
            NATIVE_WRITER_PROBE_MAGIC,
        );
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_WRITER_ACQUISITION_PROBE,
            NATIVE_WRITER_ACQUISITION_PROBE_MAGIC,
        );
        #[cfg(native_arm_token_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_ARM_TOKEN_PROBE,
            NATIVE_ARM_TOKEN_PROBE_MAGIC,
        );
        #[cfg(native_pack_value_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_PACK_VALUE_PROBE,
            NATIVE_PACK_VALUE_PROBE_MAGIC,
        );
        #[cfg(native_pack_correction_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_PACK_CORRECTION_PROBE,
            NATIVE_PACK_CORRECTION_PROBE_MAGIC,
        );
        #[cfg(native_finalization_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_FINALIZATION_PROBE,
            NATIVE_FINALIZATION_PROBE_MAGIC,
        );
        #[cfg(native_publication_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_PUBLICATION_PROBE,
            NATIVE_PUBLICATION_PROBE_MAGIC,
        );
        #[cfg(native_terminal_publish_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_TERMINAL_PUBLISH_PROBE,
            NATIVE_TERMINAL_PUBLISH_PROBE_MAGIC,
        );
        #[cfg(native_publish_boundary_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_PUBLISH_BOUNDARY_PROBE,
            NATIVE_PUBLISH_BOUNDARY_PROBE_MAGIC,
        );
        #[cfg(native_publish_slot_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_PUBLISH_SLOT_PROBE,
            NATIVE_PUBLISH_SLOT_PROBE_MAGIC,
        );
    }
    state
}

#[cfg(sample_pair_led_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_led_overlay(view: *mut u8, output: *mut u16) {
    const PAGE: usize = 0x0c;
    const COLOR_OWNER: usize = 0x04;
    const PAGE_8: u8 = 7;
    const OCCUPIED_SELECTED: u16 = 0x00a1;
    const OCCUPIED_UNSELECTED: u16 = 0x00a2;
    const PAIR_COLOR: u16 = 0x0026;
    const PAIR_SELECTED_COLOR: u16 = 0x0025;
    const SAVING_COLOR: u16 = 0x0022;
    const ARMED_PAIR_COLOR: u16 = 0x0055;

    if view.is_null() {
        return;
    }

    // Runtime identity probe for the Sample view's encoder delegate.
    //
    // `FUN_0804F9D8` splits its control range two ways:
    //   controls 0x10..0x17 -> `view+0x1c` slot 0, the macro *buttons*
    //   controls >=0x18     -> `view+0x18` slot 0, the *encoders*
    //
    // `view+0x1c` was resolved by probe `-a` to `FUN_080596C0`, which is only a
    // 4-way selector and ignores the delta - so the start/length values are
    // written through `view+0x18` instead. Both objects are connected at
    // runtime and unresolvable from the image; this reads the encoder one.
    //
    // Deliberately READ-ONLY and confined to the UI render path. It writes no
    // state at all, which is the lesson from `stereo-voiceprobe-d`: that build
    // crashed because it wrote through the unvalidated `pair_state()` on every
    // note-on. Nothing here touches an audio path.
    //
    //   page 6 : the bound byte at interface+8 (expected: the slot number)
    //   page 7 : bits 0-15  of the slot-0 function pointer
    //   page 8 : bits 16-31 of the slot-0 function pointer
    // Pad 1 is the low bit of the displayed value.
    // Storage-I/O trace read-out. Pages 1-8, pad 1 = bit 0, lit = 1.
    #[cfg(all(native_dual_capture_poc, crop_io_trace))]
    {
        let page = read_u8(view, PAGE);
        if page <= PAGE_8 {
            if output.is_null() {
                return;
            }
            let state = pair_state();
            let half = if state.is_null() {
                0
            } else {
                let _ = read_u32(state, CROP_IO_COUNTS);
                match page {
                    // Equality witnesses, folded so a difference in the high
                    // half cannot hide behind a 16-bit readout: dark means the
                    // partner's stored extent matches the leader's exactly.
                    // Only meaningful when page 7 shows a non-zero mirror count,
                    // which proves these fields were written at all.
                    // Pairs 0-15 as the scan left them: pad n = pair n-1,
                    // i.e. slots 2(n-1) and 2(n-1)+1.
                    0 => {
                        let bank = registry_bank(state, read_u8(state, STATE_PAIR_ACTIVE_BANK));
                        let mut bits = 0u32;
                        let mut pair = 0;
                        while pair < 16 {
                            if registry_bit_is_set(bank, pair) {
                                bits |= 1 << pair;
                            }
                            pair += 1;
                        }
                        bits
                    }
                    // Sentinels the scan actually observed, slots 0-15.
                    1 => read_u32(state, CROP_SCAN_LEFT) & 0xffff,
                    2 => read_u32(state, CROP_SCAN_RIGHT) & 0xffff,
                    // Read live at render time rather than from a stored field:
                    // the stored copy was only ever written by the crop path, so
                    // it read as zeros in any session without a crop.
                    3 => {
                        let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
                        let pack = if store.is_null() { 0xff } else { read_u8(store, 0) };
                        (core::cmp::min(read_u32(state, STATE_REGISTRY_GENERATION), 15))
                            | ((read_u8(state, STATE_PAIR_VALID) != 0) as u32) << 4
                            | ((read_u8(state, STATE_PAIR_ACTIVE_BANK) & 1) as u32) << 5
                            | ((read_u8(state, STATE_PAIR_ACTIVE_PACK) as u32) & 0xff) << 8
                            | ((pack as u32) & 0x7) << 6
                    }
                    // The predicate the real features use, evaluated live for
                    // pairs 0-15. Page 1 shows raw bits; this shows whether a
                    // lookup would actually succeed, so a disagreement between
                    // the two localises the failure to the guards rather than
                    // to the scan.
                    // Clear-path branch witness. Request side: pad 1 entered,
                    // pad 2 stock path, pad 3 rejected (phase busy), pad 4 pair
                    // recognised, pad 5 no pair, pad 6 custom-stereo reject,
                    // pad 7 reached arm, pad 8 peer armed. Service side: pad 9
                    // ran, pad 10 null UI, pad 11 peer not occupied, pad 12
                    // storage busy, pad 13 attempts exhausted, pad 14 issued.
                    4 => read_u32(state, CROP_REG_BITS) & 0xffff,
                    // Scan gate report: pad 1 entered, pad 3 bailed (mode busy),
                    // pad 4 load failed, pad 5 epoch moved, pad 6 committed,
                    // pad 7 inactive bank, pad 8 polarity, pads 9-12 result,
                    // pads 13/14 which entry point ran.
                    // Pair-lookup failure witness: pads 1-6 descriptor pack,
                    // pad 7 store null, pad 8 PAIR_VALID, pads 9-14 live store
                    // pack, pad 15 active pack is 0xff, pad 16 registry bit set.
                    5 => read_u32(state, CROP_REG_DIAG) & 0xffff,
                    6 => 0,
                    // Liveness witness. Sample-file operations are counted by
                    // the storage hook, so this is non-zero whenever the writer
                    // state exists and the trace is running. A dark page 8 here
                    // means the state block is missing and pages 1-7 carry no
                    // information; a lit one makes their darkness meaningful.
                    _ => read_u32(state, CROP_IO_COUNTS) & 0xffff,
                }
            };
            let mut bit = 0;
            while bit < 16 {
                write_volatile(
                    output.add(bit),
                    if half & (1 << bit) != 0 {
                        PAIR_COLOR
                    } else {
                        ARMED_PAIR_COLOR
                    },
                );
                bit += 1;
            }
            return;
        }
    }

    // Settles whether the DSP slot descriptor is the crop store. Word 0 is the
    // suspected sample base and word 1 the logical length; if a crop moves the
    // base and shortens the length, the descriptor *is* the store and mirroring
    // a pair is just a descriptor copy. Strictly read-only.
    #[cfg(all(native_dual_capture_poc, sample_pair_identity_poc, crop_extent_probe))]
    {
        const PAGE_5: u8 = 4;
        const PAGE_6: u8 = 5;
        const PAGE_7: u8 = 6;
        let page = read_u8(view, PAGE);
        if page == PAGE_5 || page == PAGE_6 || page == PAGE_7 || page == PAGE_8 {
            if output.is_null() {
                return;
            }
            let slot = sample_view_selected_slot(view);
            let half = if slot == 0xff {
                0
            } else {
                let word = if page == PAGE_5 || page == PAGE_6 {
                    native_slot_extent(slot as u16, 0)
                } else {
                    native_slot_extent(slot as u16, 1)
                };
                if page == PAGE_5 || page == PAGE_7 {
                    word & 0xffff
                } else {
                    word >> 16
                }
            };
            let mut bit = 0;
            while bit < 16 {
                write_volatile(
                    output.add(bit),
                    if half & (1 << bit) != 0 {
                        PAIR_COLOR
                    } else {
                        ARMED_PAIR_COLOR
                    },
                );
                bit += 1;
            }
            return;
        }
    }

    #[cfg(macro_delegate_probe)]
    {
        const MACRO_DELEGATE: usize = 0x18;
        const PAGE_6: u8 = 5;
        const PAGE_7: u8 = 6;
        let page = read_u8(view, PAGE);
        if page == PAGE_6 || page == PAGE_7 || page == PAGE_8 {
            if output.is_null() {
                return;
            }
            // `FUN_08069578`, resolved by probe `-c`, is a forwarding thunk:
            //   (**(iface + 4))(obj, id, value, *(u8 *)(iface + 8), ...)
            // so the real per-slot parameter setter is one hop further on, and
            // the byte at iface+8 is the value it binds in - expected to be the
            // slot number. Both are read here so a single flash settles both.
            const PARAM_INTERFACE: usize = 0x14;
            const FORWARD_TARGET: usize = 0x04;
            const BOUND_BYTE: usize = 0x08;
            let mut target = 0u32;
            let mut bound = 0u32;
            let delegate = read_u32(view, MACRO_DELEGATE) as *mut u8;
            if !delegate.is_null() {
                let iface = read_u32(delegate, PARAM_INTERFACE) as *mut u8;
                if !iface.is_null() {
                    bound = read_u8(iface, BOUND_BYTE) as u32;
                    let inner = read_u32(iface, FORWARD_TARGET) as *mut u8;
                    if !inner.is_null() {
                        let vtable = read_u32(inner, 0) as *mut u8;
                        if !vtable.is_null() {
                            target = read_u32(vtable, 0);
                        }
                    }
                }
            }
            let half = if page == PAGE_6 {
                bound
            } else if page == PAGE_7 {
                target & 0xffff
            } else {
                target >> 16
            };
            let mut bit = 0;
            while bit < 16 {
                write_volatile(
                    output.add(bit),
                    if half & (1 << bit) != 0 {
                        PAIR_COLOR
                    } else {
                        ARMED_PAIR_COLOR
                    },
                );
                bit += 1;
            }
            return;
        }
    }

    #[cfg(all(native_dual_capture_poc, voice_channel_probe))]
    if read_u8(view, PAGE) == PAGE_8 {
        if output.is_null() {
            return;
        }
        let state = pair_state();
        let probe = if state.is_null() {
            0
        } else {
            read_u16(state, STATE_VOICE_PROBE)
        };
        write_volatile(
            output,
            if probe & VOICE_PROBE_SEEN != 0 {
                PAIR_COLOR
            } else {
                ARMED_PAIR_COLOR
            },
        );
        let mut condition = 0;
        while condition < 15 {
            write_volatile(
                output.add(condition + 1),
                if probe & (1 << condition) != 0 {
                    PAIR_COLOR
                } else {
                    ARMED_PAIR_COLOR
                },
            );
            condition += 1;
        }
        return;
    }

    #[cfg(native_capture_preflight_probe_diagnostic)]
    if read_u8(view, PAGE) == PAGE_8 {
        if output.is_null() {
            return;
        }
        let state = pair_state();
        let probe = if state.is_null() {
            0
        } else {
            read_u32(state, STATE_CAPTURE_PREFLIGHT_PROBE)
        };
        if probe & 0xffff_0000 == CAPTURE_PREFLIGHT_PROBE_MAGIC {
            write_volatile(
                output,
                if probe & CAPTURE_PREFLIGHT_PROBE_ENTRY != 0 {
                    PAIR_COLOR
                } else {
                    ARMED_PAIR_COLOR
                },
            );
            let mut condition = 0;
            while condition < 15 {
                write_volatile(
                    output.add(condition + 1),
                    if probe & (1 << condition) != 0 {
                        PAIR_COLOR
                    } else {
                        ARMED_PAIR_COLOR
                    },
                );
                condition += 1;
            }
        } else {
            // Heartbeat: the probe build is live but no capture has written the
            // probe word yet. Distinguishes "probe never fired" from "overlay
            // never runs on this page".
            write_volatile(output, SAVING_COLOR);
        }
        return;
    }

    #[cfg(native_pack_correction_probe_diagnostic)]
    {
        if output.is_null() {
            return;
        }
        let state = pair_state();
        if !state.is_null() {
            let probe = read_u32(state, STATE_NATIVE_PACK_CORRECTION_PROBE);
            if probe & 0xff00_0001 == (NATIVE_PACK_CORRECTION_PROBE_MAGIC | 1)
                && read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_NATIVE_STEREO
            {
                let mut indicator = 0;
                while indicator < 12 {
                    write_volatile(
                        output.add(indicator),
                        if probe & (1 << indicator) != 0 {
                            PAIR_COLOR
                        } else {
                            ARMED_PAIR_COLOR
                        },
                    );
                    indicator += 1;
                }
                let phase = read_u8(state, STATE_WRITER_PHASE);
                let mut phase_bit = 0;
                while phase_bit < 4 {
                    write_volatile(
                        output.add(12 + phase_bit),
                        if phase & (1 << phase_bit) != 0 {
                            PAIR_COLOR
                        } else {
                            ARMED_PAIR_COLOR
                        },
                    );
                    phase_bit += 1;
                }
                return;
            }
        }
    }

    #[cfg(native_finalization_probe_diagnostic)]
    {
        if output.is_null() {
            return;
        }
        let state = pair_state();
        if !state.is_null() {
            let probe = read_u32(state, STATE_NATIVE_FINALIZATION_PROBE);
            if probe & 0xff01_0000 == (NATIVE_FINALIZATION_PROBE_MAGIC | (1 << 16))
                && read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_NATIVE_STEREO
            {
                let mut indicator = 0;
                while indicator < 16 {
                    write_volatile(
                        output.add(indicator),
                        if probe & (1 << indicator) != 0 {
                            PAIR_COLOR
                        } else {
                            ARMED_PAIR_COLOR
                        },
                    );
                    indicator += 1;
                }
                return;
            }
        }
    }

    #[cfg(native_publication_probe_diagnostic)]
    {
        if output.is_null() {
            return;
        }
        let state = pair_state();
        if !state.is_null() {
            let probe = read_u32(state, STATE_NATIVE_PUBLICATION_PROBE);
            if probe & 0xff01_0000 == (NATIVE_PUBLICATION_PROBE_MAGIC | (1 << 16)) {
                let mut indicator = 0;
                while indicator < 16 {
                    write_volatile(
                        output.add(indicator),
                        if probe & (1 << indicator) != 0 {
                            PAIR_COLOR
                        } else {
                            ARMED_PAIR_COLOR
                        },
                    );
                    indicator += 1;
                }
                return;
            }
        }
    }

    #[cfg(native_terminal_publish_probe_diagnostic)]
    {
        if output.is_null() {
            return;
        }
        let state = pair_state();
        if !state.is_null() {
            let probe = read_u32(state, STATE_NATIVE_TERMINAL_PUBLISH_PROBE);
            if probe & 0xff01_0000
                == (NATIVE_TERMINAL_PUBLISH_PROBE_MAGIC | (1 << 16))
            {
                let mut indicator = 0;
                while indicator < 16 {
                    write_volatile(
                        output.add(indicator),
                        if probe & (1 << indicator) != 0 {
                            PAIR_COLOR
                        } else {
                            ARMED_PAIR_COLOR
                        },
                    );
                    indicator += 1;
                }
                return;
            }
        }
    }

    #[cfg(native_publish_boundary_probe_diagnostic)]
    {
        if output.is_null() {
            return;
        }
        let state = pair_state();
        if !state.is_null() {
            let probe = read_u32(state, STATE_NATIVE_PUBLISH_BOUNDARY_PROBE);
            if probe & 0xff01_0000 == (NATIVE_PUBLISH_BOUNDARY_PROBE_MAGIC | (1 << 16))
            {
                let mut indicator = 0;
                while indicator < 16 {
                    write_volatile(
                        output.add(indicator),
                        if probe & (1 << indicator) != 0 {
                            PAIR_COLOR
                        } else {
                            ARMED_PAIR_COLOR
                        },
                    );
                    indicator += 1;
                }
                return;
            }
        }
    }

    #[cfg(native_publish_slot_probe_diagnostic)]
    {
        if output.is_null() {
            return;
        }
        let state = pair_state();
        if !state.is_null() {
            let probe = read_u32(state, STATE_NATIVE_PUBLISH_SLOT_PROBE);
            if probe & 0xff01_0000 == (NATIVE_PUBLISH_SLOT_PROBE_MAGIC | (1 << 16)) {
                let mut indicator = 0;
                while indicator < 16 {
                    write_volatile(
                        output.add(indicator),
                        if probe & (1 << indicator) != 0 {
                            PAIR_COLOR
                        } else {
                            ARMED_PAIR_COLOR
                        },
                    );
                    indicator += 1;
                }
                return;
            }
        }
    }

    #[cfg(native_deferred_refresh_probe_diagnostic)]
    {
        if output.is_null() {
            return;
        }
        let state = pair_state();
        if !state.is_null() {
            let mut probe = read_u32(state, STATE_CAPTURE_UI_TOKEN);
            if probe & 0xffff_0000 == NATIVE_DEFERRED_REFRESH_REPORT_MAGIC {
                let snapshot = read_u32(state, STATE_HEADER_WRITE_MARKER);
                let left = snapshot as u8;
                let right = left.wrapping_add(1);
                let pack = ((snapshot >> 8) & 0x1f) as u8;
                if snapshot & REFRESH_TOKEN_STEREO != 0
                    && is_stereo_pair_start(left)
                    && sample_view_slot_occupied(view, right)
                {
                    probe |= 1 << 9;
                }
                if is_stereo_pair_start(left) {
                    if registry_contains(state, pack, pair_index(left).unwrap_or(PAIR_COUNT)) {
                        probe |= 1 << 10;
                    }
                    probe |= 1 << 14;
                }
                if read_u32(state, STATE_REFRESH_TOKEN) == 0 {
                    probe |= 1 << 11;
                }
                if read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_IDLE {
                    probe |= 1 << 12;
                }
                if read_u8(state, STATE_ASYNC) == 8 {
                    probe |= 1 << 13;
                }
                let mut indicator = 0;
                while indicator < 16 {
                    write_volatile(
                        output.add(indicator),
                        if probe & (1 << indicator) != 0 {
                            PAIR_COLOR
                        } else {
                            ARMED_PAIR_COLOR
                        },
                    );
                    indicator += 1;
                }
                return;
            }
        }
    }

    #[cfg(sample_pair_gesture_poc)]
    if matches!(sample_view_mode(view), 2 | 3) {
        if output.is_null() {
            return;
        }
        #[cfg(not(any(
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
            native_publish_slot_probe_diagnostic
        )))]
        {
            let state = pair_state();
            if !state.is_null() {
                #[cfg(native_dual_capture_poc)]
                let token = read_u32(state, STATE_CAPTURE_UI_TOKEN);
                #[cfg(not(native_dual_capture_poc))]
                let token = read_u32(state, STATE_ARM_TOKEN);
                if token & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC {
                    let left = token as u8;
                    // No live-store pack comparison here. `Pack_Access` lazily
                    // mounts the pack and rewrites `store[0]` partway through a
                    // session, so comparing the capture-time pack against the
                    // live store made this mirror fail for the first recording
                    // after a cold boot and succeed for every later one — the
                    // N+1 pad stayed static instead of flashing red with N. The
                    // token's own pack is the trustworthy value, and the page
                    // check below already confines the mirror to the pads being
                    // drawn. See reports/first-recording-pack-mount-root-cause.md.
                    if left / 16 == read_u8(view, PAGE) && is_stereo_pair_start(left) {
                        let pad = (left & 0x0f) as usize;
                        write_volatile(output.add(pad + 1), read_volatile(output.add(pad)));
                    }
                }
            }
        }
        #[cfg(native_writer_readiness_probe_diagnostic)]
        {
            let state = pair_state();
            if !state.is_null() {
                let probe = read_u32(state, STATE_NATIVE_WRITER_PROBE);
                if probe & 0xff00_0000 == NATIVE_WRITER_PROBE_MAGIC {
                    let mut indicator = 0;
                    while indicator < 10 {
                        write_volatile(
                            output.add(indicator),
                            if probe & (1 << indicator) != 0 {
                                PAIR_COLOR
                            } else {
                                ARMED_PAIR_COLOR
                            },
                        );
                        indicator += 1;
                    }
                    let phase = (probe >> 10) & 0x0f;
                    let mut phase_bit = 0;
                    while phase_bit < 4 {
                        write_volatile(
                            output.add(10 + phase_bit),
                            if phase & (1 << phase_bit) != 0 {
                                PAIR_COLOR
                            } else {
                                ARMED_PAIR_COLOR
                            },
                        );
                        phase_bit += 1;
                    }
                    let async_state = (probe >> 14) & 0x0f;
                    write_volatile(
                        output.add(14),
                        if async_state == 0 { PAIR_COLOR } else { ARMED_PAIR_COLOR },
                    );
                    write_volatile(
                        output.add(15),
                        if async_state == 1 { PAIR_COLOR } else { ARMED_PAIR_COLOR },
                    );
                }
            }
        }
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        {
            let state = pair_state();
            if !state.is_null() {
                let probe = read_u32(state, STATE_NATIVE_WRITER_ACQUISITION_PROBE);
                if probe & 0xff00_0000 == NATIVE_WRITER_ACQUISITION_PROBE_MAGIC {
                    let mut indicator = 0;
                    while indicator < 16 {
                        write_volatile(
                            output.add(indicator),
                            if probe & (1 << indicator) != 0 {
                                PAIR_COLOR
                            } else {
                                ARMED_PAIR_COLOR
                            },
                        );
                        indicator += 1;
                    }
                }
            }
        }
        #[cfg(native_arm_token_probe_diagnostic)]
        {
            let state = pair_state();
            if !state.is_null() {
                let probe = read_u32(state, STATE_NATIVE_ARM_TOKEN_PROBE);
                if probe & 0xff00_0000 == NATIVE_ARM_TOKEN_PROBE_MAGIC {
                    let mut indicator = 0;
                    while indicator < 16 {
                        write_volatile(
                            output.add(indicator),
                            if probe & (1 << indicator) != 0 {
                                PAIR_COLOR
                            } else {
                                ARMED_PAIR_COLOR
                            },
                        );
                        indicator += 1;
                    }
                }
            }
        }
        #[cfg(native_pack_value_probe_diagnostic)]
        {
            let state = pair_state();
            if !state.is_null() {
                let probe = read_u32(state, STATE_NATIVE_PACK_VALUE_PROBE);
                if probe & 0xff00_0000 == NATIVE_PACK_VALUE_PROBE_MAGIC {
                    let mut indicator = 0;
                    while indicator < 16 {
                        write_volatile(
                            output.add(indicator),
                            if probe & (1 << indicator) != 0 {
                                PAIR_COLOR
                            } else {
                                ARMED_PAIR_COLOR
                            },
                        );
                        indicator += 1;
                    }
                }
            }
        }
        return;
    }

    let owner = read_u32(view, COLOR_OWNER) as *const u8;
    if owner.is_null() {
        return;
    }
    let colors = read_u32(owner, 0) as *mut u16;
    if colors.is_null() {
        return;
    }

    #[cfg(sample_pair_gesture_poc)]
    if !sample_view_mode_supported(view) {
        return;
    }
    #[cfg(not(sample_pair_gesture_poc))]
    if matches!(sample_view_mode(view), 2 | 3) {
        return;
    }

    #[cfg(sample_pair_gesture_poc)]
    let armed_pad = if read_u32(view, SAMPLE_VIEW_EXTENSION_MAGIC) == SAMPLE_VIEW_MAGIC {
        let armed_left = read_u8(view, SAMPLE_VIEW_ARMED_LEFT);
        let page = read_u8(view, PAGE);
        if armed_left != SAMPLE_VIEW_NO_ARM
            && armed_left / 16 == page
            && sample_pair_arm_is_active(armed_left)
        {
            Some((armed_left & 0x0f) as usize)
        } else {
            None
        }
    } else {
        None
    };

    #[cfg(not(sample_pair_writer_coupling_poc))]
    if read_u8(view, PAGE) != PAGE_8 {
        return;
    }

    #[cfg(sample_pair_identity_poc)]
    let (state, encoded_pack) = {
        let manager = read_volatile(SAMPLING_MANAGER) as *const u8;
        let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
        if manager.is_null() || store.is_null() {
            return;
        }
        let state = read_u32(manager, 0x04) as *mut u8;
        if state.is_null() {
            return;
        }
        // Byte 0 of the content store lags the actual pack load, so key the
        // overlay on the pack the boot scan bound into the registry.
        let pack = registry_pack(state);
        if pack >= 0x20 {
            return;
        }
        let encoded_pack = pack + 1;
        (state, encoded_pack)
    };
    #[cfg(sample_pair_identity_poc)]
    let selected_slot = sample_view_selected_slot(view);
    #[cfg(sample_pair_identity_poc)]
    let clear_active = sample_view_clear_active(view);

    #[cfg(sample_pair_writer_coupling_poc)]
    let mut pad = 0;
    #[cfg(not(sample_pair_writer_coupling_poc))]
    let mut pad = 0;
    while pad < 16 {
        #[cfg(sample_pair_writer_coupling_poc)]
        if pad & 7 == 7 {
            pad += 1;
            continue;
        }
        #[cfg(not(sample_pair_writer_coupling_poc))]
        let left_occupied = matches!(
            read_volatile(colors.add(pad)),
            OCCUPIED_SELECTED | OCCUPIED_UNSELECTED
        );
        #[cfg(not(sample_pair_writer_coupling_poc))]
        let right_occupied = matches!(
            read_volatile(colors.add(pad + 1)),
            OCCUPIED_SELECTED | OCCUPIED_UNSELECTED
        );
        #[cfg(sample_pair_identity_poc)]
        let identified = {
            #[cfg(sample_pair_writer_coupling_poc)]
            let pair = pair_index(read_u8(view, PAGE) * 16 + pad as u8);
            #[cfg(not(sample_pair_writer_coupling_poc))]
            let pair = Some(pad / 2);
            match pair {
                Some(pair) => registry_contains(state, encoded_pack - 1, pair),
                None => false,
            }
        };
        // A capture that has stopped but not yet finished saving is not in the
        // registry yet, so paint it from the live arm token instead. Both
        // occupancy bits are set the instant recording stops, which is exactly
        // when a stock mono capture starts showing as filled, so this makes the
        // pair look identical from stop time onwards — on the first capture
        // after a cold boot as well as every later one.
        #[cfg(all(
            sample_pair_identity_poc,
            stereo_stock_save_queue,
            sample_pair_writer_coupling_poc
        ))]
        let identified = identified || {
            let left = read_u8(view, PAGE) * 16 + pad as u8;
            armed_stereo_slot_side(left) == 1
                && sample_view_slot_occupied(view, left)
                && sample_view_slot_occupied(view, left + 1)
        };
        #[cfg(not(sample_pair_identity_poc))]
        let identified = true;
        #[cfg(sample_pair_identity_poc)]
        let visible = identified;
        #[cfg(not(sample_pair_identity_poc))]
        let visible = left_occupied && right_occupied;
        if visible {
            #[cfg(sample_pair_identity_poc)]
            let color = {
                let left = read_u8(view, PAGE) * 16 + pad as u8;
                let selected = selected_slot == left || selected_slot == left + 1;
                if clear_active && !selected {
                    pad += 1;
                    continue;
                }
                if selected {
                    PAIR_SELECTED_COLOR
                } else {
                    PAIR_COLOR
                }
            };
            #[cfg(not(sample_pair_identity_poc))]
            let color = PAIR_COLOR;
            write_volatile(colors.add(pad), color);
            write_volatile(colors.add(pad + 1), color);
        }
        #[cfg(sample_pair_writer_coupling_poc)]
        {
            pad += 1;
        }
        #[cfg(not(sample_pair_writer_coupling_poc))]
        {
            pad += 2;
        }
    }

    #[cfg(sample_pair_writer_coupling_poc)]
    {
        // The in-progress save used to be shown by flashing both pads green
        // (a diagnostic mirroring the Save button). Both pads now fill with
        // stereo teal the instant recording stops and stay that way through
        // the save, so the flash was removed at the user's request.
    }

    #[cfg(sample_pair_gesture_poc)]
    if let Some(left_pad) = armed_pad {
        // While a stereo capture is saving, the pair is already shown filled in
        // stereo teal and both channels are previewable. Painting the arm
        // indicator over it would hide that, and because
        // `sample_pair_arm_is_active` compares the arm token against the *live*
        // store pack, it survives only when no pack mount intervenes — so it
        // would also make the first recording of a session look different from
        // every later one. Suppressing it here keeps them identical.
        #[cfg(all(stereo_stock_save_queue, sample_pair_writer_coupling_poc))]
        let saving_stereo_pair = {
            let left = read_u8(view, PAGE) * 16 + left_pad as u8;
            armed_stereo_slot_side(left) == 1
                && (sample_slot_save_pending(left) || sample_slot_save_pending(left + 1))
        };
        #[cfg(not(all(stereo_stock_save_queue, sample_pair_writer_coupling_poc)))]
        let saving_stereo_pair = false;
        if !saving_stereo_pair {
            write_volatile(colors.add(left_pad), ARMED_PAIR_COLOR);
            write_volatile(colors.add(left_pad + 1), ARMED_PAIR_COLOR);
        }
    }

    #[cfg(sample_cache_refresh_poc)]
    {
        let state = pair_state();
        let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
        if !state.is_null() && !store.is_null() {
            let token = read_u32(state, STATE_REFRESH_TOKEN);
            let diagnostic = token & ARM_TOKEN_MAGIC_MASK;
            let pack = ((token >> 8) & 0x1f) as u8;
            let left = token as u8;
            let page_base = read_u8(view, PAGE) << 4;
            if matches!(
                diagnostic,
                REFRESH_DIAGNOSTIC_QUERY_MISS | REFRESH_DIAGNOSTIC_LOAD_FAILED
            ) && read_u8(store, 0) == pack
                && left >= page_base
                && left < page_base + 15
            {
                let left_pad = (left - page_base) as usize;
                let diagnostic_pad = if diagnostic == REFRESH_DIAGNOSTIC_QUERY_MISS {
                    left_pad
                } else {
                    left_pad + 1
                };
                write_volatile(colors.add(diagnostic_pad), ARMED_PAIR_COLOR);
            }
        }
    }

    #[cfg(native_slot_metadata_probe_diagnostic)]
    {
        let state = pair_state();
        if !state.is_null() {
            let probe = read_u32(state, STATE_WRITER_REQUEST_SOURCE);
            let right = probe as u8;
            if probe & 0xff00_0000 == NATIVE_METADATA_PROBE_MAGIC
                && right / 16 == read_u8(view, PAGE)
            {
                let mut indicator = 0;
                while indicator < 5 {
                    let color = if probe & (1 << (16 + indicator)) != 0 {
                        PAIR_COLOR
                    } else {
                        ARMED_PAIR_COLOR
                    };
                    write_volatile(colors.add(11 + indicator), color);
                    indicator += 1;
                }
            }

        }
    }

    #[cfg(all(paired_playback_poc, not(pan_parameter_probe)))]
    if read_u8(view, PAGE) == PAGE_8 {
        if let Some((leader_slot, follower_slot, enabled)) =
            paired_playback_assignment(PLAYBACK_LEADER_TRACK)
        {
            let mut bit = 0;
            while bit < 7 {
                if leader_slot & (1 << bit) != 0 {
                    write_volatile(colors.add(bit), SAVING_COLOR);
                }
                if follower_slot & (1 << bit) != 0 {
                    write_volatile(colors.add(8 + bit), SAVING_COLOR);
                }
                bit += 1;
            }
            if enabled {
                write_volatile(colors.add(7), SAVING_COLOR);
            }
        }
    }

    #[cfg(mixer_volume_probe)]
    {
        const PAGE_7: u8 = 6;

        let state = pair_state();
        let page = read_u8(view, PAGE);
        if !state.is_null()
            && read_u8(state, STATE_MIXER_PROBE_FLAGS) & 8 != 0
            && matches!(page, PAGE_7 | PAGE_8)
        {
            let method = read_u32(state, STATE_MIXER_PROBE_18_BEFORE);
            let shift = if page == PAGE_7 { 0 } else { 16 };
            let mut bit = 0;
            while bit < 16 {
                write_volatile(
                    colors.add(bit),
                    if method & (1 << (shift + bit)) != 0 {
                        SAVING_COLOR
                    } else {
                        0
                    },
                );
                bit += 1;
            }
        }

        #[cfg(writer_request_probe_diagnostic)]
        {
            const PAGE_7: u8 = 6;

            let state = pair_state();
            let page = read_u8(view, PAGE);
            if !state.is_null() && matches!(page, PAGE_7 | PAGE_8) {
                let source = read_u32(state, STATE_WRITER_REQUEST_SOURCE);
                let shift = if page == PAGE_7 { 0 } else { 16 };
                let mut bit = 0;
                while bit < 16 {
                    write_volatile(
                        colors.add(bit),
                        if source & (1 << (shift + bit)) != 0 {
                            SAVING_COLOR
                        } else {
                            0
                        },
                    );
                    bit += 1;
                }
                return;
            }
        }
    }

    #[cfg(pan_parameter_probe)]
    {
        const TRACK_PARAMETER_GET: usize = 0x0804_6871;
        const PAGE_7: u8 = 6;

        let page = read_u8(view, PAGE);
        let get = function::<TrackParameterGet>(TRACK_PARAMETER_GET);
        if page == PAGE_8 {
            let mut parameter = 0;
            while parameter < 16 {
                if get(PLAYBACK_LEADER_TRACK, parameter)
                    != get(PLAYBACK_LEADER_TRACK + 1, parameter)
                {
                    write_volatile(colors.add(parameter as usize), SAVING_COLOR);
                }
                parameter += 1;
            }
        } else if page == PAGE_7 {
            const PAN_PARAMETER: u32 = 17;

            let leader = get(PLAYBACK_LEADER_TRACK, PAN_PARAMETER) as u8;
            let follower = get(PLAYBACK_LEADER_TRACK + 1, PAN_PARAMETER) as u8;
            let mut bit = 0;
            while bit < 7 {
                if leader & (1 << bit) != 0 {
                    write_volatile(colors.add(bit), SAVING_COLOR);
                }
                if follower & (1 << bit) != 0 {
                    write_volatile(colors.add(8 + bit), SAVING_COLOR);
                }
                bit += 1;
            }
            if leader <= 127 {
                write_volatile(colors.add(7), SAVING_COLOR);
            }
            if follower <= 127 {
                write_volatile(colors.add(15), SAVING_COLOR);
            }
        }
    }

}

#[cfg(sample_pair_gesture_poc)]
#[inline(always)]
unsafe fn sample_view_extension_valid(view: *const u8) -> bool {
    !view.is_null() && read_u32(view, SAMPLE_VIEW_EXTENSION_MAGIC) == SAMPLE_VIEW_MAGIC
}

#[cfg(sample_pair_led_poc)]
#[inline(always)]
unsafe fn sample_view_mode(view: *const u8) -> u8 {
    const OBSERVABLE_READ: usize = 0x0803_fc59;
    const MODE: usize = 0x48;

    let mode = read_u32(view, MODE);
    if mode == 0 {
        return 0xff;
    }
    let mode_value = function::<ObservableValue>(OBSERVABLE_READ)(mode, 0);
    if mode_value.is_null() {
        0xff
    } else {
        *mode_value
    }
}

#[cfg(sample_pair_gesture_poc)]
#[inline(always)]
unsafe fn sample_view_mode_supported(view: *const u8) -> bool {
    !matches!(sample_view_mode(view), 2 | 3 | 0xff)
}

#[cfg(sample_pair_gesture_poc)]
#[inline(always)]
unsafe fn sample_view_plain_press(view: *const u8) -> bool {
    const OBSERVABLE_READ: usize = 0x0803_fc59;
    const MODIFIER_A: usize = 0x24;
    const MODIFIER_B: usize = 0x2c;

    if !sample_view_mode_supported(view) {
        return false;
    }
    let read = function::<ObservableValue>(OBSERVABLE_READ);
    let modifier_a = read_u32(view, MODIFIER_A);
    let modifier_b = read_u32(view, MODIFIER_B);
    if modifier_a == 0 || modifier_b == 0 {
        return true;
    }
    let modifier_a_value = read(modifier_a, 0);
    let modifier_b_value = read(modifier_b, 0);
    if modifier_a_value.is_null() || modifier_b_value.is_null() {
        return false;
    }
    ((*modifier_a_value & 0x04) == 0) || ((*modifier_b_value & 0x01) == 0)
}

#[cfg(sample_pair_writer_coupling_poc)]
#[inline(always)]
unsafe fn sample_view_slot_occupied(view: *const u8, slot: u8) -> bool {
    const STOCK_SAMPLE_SLOT_OCCUPIED: usize = 0x0805_985d;

    function::<SampleSlotOccupied>(STOCK_SAMPLE_SLOT_OCCUPIED)(view, slot as u32) != 0
}

#[cfg(sample_cache_refresh_poc)]
unsafe fn publish_sample_view_occupancy(view: *mut u8, slot: u8) {
    const OCCUPANCY: usize = 0x30;
    const OBSERVABLE_VALUE: usize = 0x0803_fd27;
    const OBSERVABLE_PUBLISH: usize = 0x0803_fbed;

    let observable = read_u32(view, OCCUPANCY);
    if observable == 0 {
        return;
    }
    let word = function::<ObservableValue>(OBSERVABLE_VALUE)(
        observable,
        (slot >> 5) as u32,
    )
    .cast::<u32>();
    if word.is_null() {
        return;
    }
    write_volatile(
        word,
        read_volatile(word) | (1u32 << (slot & 31)),
    );
    function::<FirmwareMethod4>(OBSERVABLE_PUBLISH)(observable, 0, 0, 0);
}

#[cfg(all(
    sample_cache_refresh_poc,
    sample_pair_identity_poc,
    native_dual_capture_poc
))]
unsafe fn live_stereo_left_for_slot(state: *const u8, pack: u8, slot: u8) -> Option<u8> {
    if let Some(left) = identified_pair_left(state, pack, slot) {
        return Some(left);
    }

    let arm = read_u32(state, STATE_ARM_TOKEN);
    if arm & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC
        && ((arm >> 8) & 0x1f) as u8 == pack
    {
        let left = arm as u8;
        if is_stereo_pair_start(left) && (slot == left || slot == left + 1) {
            return Some(left);
        }
    }

    let token = read_u32(state, STATE_REFRESH_TOKEN);
    if token & ARM_TOKEN_MAGIC_MASK == REFRESH_TOKEN_MAGIC
        && token & REFRESH_TOKEN_STEREO != 0
        && ((token >> 8) & 0x1f) as u8 == pack
    {
        let left = token as u8;
        if is_stereo_pair_start(left) && (slot == left || slot == left + 1) {
            return Some(left);
        }
    }

    if read_u8(state, STATE_WRITER_MODE) != WRITER_MODE_CUSTOM_STEREO
        || read_u8(state, STATE_DESCRIPTOR + DESCRIPTOR_PACK) != pack
    {
        return None;
    }
    let descriptor_slot = read_u32(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT) as u8;
    let left = match read_u32(state, STATE_MAGIC) {
        WRITER_MAGIC_LEFT => descriptor_slot,
        WRITER_MAGIC_RIGHT_PENDING | WRITER_MAGIC_RIGHT => descriptor_slot.wrapping_sub(1),
        _ => return None,
    };
    if is_stereo_pair_start(left) && (slot == left || slot == left + 1) {
        Some(left)
    } else {
        None
    }
}

#[cfg(all(
    sample_cache_refresh_poc,
    sample_pair_identity_poc,
    native_dual_capture_poc
))]
unsafe fn refresh_committed_stereo_on_press(view: *mut u8) {
    const QUERY_SAMPLE_SLOT: usize = 0x0804_736d;
    const INVALIDATE_SAMPLE_SLOT: usize = 0x0804_a4a9;
    const LOAD_SAMPLE_SLOT: usize = 0x0804_60b9;

    let state = pair_state();
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if state.is_null() || store.is_null() {
        return;
    }
    let token = read_u32(state, STATE_REFRESH_TOKEN);
    if token & ARM_TOKEN_MAGIC_MASK != REFRESH_TOKEN_MAGIC
        || token & REFRESH_TOKEN_STEREO == 0
        || read_u8(state, STATE_ASYNC) != 8
        || read_u8(state, STATE_WRITER_MODE) != WRITER_MODE_IDLE
    {
        return;
    }
    let pack = ((token >> 8) & 0x1f) as u8;
    let left = token as u8;
    if pack != read_u8(store, 0)
        || !is_stereo_pair_start(left)
        || sample_slot_save_pending(left)
        || sample_slot_save_pending(left + 1)
    {
        return;
    }
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, 1 << 1);
    let right = left + 1;
    let query = function::<QuerySampleSlot>(QUERY_SAMPLE_SLOT);
    let left_request = [4u32, left as u32, pack as u32];
    let right_request = [4u32, right as u32, pack as u32];
    if query(0, left_request.as_ptr()) == 0 || query(0, right_request.as_ptr()) == 0 {
        write_u32(
            state,
            STATE_REFRESH_TOKEN,
            REFRESH_DIAGNOSTIC_QUERY_MISS | ((pack as u32) << 8) | left as u32,
        );
        return;
    }
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, 1 << 2);
    let invalidate = function::<InvalidateSampleSlot>(INVALIDATE_SAMPLE_SLOT);
    let load = function::<LoadSampleSlot>(LOAD_SAMPLE_SLOT);
    invalidate(left as u32);
    if load(left as u32) != 0 {
        write_u32(
            state,
            STATE_REFRESH_TOKEN,
            REFRESH_DIAGNOSTIC_LOAD_FAILED | ((pack as u32) << 8) | left as u32,
        );
        return;
    }
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, 1 << 3);
    invalidate(right as u32);
    if load(right as u32) != 0 {
        write_u32(
            state,
            STATE_REFRESH_TOKEN,
            REFRESH_DIAGNOSTIC_LOAD_FAILED | ((pack as u32) << 8) | left as u32,
        );
        return;
    }
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, 1 << 4);
    if !registry_set_pair(state, pack, left) {
        return;
    }
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, 1 << 5);
    publish_sample_view_occupancy(view, right);
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, 1 << 6);
    publish_sample_slot(right);
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, 1 << 7);
    write_u32(state, STATE_REFRESH_TOKEN, 0);
    #[cfg(native_deferred_refresh_probe_diagnostic)]
    deferred_refresh_probe_or(state, (1 << 8) | (1 << 15));
}

#[cfg(all(
    sample_cache_refresh_poc,
    sample_pair_identity_poc,
    native_dual_capture_poc,
    not(native_deferred_refresh_probe_diagnostic),
    not(stereo_stock_save_queue)
))]
unsafe fn service_deferred_stereo_refresh() {
    let state = pair_state();
    if state.is_null() {
        return;
    }
    let token = read_u32(state, STATE_REFRESH_TOKEN);
    if token & ARM_TOKEN_MAGIC_MASK != REFRESH_TOKEN_MAGIC
        || token & REFRESH_TOKEN_STEREO == 0
    {
        return;
    }
    let view = read_u32(state, STATE_DEFERRED_REFRESH_VIEW) as *mut u8;
    if !sample_view_extension_valid(view) {
        return;
    }
    refresh_committed_stereo_on_press(view);
    if read_u32(state, STATE_REFRESH_TOKEN) != token {
        write_u32(state, STATE_DEFERRED_REFRESH_VIEW, 0);
    }
}

/// Replaces the Main task's own `bl 0x08027624` so deferred stereo work runs
/// from the top of the Main loop: no callback frame, no held lock, and no
/// second producer on the Content task's command pipe.
///
/// Under the stock save queue the deferred refresh is unnecessary (the stock
/// queue publishes both slots itself), but the tick is still required: the
/// deferred peer Clear must be submitted from the main loop rather than from a
/// storage completion callback.
#[cfg(all(
    sample_cache_refresh_poc,
    sample_pair_identity_poc,
    native_dual_capture_poc,
    not(native_deferred_refresh_probe_diagnostic)
))]
#[no_mangle]
pub unsafe extern "C" fn stereo_main_loop_tick() {
    const STOCK_MAIN_LOOP_SERVICE: usize = 0x0802_7625;

    function::<unsafe extern "C" fn()>(STOCK_MAIN_LOOP_SERVICE)();
    #[cfg(stereo_position_poc)]
    service_pair_positions();
    // Sends are reconciled here rather than from the FX page render. The render
    // only runs while that page is drawn, so a pairing changed anywhere else
    // was never seen; the main loop sees every change the moment it happens,
    // which is how pan and volume already work. The render was only ever needed
    // to obtain a send controller, and one is latched on the first knob turn.
    #[cfg(stereo_position_poc)]
    service_latched_sends();
    #[cfg(stereo_position_poc)]
    service_pair_volume();
    #[cfg(stereo_position_poc)]
    service_sidechain_residue();
    #[cfg(stereo_position_poc)]
    service_pair_colors();
    #[cfg(stereo_position_poc)]
    service_pair_track_buttons();
    #[cfg(stereo_position_poc)]
    service_pair_selection();
    #[cfg(stereo_position_poc)]
    service_occupancy_model_cache();
    #[cfg(not(stereo_stock_save_queue))]
    service_deferred_stereo_refresh();
    #[cfg(all(sample_pair_identity_poc, native_dual_capture_poc))]
    service_pending_peer_clear();
}

/// Stock-save-queue stereo retention.
///
/// Replaces the `bl 0x08035056` inside `SaveController_RequestSlot` so that an
/// armed stereo pair enqueues *both* slots into the stock Save controller's
/// 128-bit pending bitmap at `controller + 0x1c`.
///
/// The stock controller already behaves as a multi-slot queue: `0x08035056`
/// sets a bit and, when idle, calls `0x080350ee` to start the lowest set bit;
/// `SaveController_Complete` (`0x08035012`) clears that bit, publishes the slot
/// to observers and calls `0x080350a2`, which selects and starts the next set
/// bit. Enqueuing `N` and `N + 1` therefore runs two complete, independent,
/// entirely stock save transactions back to back.
///
/// Consequently every retention-critical step - operation 5 prepare,
/// operation 7 WAV write plus content-index refresh through `0x0802e6c2`,
/// operation 8 `f_sync`, occupancy publication and observer broadcast - is
/// stock for both channels. No custom writer, publication token, registry
/// mutation or deferred refresh participates in persistence.
#[cfg(stereo_stock_save_queue)]
#[no_mangle]
pub unsafe extern "C" fn stereo_save_request(controller: *mut u8, slot: u32) -> u32 {
    const STOCK_SAVE_REQUEST: usize = 0x0803_5057;
    type SaveRequest = unsafe extern "C" fn(*mut u8, u32) -> u32;

    let result = function::<SaveRequest>(STOCK_SAVE_REQUEST)(controller, slot);
    if let Some(right) = armed_stereo_right_slot(slot) {
        function::<SaveRequest>(STOCK_SAVE_REQUEST)(controller, right as u32);
    } else {
        // No arm token, so this is an ordinary save - which is how a crop
        // persists: `Sampling_PumpCaptureToFile` re-reads the slot through its
        // (now cropped) capture window and rewrites the file from frame zero.
        // Nothing crop-specific exists in that path.
        //
        // The crop UI's window change is mirrored onto the partner by
        // `crop_window_mirror`, but only the edited slot is ever enqueued, so
        // the partner's window change stays in RAM and is lost at reboot.
        // Enqueuing the partner here runs a second, entirely stock save
        // transaction that pumps the partner through its mirrored window.
        //
        // Safe to do unconditionally for registered pairs: when no window
        // changed, the partner is simply rewritten with the same extent it
        // already had. The sentinel survives because the header stamp now falls
        // back to the registry.
        #[cfg(all(sample_pair_identity_poc, crop_mirror_window))]
        if let Some(partner) = registered_pair_partner(slot) {
            let state = pair_state();
            if !state.is_null()
                && read_u32(state, STATE_CROP_MIRROR_ARM) == CROP_MIRROR_ARM_MAGIC | slot
            {
                write_u32(state, STATE_CROP_MIRROR_ARM, 0);
                function::<SaveRequest>(STOCK_SAVE_REQUEST)(controller, partner as u32);
            }
        }
    }
    result
}

/// Returns the durable-registry partner of `slot`, in either direction.
///
/// Unlike `armed_stereo_right_slot` this consults no capture-time token, so it
/// is valid for saves that are not captures at all - notably a crop commit.
#[cfg(all(stereo_stock_save_queue, sample_pair_identity_poc))]
unsafe fn registered_pair_partner(slot: u32) -> Option<u8> {
    if slot >= 128 {
        return None;
    }
    let state = pair_state();
    if state.is_null() {
        return None;
    }
    let left = identified_pair_left(state, registry_pack(state), slot as u8)?;
    let partner = if left == slot as u8 { left + 1 } else { left };
    if partner == slot as u8 || partner >= 128 {
        None
    } else {
        Some(partner)
    }
}

/// Returns the right slot of the currently armed stereo pair when `slot` is
/// that pair's left slot in the mounted pack, otherwise `None`.
///
/// The arm token is published by the two-pad Sample Rec gesture and is the only
/// state this path consults; it is never a source of durable truth.
#[cfg(stereo_stock_save_queue)]
unsafe fn armed_stereo_right_slot(slot: u32) -> Option<u8> {
    if slot >= 128 {
        return None;
    }
    if armed_stereo_slot_side(slot as u8) != 1 {
        return None;
    }
    Some(slot as u8 + 1)
}

/// Returns 1 for the armed left slot, 2 for its right slot, and 0 otherwise.
///
/// Prefers the live gesture token, but falls back to the capture-time latch.
/// The gesture token is torn down by the next pad press, which routinely lands
/// before the right slot's Save completion runs; the latch is owned by the save
/// pipeline and survives until the terminal publication clears it.
/// Returns the effective stereo arm token: the live gesture token when valid,
/// otherwise the capture-time latch.
///
/// Deliberately does **not** consult `ACTIVE_CONTENT_STORE`. Hardware probe
/// `stereo-probe-b` proved that the store's pack byte matches at capture start
/// but no longer matches by the time the first save of a session writes its WAV
/// header, so re-validating the pack against that volatile global silently
/// disarmed the whole stereo path. The pack recorded inside the token was
/// already checked against the store when the capture began; that snapshot is
/// the trustworthy value.
#[cfg(stereo_stock_save_queue)]
unsafe fn armed_stereo_token() -> Option<u32> {
    let state = pair_state();
    if state.is_null() {
        return None;
    }
    let mut token = read_u32(state, STATE_ARM_TOKEN);
    if token & ARM_TOKEN_MAGIC_MASK != ARM_TOKEN_MAGIC {
        token = read_u32(state, STATE_SAVE_PAIR_LATCH);
    }
    if token & ARM_TOKEN_MAGIC_MASK != ARM_TOKEN_MAGIC {
        return None;
    }
    if !is_stereo_pair_start(token as u8) {
        return None;
    }
    Some(token)
}

/// Pack recorded in the effective stereo arm token.
#[cfg(stereo_stock_save_queue)]
unsafe fn armed_stereo_pack() -> Option<u8> {
    armed_stereo_token().map(|token| ((token >> 8) & 0x1f) as u8)
}

#[cfg(stereo_stock_save_queue)]
unsafe fn armed_stereo_slot_side(slot: u8) -> u8 {
    let Some((token, _)) = stereo_token_for_slot(slot) else {
        return 0;
    };
    if slot == token as u8 {
        1
    } else {
        2
    }
}

/// Resolve the stereo pair that owns `slot`.
///
/// Prefers the live arm token, then falls back to a displaced pair whose save
/// is still running. Returns the token and, when it came from the carry table,
/// that entry's index. The primary latch and a carried entry must never be
/// healed or retired interchangeably: clearing the primary on behalf of a
/// carried pair would destroy the identity of the capture currently arming.
#[cfg(stereo_stock_save_queue)]
unsafe fn stereo_token_for_slot(slot: u8) -> Option<(u32, Option<usize>)> {
    if let Some(token) = armed_stereo_token() {
        let left = token as u8;
        if slot == left || slot == left + 1 {
            return Some((token, None));
        }
    }
    let state = pair_state();
    if state.is_null() {
        return None;
    }
    let mut index = 0;
    while index < STEREO_CARRY_SLOTS {
        let token = read_u32(state, STATE_SAVE_PAIR_CARRY + index * 4);
        if token & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC && is_stereo_pair_start(token as u8)
        {
            let left = token as u8;
            if slot == left || slot == left + 1 {
                return Some((token, Some(index)));
            }
        }
        index += 1;
    }
    None
}

/// Preserve a still-in-flight pair's identity before a new capture overwrites
/// the primary latch.
///
/// The save pipeline resolves "is this the left or the right channel?" from the
/// latch at header-write time. A capture that began while the previous pair was
/// still saving used to overwrite (or, for a mono capture, zero) that latch, so
/// the older pair's right channel was no longer recognised as a right channel
/// and its `96002` byte-rate marker was never written. Because the boot rescan
/// rebuilds the registry from those markers, the pair stayed unlinked even
/// across a reboot.
///
/// Entries are normally retired by `sample_pair_save_publish`. Compacting here
/// preserves arrival order, so if the table is genuinely full the pair evicted
/// is the one that has been waiting longest.
#[cfg(native_dual_capture_poc)]
unsafe fn stereo_carry_displaced_latch(state: *mut u8, replacement: u32) {
    const MAGIC: u32 = 0xa55a_0000;
    const MASK: u32 = 0xffff_0000;

    if state.is_null() {
        return;
    }
    let existing = read_u32(state, STATE_SAVE_PAIR_LATCH);
    if existing & MASK != MAGIC || existing == replacement {
        return;
    }

    let mut index = 0;
    while index < STEREO_CARRY_SLOTS {
        if read_u32(state, STATE_SAVE_PAIR_CARRY + index * 4) == existing {
            return;
        }
        index += 1;
    }

    let mut live = 0;
    let mut index = 0;
    while index < STEREO_CARRY_SLOTS {
        let entry = read_u32(state, STATE_SAVE_PAIR_CARRY + index * 4);
        if entry & MASK == MAGIC {
            write_u32(state, STATE_SAVE_PAIR_CARRY + live * 4, entry);
            live += 1;
        }
        index += 1;
    }
    let mut index = live;
    while index < STEREO_CARRY_SLOTS {
        write_u32(state, STATE_SAVE_PAIR_CARRY + index * 4, 0);
        index += 1;
    }

    if live == STEREO_CARRY_SLOTS {
        let mut index = 1;
        while index < STEREO_CARRY_SLOTS {
            write_u32(
                state,
                STATE_SAVE_PAIR_CARRY + (index - 1) * 4,
                read_u32(state, STATE_SAVE_PAIR_CARRY + index * 4),
            );
            index += 1;
        }
        live -= 1;
    }
    write_u32(state, STATE_SAVE_PAIR_CARRY + live * 4, existing);
}

/// Replaces the sample-occupancy model's *observer* vtable slot (`0x08067588`)
/// so a stereo capture fills **both** pads at the moment recording stops.
///
/// Stock reacts to the recording-state observable reaching 3 — the instant the
/// capture is handed off and the save is requested — by setting the occupancy
/// bit for the one slot the recording controller owns. That single bit is what
/// makes a stock mono capture appear "filled" and become previewable while the
/// save is still running.
///
/// A stereo capture writes two slots natively, but only the left one is known
/// to the recording controller, so the right pad stayed dark until the save
/// completed and the registry was published. This wrapper runs stock first,
/// then mirrors the bit onto `left + 1` whenever the armed stereo token says
/// this slot is the left half of a pair.
///
/// # Multiple inheritance
///
/// The model (ctor `0x08039EBC`, vtable base `0x08067568`) has its primary
/// vtable at base+8 (`0x08067570`) but registers `this + 4` as the observer, so
/// notifications dispatch through base+0x20 (`0x08067588`) into the adjustor
/// thunk `0x08039FEC` (`subs r0, r0, #4`), which falls through into the body at
/// `0x08039FEE`. `observer` therefore arrives here as the *secondary* pointer:
/// stock is re-entered through the thunk unchanged, while our own field reads
/// subtract the same 4 bytes. Patching the primary vtable instead has no effect
/// at all — the notification path never reads it.
///
/// Deliberately keyed on the arm token alone and never on a pack value, so the
/// first capture after a cold boot behaves identically to every later one (see
/// `reports/first-recording-pack-mount-root-cause.md`). The gesture guarantees
/// `(left & 7) != 7`, so `left` and `left + 1` always share one bitmap word.
#[cfg(stereo_stock_save_queue)]
#[no_mangle]
pub unsafe extern "C" fn sample_pair_occupancy_notify(
    observer: *mut u8,
    changed: u32,
    flags: u32,
) {
    const STOCK_OCCUPANCY_THUNK: usize = 0x0803_9fed;
    const OBSERVER_THIS_ADJUST: usize = 4;
    const OCCUPANCY_OBSERVABLE: usize = 0x14;
    const RECORDING_STATE_OBSERVABLE: usize = 0x24;
    const SELECTED_SLOT_OBSERVABLE: usize = 0x28;
    const OBSERVABLE_READ: usize = 0x0803_fc59;
    const OBSERVABLE_VALUE: usize = 0x0803_fd27;
    const OBSERVABLE_PUBLISH: usize = 0x0803_fbed;

    function::<FirmwareMethod4>(STOCK_OCCUPANCY_THUNK)(observer as u32, changed, flags, 0);

    if observer.is_null() {
        return;
    }
    let model = (observer as usize - OBSERVER_THIS_ADJUST) as *mut u8;
    // Only mirror on the same edge stock acts on: the recording-state
    // observable transitioning to 3.
    let state_observable = read_u32(model, RECORDING_STATE_OBSERVABLE);
    if state_observable == 0 || changed != state_observable {
        return;
    }
    let state_value = function::<ObservableValue>(OBSERVABLE_READ)(state_observable, 0);
    if state_value.is_null() || read_volatile(state_value) != 3 {
        return;
    }
    let slot_observable = read_u32(model, SELECTED_SLOT_OBSERVABLE);
    if slot_observable == 0 {
        return;
    }
    let slot_value = function::<ObservableValue>(OBSERVABLE_READ)(slot_observable, 0);
    if slot_value.is_null() {
        return;
    }
    let left = read_volatile(slot_value);
    if armed_stereo_slot_side(left) != 1 {
        return;
    }
    let right = left + 1;
    let occupancy = read_u32(model, OCCUPANCY_OBSERVABLE);
    if occupancy == 0 {
        return;
    }
    let word =
        function::<ObservableValue>(OBSERVABLE_VALUE)(occupancy, (right >> 5) as u32).cast::<u32>();
    if word.is_null() {
        return;
    }
    let previous = read_volatile(word);
    write_volatile(word, previous | (1u32 << (right & 31)));
    function::<FirmwareMethod4>(OBSERVABLE_PUBLISH)(occupancy, previous, 1, (right >> 4) as u32);
}

/// Replaces stock `AudioCapture_ReadFrames` (capture-source vtable `+0x18`).
///
/// Stock walks a multi-page request by adding each chunk length to the *physical*
/// address it resolved once for the first frame. Logical pages are handed out by
/// the DSP page allocator and are not physically contiguous, so every request that
/// straddles a page boundary reads unrelated memory for its tail. This wrapper
/// re-resolves the physical base for each page and dispatches through the
/// capture-source `+0x14` chunk method, preserving the stock return contract of
/// "frames actually delivered".
#[cfg(stereo_stock_save_queue)]
#[no_mangle]
pub unsafe extern "C" fn native_stock_capture_source_read(
    source: *mut u8,
    slot: u16,
    cursor: u32,
    destination: *mut i16,
    requested: u32,
) -> u32 {
    // Keep the incoming C-ABI r0 value live across the helper calls below.
    // Without an addressable spill, this rustc/LLVM target combination allocated
    // it to r4 but omitted the defining move in Candidate 55.
    let source_spill = source;
    let source = read_volatile(&source_spill);
    if requested == 0 {
        return 0;
    }

    let position = source_method::<SourcePosition>(source, 0x08);
    let chunk_read = source_method::<SourceChunkRead>(source, 0x14);

    // The walk/mapping instrumentation that lived here has been retired: it
    // established that the crop start is applied beneath this interface, and
    // its fields are now reused by the extent probe below.


    let mut done: u32 = 0;
    while done < requested {
        let logical = cursor + done;
        let page_remaining = DSP_PAGE_FRAMES - (logical % DSP_PAGE_FRAMES);
        let chunk = core::cmp::min(requested - done, page_remaining);
        let physical = position(source, slot, logical);
        chunk_read(source, physical, destination.add(done as usize), chunk);
        done += chunk;
    }
    done
}

/// Replaces stock `AudioCapture_CopyContiguousFrames` (capture-source vtable
/// `+0x14`), the single-burst physical read.
///
/// The stock DSP transport is addressed with 256-word granularity. For a target
/// whose low byte is non-zero the stock burst helper discards the remainder and
/// re-synchronises the transport; when the low byte is zero it skips both steps,
/// and the first two words of that burst come back stale (observed as zero). Every
/// page-aligned chunk therefore begins with two dead samples, which is the ~11.7 Hz
/// click train heard in saved captures.
///
/// Candidate 13 proved the correction: read the burst from `physical + 2`, which
/// forces the re-synchronisation path, then reconstruct the two head samples by
/// saturating backward linear extrapolation. Applying it here - one layer below the
/// stock page walk - corrects *every* chunk rather than only the first of a request.
#[cfg(stereo_stock_save_queue)]
#[no_mangle]
pub unsafe extern "C" fn native_stock_capture_chunk_read(
    source: *mut u8,
    physical: u32,
    destination: *mut i16,
    count: u32,
) -> u64 {
    const STOCK_CHUNK_READ: usize = 0x0806_444b;

    let source_spill = source;
    let stock_chunk = function::<SourceChunkRead>(STOCK_CHUNK_READ);
    let source = read_volatile(&source_spill);

    // Stock applies this same clamp internally; mirror it so the extrapolation
    // only ever inspects words the burst actually delivered.
    let bounded = core::cmp::min(count, DSP_PAGE_FRAMES - (physical % DSP_PAGE_FRAMES));
    if physical & 0xff != 0 || bounded < 4 {
        return stock_chunk(source, physical, destination, count);
    }

    let result = stock_chunk(source, physical + 2, destination.add(2), bounded - 2);
    let sample_2 = read_volatile(destination.add(2)) as i32;
    let sample_3 = read_volatile(destination.add(3)) as i32;
    let sample_1 = (sample_2 * 2 - sample_3).clamp(i16::MIN as i32, i16::MAX as i32);
    let sample_0 = (sample_1 * 2 - sample_2).clamp(i16::MIN as i32, i16::MAX as i32);
    write_volatile(destination, sample_0 as i16);
    write_volatile(destination.add(1), sample_1 as i16);
    (result & 0xffff_ffff_0000_0000) | bounded as u64
}

/// Stamps pair identity into the stock writer's completed 44-byte header, then
/// forwards the unmodified stock async-write transaction.
#[cfg(stereo_stock_save_queue)]
#[no_mangle]
pub unsafe extern "C" fn stereo_stock_header_write(
    descriptor: *mut u8,
    offset: u32,
    buffer: *mut i16,
    length: u32,
    callback: usize,
    context: *mut u8,
) -> u32 {
    if !descriptor.is_null() && !buffer.is_null() && offset == 0 && length == 0x2c {
        let slot = read_u32(descriptor, DESCRIPTOR_SLOT) as u8;
        let resolved = stereo_token_for_slot(slot);
        let side = match resolved {
            Some((token, _)) => {
                if slot == token as u8 {
                    1
                } else {
                    2
                }
            }
            None => 0,
        };
        // An ordinary save carries no arm token, so `side` is 0 and the stock
        // byte rate would overwrite the 96001/96002 sentinel - silently
        // un-pairing the sample at the next pack-load scan. That is a
        // data-integrity bug in its own right (any rewrite of a paired slot
        // loses the pair across a reboot) and it is fatal to the crop mirror,
        // which persists precisely by issuing ordinary saves.
        //
        // Fall back to the durable registry, which is the same authority the
        // scan rebuilds from. This only ever RESTORES a marker for a slot the
        // pack already records as a pair member; it never invents one. The
        // descriptor's pack must agree with the pack the registry is keyed to,
        // so a save into a different pack cannot be stamped from this one.
        #[cfg(sample_pair_identity_poc)]
        let side = if side != 0 {
            side
        } else {
            let state = pair_state();
            let pack = if state.is_null() {
                0xff
            } else {
                registry_pack(state)
            };
            if !state.is_null() && pack < 0x20 && read_u8(descriptor, DESCRIPTOR_PACK) & 0x1f == pack
            {
                match identified_pair_left(state, pack, slot) {
                    Some(left) if left == slot => 1,
                    Some(_) => 2,
                    None => 0,
                }
            } else {
                0
            }
        };
        #[cfg(native_capture_preflight_probe_diagnostic)]
        {
            let pstate = pair_state();
            if !pstate.is_null() {
                let probe = read_u32(pstate, STATE_CAPTURE_PREFLIGHT_PROBE);
                if probe & 0xffff_0000 == CAPTURE_PREFLIGHT_PROBE_MAGIC {
                    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
                    let token = read_u32(pstate, STATE_ARM_TOKEN);
                    let latch = read_u32(pstate, STATE_SAVE_PAIR_LATCH);
                    let token_ok = token & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC;
                    let latch_ok = latch & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC;
                    // Bit 13 now reports the stable comparison the save path
                    // actually uses: descriptor pack vs. the armed token's pack.
                    let pack_ok =
                        armed_stereo_pack() == Some(read_u8(descriptor, DESCRIPTOR_PACK));
                    let stamped = side != 0;
                    let bits = (1u32 << 9)
                        | (((!store.is_null()) as u32) << 10)
                        | ((token_ok as u32) << 11)
                        | ((latch_ok as u32) << 12)
                        | ((pack_ok as u32) << 13)
                        | ((stamped as u32) << 14);
                    write_u32(pstate, STATE_CAPTURE_PREFLIGHT_PROBE, probe | bits);
                }
            }
        }
        if side != 0 {
            // The capture-time pack snapshot goes stale during the first save of
            // a session: hardware probe `stereo-probe-b` showed the live store's
            // pack byte matching at capture start but not at header-write time.
            // The descriptor names the pack this file is actually being written
            // into, so it is the authoritative value. Heal the latch from it so
            // the terminal publication keys the registry to the same pack the UI
            // will later read, and so a stale snapshot can no longer veto the
            // marker stamp.
            let descriptor_pack = read_u8(descriptor, DESCRIPTOR_PACK) & 0x1f;
            if let Some((token, carry_index)) = resolved {
                let healed = encode_arm_token(descriptor_pack, token as u8);
                let state = pair_state();
                if !state.is_null() {
                    match carry_index {
                        // Heal only the carried copy. The primary latch and arm
                        // token now describe a *different*, newer capture.
                        Some(index) => {
                            write_u32(state, STATE_SAVE_PAIR_CARRY + index * 4, healed)
                        }
                        None => {
                            write_u32(state, STATE_SAVE_PAIR_LATCH, healed);
                            if read_u32(state, STATE_ARM_TOKEN) & ARM_TOKEN_MAGIC_MASK
                                == ARM_TOKEN_MAGIC
                            {
                                write_u32(state, STATE_ARM_TOKEN, healed);
                            }
                        }
                    }
                }
            }
            write_u32(
                buffer.cast::<u8>(),
                WAV_BYTE_RATE_OFFSET,
                if side == 1 {
                    WAV_BYTE_RATE_STEREO_LEFT
                } else {
                    WAV_BYTE_RATE_STEREO_RIGHT
                },
            );
        }
    }
    function::<AsyncWrite>(ASYNC_WRITE)(
        descriptor,
        offset,
        buffer,
        length,
        callback,
        context,
    )
}

#[cfg(native_deferred_refresh_probe_diagnostic)]
unsafe fn deferred_refresh_probe_or(state: *mut u8, flags: u32) {
    write_u32(
        state,
        STATE_CAPTURE_UI_TOKEN,
        read_u32(state, STATE_CAPTURE_UI_TOKEN) | flags,
    );
}

#[cfg(native_deferred_refresh_probe_diagnostic)]
unsafe fn probe_deferred_refresh_on_press(view: *mut u8) {
    let state = pair_state();
    if state.is_null()
        || read_u32(state, STATE_CAPTURE_UI_TOKEN) != DEFERRED_REFRESH_READY_MAGIC
    {
        return;
    }
    let token = read_u32(state, STATE_REFRESH_TOKEN);
    write_u32(
        state,
        STATE_HEADER_WRITE_MARKER,
        token & (REFRESH_TOKEN_STEREO | 0x1fff),
    );
    write_u32(
        state,
        STATE_CAPTURE_UI_TOKEN,
        NATIVE_DEFERRED_REFRESH_REPORT_MAGIC | 1,
    );
    refresh_committed_stereo_on_press(view);
}

#[cfg(all(
    sample_cache_refresh_poc,
    sample_pair_identity_poc,
    native_dual_capture_poc
))]
unsafe fn publish_live_stereo_occupancy(view: *mut u8, slot: u8) {
    let state = pair_state();
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if state.is_null() || store.is_null() {
        return;
    }
    let pack = read_u8(store, 0);
    let Some(left) = live_stereo_left_for_slot(state, pack, slot) else {
        return;
    };
    let right = left + 1;
    if slot != right || sample_view_slot_occupied(view, right) {
        return;
    }
    let length = native_slot_extent(right as u16, 1);
    if length != 0 && native_slot_extent(right as u16, 3) >= length {
        publish_sample_view_occupancy(view, right);
    }
}

#[cfg(sample_pair_identity_poc)]
unsafe fn sample_view_selected_slot(view: *const u8) -> u8 {
    const SELECTED_SLOT: usize = 0x20;
    const OBSERVABLE_READ: usize = 0x0803_fc59;

    let observable = read_u32(view, SELECTED_SLOT);
    if observable == 0 {
        return 0xff;
    }

    let selected = function::<ObservableValue>(OBSERVABLE_READ)(observable, 0);
    if selected.is_null() {
        0xff
    } else {
        read_volatile(selected)
    }
}

#[cfg(sample_pair_identity_poc)]
unsafe fn sample_view_clear_active(view: *const u8) -> bool {
    const OBSERVABLE_READ: usize = 0x0803_fc59;
    const MODIFIER_A: usize = 0x24;
    const MODIFIER_B: usize = 0x2c;

    let modifier_a = read_u32(view, MODIFIER_A);
    let modifier_b = read_u32(view, MODIFIER_B);
    if modifier_a == 0 || modifier_b == 0 {
        return false;
    }
    let read = function::<ObservableValue>(OBSERVABLE_READ);
    let a = read(modifier_a, 0);
    let b = read(modifier_b, 0);
    !a.is_null() && !b.is_null() && (*a & 0x04) != 0 && (*b & 0x01) != 0
}

#[cfg(all(sample_pair_gesture_poc, sample_pair_identity_poc))]
#[no_mangle]
unsafe extern "C" fn sample_pair_clear_canonical_pad(view: *const u8, pad: u32) -> u32 {
    const PAGE: usize = 0x0c;

    // Each gate is evaluated into a flag rather than taken as an early return
    // so that a probe build can report *which* one rejected the press. All of
    // them are side-effect-free reads, so evaluating them eagerly is safe.
    let in_range = pad < 16;
    let ext_valid = in_range && sample_view_extension_valid(view);
    let clear_active = ext_valid && sample_view_clear_active(view);
    let mode_ok = clear_active && sample_view_mode_supported(view);
    let state = if mode_ok {
        pair_state()
    } else {
        core::ptr::null_mut()
    };
    let store = if mode_ok {
        read_volatile(ACTIVE_CONTENT_STORE) as *const u8
    } else {
        core::ptr::null()
    };
    let ptrs_ok = mode_ok && !state.is_null() && !store.is_null();
    let page_base = if ext_valid { read_u8(view, PAGE) << 4 } else { 0 };
    let slot = page_base.wrapping_add(pad as u8);
    let left = if ptrs_ok {
        identified_pair_left(state, registry_pack(state), slot)
    } else {
        None
    };
    let selected = if ext_valid {
        sample_view_selected_slot(view)
    } else {
        0xff
    };
    // Stock only issues a clear request when the pressed slot equals the
    // selected slot (`FUN_0804f9d8`, compared against the observable at
    // `view+0x20`). Rewriting the press onto the *left* member therefore breaks
    // the very case it was meant to fix: pressing an already-selected right
    // member turns a passing comparison into a failing one.
    //
    // Rewrite toward the **selected** member instead, so that pressing either
    // half of a pair while either half is selected satisfies the comparison.
    // Mapping the request onto the left member for the actual deletion is no
    // longer this function's job -- `sample_pair_clear_entry` does it at
    // `0x0803A074`, below the gate and on every clear path.
    let result = match left {
        Some(left)
            if (selected == left || selected == left + 1)
                && selected >= page_base
                && selected < page_base.wrapping_add(16) =>
        {
            (selected - page_base) as u32
        }
        _ => pad,
    };

    // Reported on pads 1-15 as bits 0-14 by the preflight probe renderer.
    #[cfg(native_capture_preflight_probe_diagnostic)]
    {
        let probe_state = pair_state();
        if !probe_state.is_null() && clear_active {
            let bits = (in_range as u32)
                | ((ext_valid as u32) << 1)
                | ((clear_active as u32) << 2)
                | ((mode_ok as u32) << 3)
                | ((ptrs_ok as u32) << 4)
                | ((left.is_some() as u32) << 5)
                | (((result != pad) as u32) << 6)
                | (((selected == slot) as u32) << 7)
                | (((selected as u32) & 0x7f) << 8);
            write_u32(
                probe_state,
                STATE_CAPTURE_PREFLIGHT_PROBE,
                CAPTURE_PREFLIGHT_PROBE_MAGIC | CAPTURE_PREFLIGHT_PROBE_ENTRY | bits,
            );
        }
    }
    let _ = selected;
    result
}

/// Canonicalises a Sample Clear request onto the left member of a stereo pair.
///
/// This runs at `0x0803A074`, inside the stock `FUN_0803a04a` epilogue, which
/// is the *only* caller of the clear entry `0x0803A10E` and sits ahead of its
/// busy and occupancy checks. Canonicalising the pad at the UI layer is not
/// enough: a press on the right member that reaches `0x0803A10E` unrewritten is
/// silently dropped by that occupancy check, so the hook has to land before it.
///
/// The deferred peer replay calls `0x0803A10E` directly and therefore bypasses
/// this shim; it must, or the peer would be rewritten back onto the member that
/// was just deleted.
#[cfg(sample_pair_identity_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_clear_canonical_slot(slot: u32) -> u32 {
    if slot >= 128 {
        return slot;
    }
    let state = pair_state();
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if state.is_null() || store.is_null() {
        return slot;
    }
    identified_pair_left(state, registry_pack(state), slot as u8)
        .map_or(slot, |left| left as u32)
}

#[cfg(sample_pair_identity_poc)]
core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb

    .section .text.sample_pair_clear_entry,"ax",%progbits
    .global sample_pair_clear_entry
    .type sample_pair_clear_entry,%function
    .thumb_func
sample_pair_clear_entry:
    pop.w {{r2, r4, r5, lr}}
    push {{r0, r2, r3, lr}}
    mov r0, r1
    bl sample_pair_clear_canonical_slot
    mov r1, r0
    pop {{r0, r2, r3, lr}}
    ldr ip, .Lsample_pair_clear_entry_stock
    bx ip
    .align 2
.Lsample_pair_clear_entry_stock:
    .word 0x0803a10f
    .size sample_pair_clear_entry, .-sample_pair_clear_entry
"#
);

#[cfg(all(native_dual_capture_poc, crop_io_trace))]
type ControlRouter = unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u32;

#[cfg(all(native_dual_capture_poc, any(crop_io_trace, crop_mirror_window)))]
type LoopWindowSetter = unsafe extern "C" fn(*mut u8, u32, u32, u32) -> u32;

#[cfg(all(native_dual_capture_poc, crop_io_trace))]
unsafe fn crop_window_record(slot: u32, start: u32, end: u32, waiting: bool) {
    let state = pair_state();
    if state.is_null() {
        return;
    }
    write_u32(state, CROP_WIN_SLOT, slot);
    write_u32(state, CROP_WIN_START, start);
    write_u32(state, CROP_WIN_END, end);
    let counter = if waiting {
        CROP_WIN_CALLS_WAIT
    } else {
        CROP_WIN_CALLS
    };
    write_u32(state, counter, read_u32(state, counter).wrapping_add(1));
}

/// Applies the crop window a slot just received to that slot's stereo partner.
///
/// The crop UI pushes start and end straight at the capture hardware once per
/// knob increment, so the partner's window is simply the same pair of values
/// against the partner's slot id. The values are copied verbatim: they are a
/// hardware-side unit, not frames, and nothing here needs to know which.
///
/// This writes hardware window registers only. No sample file is touched, so a
/// wrong partner would misplace a later rewrite but cannot by itself corrupt
/// audio.
#[cfg(all(
    native_dual_capture_poc,
    crop_mirror_window,
    sample_pair_identity_poc
))]
unsafe fn crop_window_mirror(object: *mut u8, slot: u32, start: u32, end: u32, waiting: bool) {
    const STOCK_SET_LOOP_WINDOW: usize = 0x0806_44e5;
    const STOCK_SET_LOOP_WINDOW_WAIT: usize = 0x0806_4513;

    let state = pair_state();
    if state.is_null() || object.is_null() || slot >= 128 {
        return;
    }
    // Count every attempt, and record the outcome in the same field, so the
    // partner slot below can never be read as data when no mirror happened.
    #[cfg(crop_io_trace)]
    let attempts = read_u32(state, CROP_WIN_ATTEMPTS) & 0xff;
    #[cfg(crop_io_trace)]
    {
        let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
        let reason: u32 = if store.is_null() { 1 } else { 2 };
        write_u32(
            state,
            CROP_WIN_ATTEMPTS,
            attempts.wrapping_add(1) & 0xff | (reason << 8),
        );
    }

    // Registry health is now read live by the LED render, which is valid in
    // sessions without a crop; the stored copy here was dead weight.
    let partner = match crop_mirror_partner(state, slot as u8) {
        Some(partner) => partner,
        None => return,
    };
    #[cfg(crop_io_trace)]
    write_u32(
        state,
        CROP_WIN_ATTEMPTS,
        attempts.wrapping_add(1) & 0xff | (3 << 8),
    );

    // Calls the stock setters directly rather than the hooked vtable slots, so
    // the mirror cannot re-enter itself.
    let setter = if waiting {
        STOCK_SET_LOOP_WINDOW_WAIT
    } else {
        STOCK_SET_LOOP_WINDOW
    };
    function::<LoopWindowSetter>(setter)(object, partner as u32, start, end);

    // Arm the partner rewrite. The save request consumes this, so the partner's
    // file can only be rewritten when its window was genuinely changed here.
    // Without it, every ordinary save of a paired slot would rewrite the
    // partner through whatever window it happened to hold - which is how a
    // mirror that touches no files becomes one that can destroy audio.
    write_u32(
        state,
        STATE_CROP_MIRROR_ARM,
        CROP_MIRROR_ARM_MAGIC | slot,
    );

    #[cfg(crop_io_trace)]
    {
        write_u32(state, CROP_WIN_PARTNER, partner as u32);
        write_u32(
            state,
            CROP_WIN_MIRRORS,
            read_u32(state, CROP_WIN_MIRRORS).wrapping_add(1),
        );
    }
}

#[cfg(all(native_dual_capture_poc, any(crop_io_trace, crop_mirror_window)))]
#[allow(unused_variables)]
unsafe fn crop_window_mirror_dispatch(
    object: *mut u8,
    slot: u32,
    start: u32,
    end: u32,
    waiting: bool,
) {
    #[cfg(all(crop_mirror_window, sample_pair_identity_poc))]
    crop_window_mirror(object, slot, start, end, waiting);
}

#[cfg(all(native_dual_capture_poc, any(crop_io_trace, crop_mirror_window)))]
#[no_mangle]
pub unsafe extern "C" fn crop_window_set(
    object: *mut u8,
    slot: u32,
    start: u32,
    end: u32,
) -> u32 {
    const STOCK_SET_LOOP_WINDOW: usize = 0x0806_44e5;
    #[cfg(crop_io_trace)]
    crop_window_record(slot, start, end, false);
    crop_window_mirror_dispatch(object, slot, start, end, false);
    function::<LoopWindowSetter>(STOCK_SET_LOOP_WINDOW)(object, slot, start, end)
}

#[cfg(all(native_dual_capture_poc, any(crop_io_trace, crop_mirror_window)))]
#[no_mangle]
pub unsafe extern "C" fn crop_window_set_wait(
    object: *mut u8,
    slot: u32,
    start: u32,
    end: u32,
) -> u32 {
    const STOCK_SET_LOOP_WINDOW_WAIT: usize = 0x0806_4513;
    #[cfg(crop_io_trace)]
    crop_window_record(slot, start, end, true);
    crop_window_mirror_dispatch(object, slot, start, end, true);
    function::<LoopWindowSetter>(STOCK_SET_LOOP_WINDOW_WAIT)(object, slot, start, end)
}

#[cfg(all(native_dual_capture_poc, crop_io_trace))]
#[no_mangle]
unsafe extern "C" fn crop_io_trace_record(
    op: u32,
    desc: *const u8,
    params: *const u8,
    caller: u32,
) {
    const SAMPLE_FILE_COMMAND: u32 = 4;
    const OP_READ: u32 = 6;
    const OP_WRITE: u32 = 7;
    const WAV_DATA_START: u32 = 0x2c;
    const WAV_HEADER_SIZE: u32 = 0x2c;
    const RIFF_MAGIC: u32 = 0x4646_4952;
    const WAVE_MAGIC: u32 = 0x4556_4157;
    // Deliberately loose for this read-only round so the real gap can be
    // observed; it will be tightened to the measured value before any write.
    const CROP_SOURCE_READ_WINDOW: u32 = 16;

    if desc.is_null() || params.is_null() {
        return;
    }
    if read_u32(desc, 0) != SAMPLE_FILE_COMMAND {
        return;
    }

    let state = pair_state();
    if state.is_null() {
        return;
    }

    let slot = read_u32(desc, 4);
    let offset = read_u32(params, 0);
    let buffer = read_u32(params, 4) as *const u8;
    let len = read_u32(params, 8);
    let op = op & 0xff;

    let sequence = read_u32(state, CROP_IO_OP_COUNT).wrapping_add(1);
    write_u32(state, CROP_IO_OP_COUNT, sequence);

    // Counted unconditionally so the audio-source hook can tell a rewrite from
    // playback: only a rewrite interleaves sample-file writes with source reads.
    if op == OP_WRITE {
        write_u32(
            state,
            CROP_SRC_WRITES,
            read_u32(state, CROP_SRC_WRITES).wrapping_add(1),
        );
    }

    if op == OP_READ {
        // Rolling, so that whichever read immediately precedes the first write
        // can be frozen as the crop's source offset. Header reads at offset 0
        // are noise from every upload, so they are not allowed to displace a
        // genuine audio-region read.
        if offset >= WAV_DATA_START {
            write_u32(state, CROP_IO_LAST_READ_OFF, offset);
            write_u32(state, CROP_IO_LAST_READ_LEN, len);
            write_u32(state, CROP_IO_LAST_READ_SLOT, slot);
            write_u32(state, CROP_IO_LAST_READ_OP, sequence);
        }
        let counts = read_u32(state, CROP_IO_COUNTS);
        write_u32(state, CROP_IO_COUNTS, counts.wrapping_add(1) & 0xffff | counts & 0xffff_0000);
        return;
    }

    if op != OP_WRITE {
        return;
    }

    let counts = read_u32(state, CROP_IO_COUNTS);
    let writes = counts >> 16;
    write_u32(
        state,
        CROP_IO_COUNTS,
        (counts & 0xffff) | (writes.wrapping_add(1) << 16),
    );
    let mut diag = read_u32(state, CROP_IO_DIAG);

    if offset == WAV_DATA_START {
        // Every crop restarts here, so the candidate is re-evaluated on each
        // such write rather than once per boot: a single unrelated sample-file
        // write ahead of the crop must not be able to disarm detection.
        write_u32(state, CROP_IO_TOTAL_WRITE, 0);
        write_u32(state, CROP_IO_CALLER, caller);
        diag = diag.wrapping_add(1) & 0x1f | diag & !0x1f;
        let source = read_u32(state, CROP_IO_LAST_READ_OFF);
        // Measured rather than assumed: the firmware reads its own header
        // between the source read and the shift, so the two are not adjacent.
        let gap = sequence.wrapping_sub(read_u32(state, CROP_IO_LAST_READ_OP));
        write_u32(state, CROP_IO_GAP, if gap > 255 { 255 } else { gap });
        // A crop rewrites a file it is also reading, so this write is preceded
        // by a read of the same slot at or beyond the data start. An end-trim
        // crop reads from `0x2C` itself, so equality has to be allowed.
        if read_u32(state, CROP_IO_LAST_READ_SLOT) == slot
            && source >= WAV_DATA_START
            && gap <= CROP_SOURCE_READ_WINDOW
        {
            write_u32(state, CROP_IO_SRC_OFF, source - WAV_DATA_START);
            write_u32(state, CROP_IO_SNAP_READ_LEN, read_u32(state, CROP_IO_LAST_READ_LEN));
            write_u32(state, CROP_IO_ACTIVE, 1);
            write_u32(state, CROP_IO_SLOT, slot);
            write_u32(state, CROP_IO_NEXT_WRITE, WAV_DATA_START);
            diag = (diag & !0x3e0) | (((diag >> 5) + 1) & 0x1f) << 5;
        } else {
            write_u32(state, CROP_IO_ACTIVE, 0);
        }
    }

    let candidate =
        read_u32(state, CROP_IO_ACTIVE) != 0 && read_u32(state, CROP_IO_SLOT) == slot;

    if offset >= WAV_DATA_START {
        // The shift is one contiguous run down from `0x2C`. Holding the writes
        // to that run is what makes the accumulated length a real extent rather
        // than a sum of unrelated writes.
        if candidate {
            if read_u32(state, CROP_IO_NEXT_WRITE) == offset {
                write_u32(state, CROP_IO_NEXT_WRITE, offset + len);
                let total = read_u32(state, CROP_IO_TOTAL_WRITE);
                write_u32(state, CROP_IO_TOTAL_WRITE, total.wrapping_add(len));
            } else {
                write_u32(state, CROP_IO_ACTIVE, 0);
            }
        }
        write_u32(state, CROP_IO_DIAG, diag);
        return;
    }

    // The 44-byte header goes last and is what makes the new length durable, so
    // it is the crop's completion edge and the only safe point to mirror from.
    if offset != 0 || len != WAV_HEADER_SIZE {
        write_u32(state, CROP_IO_DIAG, diag);
        return;
    }
    diag = (diag & 0x3ff) | (((diag >> 10) + 1) & 0xf) << 10;
    let kept = read_u32(state, CROP_IO_TOTAL_WRITE);
    // The buffer being written *is* the new header, so the firmware states the
    // resulting length itself. Requiring it to be a well-formed RIFF/WAVE whose
    // `data` size equals the bytes just written replaces every inference about
    // what the operation was with the operation's own declaration.
    let declared = if buffer.is_null() {
        0
    } else {
        if read_u32(buffer, 0) == RIFF_MAGIC && read_u32(buffer, 8) == WAVE_MAGIC {
            diag |= 1 << 14;
        }
        read_u32(buffer, 0x28)
    };
    if declared == kept && kept != 0 {
        diag |= 1 << 15;
    }
    write_u32(state, CROP_IO_DIAG, diag);
}

/// Resolves the other member of `slot`'s identified stereo pair.
///
/// Reuses the same registry the pair-wide Clear canonicalisation consults, so a
/// crop can only ever mirror onto a slot the pack genuinely pairs with the one
/// that was cropped - adjacency alone is never enough.
#[cfg(all(
    native_dual_capture_poc,
    crop_mirror_window,
    sample_pair_identity_poc
))]
unsafe fn crop_mirror_partner(state: *const u8, slot: u8) -> Option<u8> {
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if store.is_null() {
        return None;
    }
    let left = identified_pair_left(state, registry_pack(state), slot)?;
    let partner = if left == slot { left + 1 } else { left };
    if partner == slot || partner >= 128 {
        None
    } else {
        Some(partner)
    }
}

// Replaces the stock `push.w {r4-r11, lr}` prologue of `FUN_08045EB6`, replays
// it, and continues at `0x08045EBA`. Registers r0-r3 carry the op code, the
// descriptor and the offset/length triple, so they are preserved across the
// recorder call and the stock frame is built exactly as before.
//
// The recorder's fourth argument is the *grandparent* return address, not `lr`.
// `lr` is useless here: the compiler tail-shares one `bl` between the op-6 and
// op-7 wrappers, so every call returns to `0x08046075` regardless of caller.
// Both wrappers build the same frame (`push {r2,r3,r4,r6,lr}` + `sub sp,#0x14`),
// placing their own return address 0x24 above the stack pointer at the `bl`,
// hence 0x34 after this shim's four-register push.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
core::arch::global_asm!(
    r#"
    .section .text.crop_io_trace_entry,"ax",%progbits
    .global crop_io_trace_entry
    .type crop_io_trace_entry,%function
    .thumb_func
crop_io_trace_entry:
    push {{r0, r1, r2, r3}}
    ldr r3, [sp, #0x34]
    push {{lr}}
    bl crop_io_trace_record
    pop {{lr}}
    pop {{r0, r1, r2, r3}}
    push.w {{r4, r5, r6, r7, r8, r9, r10, r11, lr}}
    ldr ip, .Lcrop_io_trace_stock
    bx ip
    .align 2
.Lcrop_io_trace_stock:
    .word 0x08045ebb
    .size crop_io_trace_entry, .-crop_io_trace_entry
"#
);

#[cfg(sample_pair_writer_coupling_poc)]
unsafe fn sample_slot_save_pending(slot: u8) -> bool {
    const PENDING_BITMAP: usize = 0x1c;
    const OBSERVABLE_READ: usize = 0x0803_fc59;

    let controller = read_volatile(SAVE_CALLBACK_REGISTRATION.add(1)) as *const u8;
    if controller.is_null() {
        return false;
    }
    let pending = read_u32(controller, PENDING_BITMAP);
    if pending == 0 {
        return false;
    }
    let word = function::<ObservableValue>(OBSERVABLE_READ)(pending, (slot >> 5) as u32);
    !word.is_null() && read_volatile(word.cast::<u32>()) & (1u32 << (slot & 31)) != 0
}

#[cfg(sample_pair_writer_coupling_poc)]
unsafe fn saving_stereo_left(state: *const u8, encoded_pack: u8, slot: u8) -> bool {
    if state.is_null() || !is_stereo_pair_start(slot) {
        return false;
    }
    if let Some(pair) = pair_index(slot) {
        if registry_contains(state, encoded_pack.wrapping_sub(1), pair) {
            return true;
        }
    }
    let active_pack = read_u8(state, STATE_DESCRIPTOR + DESCRIPTOR_PACK).wrapping_add(1);
    active_pack == encoded_pack
        && match read_u32(state, STATE_MAGIC) {
            WRITER_MAGIC_LEFT => {
                read_u32(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT) as u8 == slot
            }
            WRITER_MAGIC_RIGHT_PENDING | WRITER_MAGIC_RIGHT => {
                read_u32(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT) as u8 == slot + 1
            }
            _ => false,
        }
}

#[cfg(sample_pair_gesture_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_gesture_press_before_stock(
    view: *mut u8,
    pad: u32,
) -> u32 {
    const PAGE: usize = 0x0c;

    if pad >= 16 || !sample_view_extension_valid(view) {
        return 0;
    }
    #[cfg(native_slot_metadata_probe_diagnostic)]
    {
        let right = read_u8(view, PAGE) * 16 + pad as u8;
        if right > 0 && right & 7 != 0 {
            let state = pair_state();
            if !state.is_null() {
                let left_length = native_slot_extent((right - 1) as u16, 1);
                let right_length = native_slot_extent(right as u16, 1);
                let right_produced = native_slot_extent(right as u16, 2);
                let right_readable = native_slot_extent(right as u16, 3);
                let flags = ((left_length != 0) as u32)
                    | (((right_length != 0) as u32) << 1)
                    | (((right_length == left_length && right_length != 0) as u32) << 2)
                    | (((right_produced == right_length && right_length != 0) as u32) << 3)
                    | (((right_readable >= right_length && right_length != 0) as u32) << 4);
                write_u32(
                    state,
                    STATE_WRITER_REQUEST_SOURCE,
                    NATIVE_METADATA_PROBE_MAGIC
                        | (flags << 16)
                        | ((read_u8(view, PAGE) as u32) << 8)
                        | right as u32,
                );
            }
        }
    }
    if !sample_view_plain_press(view) {
        return 0;
    }

    let pad = pad as u8;
    #[cfg(all(
        sample_cache_refresh_poc,
        sample_pair_identity_poc,
        native_dual_capture_poc
    ))]
    {
        #[cfg(native_deferred_refresh_probe_diagnostic)]
        probe_deferred_refresh_on_press(view);
        #[cfg(not(native_deferred_refresh_probe_diagnostic))]
        {
            let state = pair_state();
            if !state.is_null()
                && read_u32(state, STATE_DEFERRED_REFRESH_VIEW) == view as u32
            {
                refresh_committed_stereo_on_press(view);
                if read_u32(state, STATE_REFRESH_TOKEN) == 0 {
                    write_u32(state, STATE_DEFERRED_REFRESH_VIEW, 0);
                }
            }
        }
    }
    let held = read_u16(view, SAMPLE_VIEW_HELD_MASK);
    if held == 0 {
        sample_pair_clear_arm_token();
        write_u16(view, SAMPLE_VIEW_HELD_MASK, 1u16 << pad);
        write_u8(view, SAMPLE_VIEW_ANCHOR_PAD, pad);
        write_u8(view, SAMPLE_VIEW_ARMED_LEFT, SAMPLE_VIEW_NO_ARM);
        return 0;
    }

    write_u16(view, SAMPLE_VIEW_HELD_MASK, held | (1u16 << pad));
    let anchor = read_u8(view, SAMPLE_VIEW_ANCHOR_PAD);
    if anchor >= 16 || held != (1u16 << anchor) || anchor == pad {
        write_u8(view, SAMPLE_VIEW_ARMED_LEFT, SAMPLE_VIEW_NO_ARM);
        return 0;
    }

    let page_base = read_u8(view, PAGE) << 4;
    let first = page_base.wrapping_add(anchor);
    let second = page_base.wrapping_add(pad);
    let left = if first < second { first } else { second };
    let right = if first < second { second } else { first };
    if right != left + 1 || (left & 7) == 7 {
        write_u8(view, SAMPLE_VIEW_ARMED_LEFT, SAMPLE_VIEW_NO_ARM);
        return 0;
    }
    #[cfg(sample_pair_writer_coupling_poc)]
    if sample_view_slot_occupied(view, left) || sample_view_slot_occupied(view, right) {
        write_u8(view, SAMPLE_VIEW_ARMED_LEFT, SAMPLE_VIEW_NO_ARM);
        sample_pair_clear_arm_token();
        return 0;
    }

    write_u8(view, SAMPLE_VIEW_ARMED_LEFT, left);
    sample_pair_publish_arm_token(left);
    #[cfg(native_dual_capture_poc)]
    {
        let state = pair_state();
        if !state.is_null() {
            write_u32(state, STATE_DEFERRED_REFRESH_VIEW, view as u32);
        }
    }
    left as u32 + 1
}

#[cfg(sample_pair_gesture_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_gesture_select_canonical(
    view: *mut u8,
    left: u32,
) {
    const SELECTED_SLOT_OBSERVABLE: usize = 0x20;
    const OBSERVABLE_VALUE: usize = 0x0803_fd27;
    const OBSERVABLE_PUBLISH: usize = 0x0803_fbed;
    const REFRESH_SAMPLE_VIEW: usize = 0x0804_fcdd;

    if !sample_view_extension_valid(view) || left >= 128 {
        return;
    }
    let observable = read_u32(view, SELECTED_SLOT_OBSERVABLE);
    if observable == 0 {
        return;
    }
    let value = function::<ObservableValue>(OBSERVABLE_VALUE)(observable, 0);
    if value.is_null() {
        return;
    }
    write_volatile(value, left as u8);
    function::<FirmwareMethod4>(OBSERVABLE_PUBLISH)(observable, 0, 0, 0);
    function::<FirmwareMethod4>(REFRESH_SAMPLE_VIEW)(view as u32, 0, 0, 0);
}

#[cfg(sample_pair_gesture_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_save_preview_canonical(
    view: *mut u8,
    pad: u32,
) -> u32 {
    #[cfg(all(
        sample_pair_writer_coupling_poc,
        sample_cache_refresh_poc,
        sample_pair_identity_poc,
        native_dual_capture_poc
    ))]
    {
        const PAGE: usize = 0x0c;

        if pad < 16 && sample_view_extension_valid(view) {
            let right = read_u8(view, PAGE) * 16 + pad as u8;
            if right > 0 {
                let left = right - 1;
                let state = pair_state();
                let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
                if !state.is_null()
                    && !store.is_null()
                    && is_stereo_pair_start(left)
                    && (sample_slot_save_pending(left) || sample_slot_save_pending(right))
                    && live_stereo_left_for_slot(state, read_u8(store, 0), right)
                        == Some(left)
                {
                    return left as u32 + 1;
                }
            }
        }
    }
    0
}

#[cfg(sample_pair_gesture_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_gesture_release_after_stock(
    view: *mut u8,
    pad: u32,
) {
    if pad >= 16 || !sample_view_extension_valid(view) {
        return;
    }
    let held = read_u16(view, SAMPLE_VIEW_HELD_MASK) & !(1u16 << pad);
    write_u16(view, SAMPLE_VIEW_HELD_MASK, held);
    if held == 0 {
        write_u8(view, SAMPLE_VIEW_ANCHOR_PAD, SAMPLE_VIEW_NO_PAD);
    }
}

#[cfg(sample_pair_gesture_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_gesture_clear(view: *mut u8) {
    if !sample_view_extension_valid(view) {
        return;
    }
    write_u16(view, SAMPLE_VIEW_HELD_MASK, 0);
    write_u8(view, SAMPLE_VIEW_ANCHOR_PAD, SAMPLE_VIEW_NO_PAD);
    write_u8(view, SAMPLE_VIEW_ARMED_LEFT, SAMPLE_VIEW_NO_ARM);
    sample_pair_clear_arm_token();
}

#[inline(always)]
unsafe fn function<T>(address: usize) -> T {
    transmute_copy_usize(address)
}

#[inline(always)]
unsafe fn transmute_copy_usize<T>(address: usize) -> T {
    debug_assert!(core::mem::size_of::<T>() == core::mem::size_of::<usize>());
    core::ptr::read((&address as *const usize).cast::<T>())
}

#[inline(always)]
unsafe fn source_from_manager(manager: *mut u8) -> *mut u8 {
    read_u32(manager, 0x08) as *mut u8
}

#[inline(always)]
unsafe fn source_method<T>(source: *mut u8, offset: usize) -> T {
    let vtable = read_u32(source, 0) as *const u8;
    function(read_u32(vtable, offset) as usize)
}

#[inline(always)]
unsafe fn writer_source(state: *mut u8) -> *mut u8 {
    source_from_manager(read_u32(state, STATE_OWNER) as *mut u8)
}

unsafe fn initialize_extension(state: *mut u8, stereo: bool) {
    #[cfg(single_pass_left_diagnostic)]
    let magic = WRITER_MAGIC_LEFT;
    #[cfg(single_pass_right_diagnostic)]
    let magic = WRITER_MAGIC_RIGHT;
    #[cfg(not(any(
        single_pass_left_diagnostic,
        single_pass_right_diagnostic
    )))]
    let magic = if stereo {
        WRITER_MAGIC_LEFT
    } else {
        WRITER_MAGIC_MONO
    };
    write_u32(
        state,
        STATE_MAGIC,
        magic,
    );
    #[cfg(synthetic_commit_probe_diagnostic)]
    {
        let mut byte = 0;
        while byte < (SYNTHETIC_CHUNK_SAMPLES as usize * 2) {
            write_u8(state, STATE_SYNTHETIC_BUFFER + byte, 0);
            byte += 1;
        }
    }
}

unsafe fn reset_pass_counters(state: *mut u8) {
    write_u32(state, STATE_TOTAL_FRAMES, 0);
    write_u32(state, STATE_WRITTEN_FRAMES, 0);
    write_u8(state, STATE_BUFFER_OFFSET, 0);
    write_u32(state, STATE_BUFFER, 0);
    write_u32(state, STATE_BUFFERED_FRAMES, 0);
}

unsafe fn writer_magic_is_valid(state: *const u8) -> bool {
    matches!(
        read_u32(state, STATE_MAGIC),
        WRITER_MAGIC_MONO
            | WRITER_MAGIC_LEFT
            | WRITER_MAGIC_RIGHT_PENDING
            | WRITER_MAGIC_RIGHT
    )
}

fn is_stereo_pair_start(slot: u8) -> bool {
    #[cfg(sample_pair_writer_coupling_poc)]
    {
        slot < 127 && slot & 7 != 7
    }
    #[cfg(not(sample_pair_writer_coupling_poc))]
    {
        slot >= FIRST_RESERVED_STEREO_SLOT
            && slot < LAST_RESERVED_STEREO_SLOT
            && slot & 1 == 0
    }
}

fn is_stereo_pair_slot(slot: u8) -> bool {
    #[cfg(sample_pair_writer_coupling_poc)]
    {
        slot < 128
    }
    #[cfg(not(sample_pair_writer_coupling_poc))]
    {
        slot >= FIRST_RESERVED_STEREO_SLOT && slot <= LAST_RESERVED_STEREO_SLOT
    }
}

fn stereo_left_for_right(slot: u8) -> Option<u8> {
    if slot > 0 && is_stereo_pair_start(slot - 1) {
        Some(slot - 1)
    } else {
        None
    }
}

#[cfg(sample_pair_identity_poc)]
fn pair_index(slot: u8) -> Option<usize> {
    #[cfg(sample_pair_writer_coupling_poc)]
    {
        if is_stereo_pair_start(slot) {
            Some((slot as usize / 8) * 7 + (slot as usize & 7))
        } else {
            None
        }
    }
    #[cfg(not(sample_pair_writer_coupling_poc))]
    {
        if is_stereo_pair_slot(slot) {
            Some(((slot - FIRST_RESERVED_STEREO_SLOT) / 2) as usize)
        } else {
            None
        }
    }
}

#[cfg(any(sample_pair_identity_poc, sample_pair_writer_coupling_poc))]
unsafe fn pair_state() -> *mut u8 {
    let manager = read_volatile(SAMPLING_MANAGER) as *const u8;
    if manager.is_null() {
        return core::ptr::null_mut();
    }
    read_u32(manager, 0x04) as *mut u8
}

// The pack to key registry lookups on. Byte 0 of the active content store is
// the read side of a double-buffered observable that the stock loader only
// publishes after a pack load returns, so right after a boot it still names the
// previously loaded pack. The boot scan binds the pack it actually loaded, so
// prefer that and fall back to the store only while the registry is unpopulated.
#[cfg(sample_pair_identity_poc)]
unsafe fn registry_pack(state: *const u8) -> u8 {
    let bound = read_u8(state, STATE_PAIR_ACTIVE_PACK);
    if read_u8(state, STATE_PAIR_VALID) != 0 && bound < 0x20 {
        return bound;
    }
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if store.is_null() {
        0xff
    } else {
        read_u8(store, 0)
    }
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_bank(state: *const u8, bank: u8) -> *const u8 {
    state.add(if bank & 1 == 0 {
        STATE_PAIR_BANK_0
    } else {
        STATE_PAIR_BANK_1
    })
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_bank_mut(state: *mut u8, bank: u8) -> *mut u8 {
    state.add(if bank & 1 == 0 {
        STATE_PAIR_BANK_0
    } else {
        STATE_PAIR_BANK_1
    })
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_bit_is_set(bank: *const u8, pair: usize) -> bool {
    read_u8(bank, pair / 8) & (1 << (pair & 7)) != 0
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_set_bit(bank: *mut u8, pair: usize) {
    let offset = pair / 8;
    write_u8(bank, offset, read_u8(bank, offset) | (1 << (pair & 7)));
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_clear_bit(bank: *mut u8, pair: usize) {
    let offset = pair / 8;
    write_u8(bank, offset, read_u8(bank, offset) & !(1 << (pair & 7)));
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_contains(state: *const u8, pack: u8, pair: usize) -> bool {
    // No fallback to byte 0 of the content store: that byte is a lagging
    // double-buffered observable, so matching against it produced both false
    // positives and false negatives. The scan now always binds the pack it was
    // asked to load, so an unbound pack here genuinely means "unknown" and must
    // fail closed.
    let pack_matches = read_u8(state, STATE_PAIR_ACTIVE_PACK) == pack;
    pair < PAIR_COUNT
        && read_u8(state, STATE_PAIR_VALID) != 0
        && pack_matches
        && registry_bit_is_set(
            registry_bank(state, read_u8(state, STATE_PAIR_ACTIVE_BANK)),
            pair,
        )
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_bind_active_pack(state: *mut u8, pack: u8) -> bool {
    if pack >= 0x20 || read_u8(state, STATE_PAIR_VALID) == 0 {
        return false;
    }
    // The pack the scan bound is the only authority here. The previous
    // agreement check against byte 0 of the active content store compared
    // against a lagging published observable, so it rejected correct packs
    // right after a boot and accepted stale ones. The scan now always binds,
    // so a disagreement is a genuine mismatch and must fail closed.
    read_u8(state, STATE_PAIR_ACTIVE_PACK) == pack
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_begin_delta(state: *mut u8, pack: u8) -> Option<(*mut u8, u8)> {
    if read_u8(state, STATE_SCAN_MODE) != SCAN_MODE_IDLE {
        write_u32(
            state,
            STATE_MUTATION_EPOCH,
            read_u32(state, STATE_MUTATION_EPOCH).wrapping_add(1),
        );
        return None;
    }
    if !registry_bind_active_pack(state, pack) {
        return None;
    }
    let active = read_u8(state, STATE_PAIR_ACTIVE_BANK) & 1;
    let inactive = active ^ 1;
    let source = registry_bank(state, active);
    let destination = registry_bank_mut(state, inactive);
    let mut offset = 0;
    while offset < PAIR_BANK_SIZE {
        write_u8(destination, offset, read_u8(source, offset));
        offset += 1;
    }
    Some((destination, inactive))
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_finish_delta(state: *mut u8, bank: u8) {
    write_u32(
        state,
        STATE_MUTATION_EPOCH,
        read_u32(state, STATE_MUTATION_EPOCH).wrapping_add(1),
    );
    write_u32(
        state,
        STATE_REGISTRY_GENERATION,
        read_u32(state, STATE_REGISTRY_GENERATION).wrapping_add(1),
    );
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
    write_u8(state, STATE_PAIR_ACTIVE_BANK, bank);
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_set_pair(state: *mut u8, pack: u8, slot: u8) -> bool {
    let Some(pair) = pair_index(slot) else {
        return false;
    };
    let Some((bank, inactive)) = registry_begin_delta(state, pack) else {
        return false;
    };
    registry_set_bit(bank, pair);
    registry_finish_delta(state, inactive);
    true
}

#[cfg(sample_pair_identity_poc)]
unsafe fn registry_clear_slot(state: *mut u8, pack: u8, slot: u8) {
    let Some((bank, inactive)) = registry_begin_delta(state, pack) else {
        return;
    };
    let mut changed = false;
    if slot & 7 != 0 {
        if let Some(pair) = pair_index(slot - 1) {
            if registry_bit_is_set(bank, pair) {
                registry_clear_bit(bank, pair);
                changed = true;
            }
        }
    }
    if slot & 7 != 7 {
        if let Some(pair) = pair_index(slot) {
            if registry_bit_is_set(bank, pair) {
                registry_clear_bit(bank, pair);
                changed = true;
            }
        }
    }
    if changed {
        registry_finish_delta(state, inactive);
    }
}

#[cfg(sample_pair_writer_coupling_poc)]
#[inline(always)]
fn encode_arm_token(pack: u8, left: u8) -> u32 {
    ARM_TOKEN_MAGIC | ((pack as u32) << 8) | left as u32
}

#[cfg(sample_pair_writer_coupling_poc)]
unsafe fn sample_pair_publish_arm_token(left: u8) {
    let state = pair_state();
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if state.is_null() || store.is_null() || !is_stereo_pair_start(left) {
        return;
    }
    let pack = read_u8(store, 0);
    if pack < 0x20 {
        write_u32(state, STATE_ARM_TOKEN, encode_arm_token(pack, left));
    }
}

#[cfg(sample_cache_refresh_poc)]
unsafe fn sample_pair_publish_refresh_token(state: *mut u8, stereo: bool) {
    #[cfg(native_publish_boundary_probe_diagnostic)]
    if stereo {
        native_publish_boundary_probe_or(state, 1 << 0);
    }
    let terminal_slot = read_u32(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT) as u8;
    let left = if stereo {
        let Some(left) = stereo_left_for_right(terminal_slot) else {
            return;
        };
        #[cfg(native_publish_boundary_probe_diagnostic)]
        native_publish_boundary_probe_or(state, 1 << 1);
        left
    } else {
        terminal_slot
    };
    let pack = read_u8(state, STATE_DESCRIPTOR + DESCRIPTOR_PACK);
    if pack < 0x20 {
        #[cfg(native_publish_boundary_probe_diagnostic)]
        if stereo {
            native_publish_boundary_probe_or(state, 1 << 3);
        }
        write_u32(
            state,
            STATE_REFRESH_TOKEN,
            REFRESH_TOKEN_MAGIC
                | ((pack as u32) << 8)
                | left as u32
                | if stereo { REFRESH_TOKEN_STEREO } else { 0 },
        );
        #[cfg(native_publication_probe_diagnostic)]
        if stereo {
            native_publication_probe_or(state, 1 << 0);
        }
        #[cfg(native_publish_boundary_probe_diagnostic)]
        if stereo {
            native_publish_boundary_probe_or(state, 1 << 4);
        }
    }
}

#[cfg(sample_cache_refresh_poc)]
unsafe fn reload_sample_slot(slot: u8, pack: u8) -> u32 {
    const QUERY_SAMPLE_SLOT: usize = 0x0804_736d;
    const INVALIDATE_SAMPLE_SLOT: usize = 0x0804_a4a9;
    const LOAD_SAMPLE_SLOT: usize = 0x0804_60b9;

    let request = [4u32, slot as u32, pack as u32];
    function::<InvalidateSampleSlot>(INVALIDATE_SAMPLE_SLOT)(slot as u32);
    if function::<QuerySampleSlot>(QUERY_SAMPLE_SLOT)(0, request.as_ptr()) == 0 {
        return RELOAD_QUERY_MISS;
    }
    if function::<LoadSampleSlot>(LOAD_SAMPLE_SLOT)(slot as u32) == 0 {
        RELOAD_SUCCEEDED
    } else {
        RELOAD_FAILED
    }
}

#[cfg(any(sample_cache_refresh_poc, sample_pair_identity_poc))]
unsafe fn publish_sample_slot(slot: u8) {
    const PUBLISH_SAMPLE_SLOT: usize = 0x0803_1783;

    let controller = read_volatile(SAVE_CALLBACK_REGISTRATION.add(1)) as *const u8;
    if controller.is_null() {
        return;
    }
    let observers = read_u32(controller, 0x18) as *mut u8;
    if !observers.is_null() {
        function::<PublishSampleSlot>(PUBLISH_SAMPLE_SLOT)(observers, slot as u32);
    }
}

#[cfg(sample_pair_identity_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_note_wav_header(header: *const u8, slot: u32) {
    if header.is_null() || slot >= 128 {
        return;
    }
    let state = pair_state();
    if state.is_null() || read_u8(state, STATE_SCAN_MODE) != SCAN_MODE_FULL {
        return;
    }
    let slot = slot as u8;
    let byte_rate = read_u32(header, WAV_BYTE_RATE_OFFSET);
    // Witness what the scan actually sees per slot, so a missing pair can be
    // attributed to an unstamped header rather than to the pairing logic.
    #[cfg(all(native_dual_capture_poc, crop_io_trace))]
    let _ = byte_rate;
    if byte_rate == WAV_BYTE_RATE_STEREO_LEFT && is_stereo_pair_start(slot) {
        write_u8(state, STATE_SCAN_PENDING_LEFT, slot);
    } else {
        if byte_rate == WAV_BYTE_RATE_STEREO_RIGHT
            && slot > 0
            && read_u8(state, STATE_SCAN_PENDING_LEFT) == slot - 1
        {
            if let Some(pair) = pair_index(slot - 1) {
                let scan_bank = read_u8(state, STATE_SCAN_BANK);
                if scan_bank < 2 {
                    registry_set_bit(registry_bank_mut(state, scan_bank), pair);
                }
            }
        }
        write_u8(state, STATE_SCAN_PENDING_LEFT, 0xff);
    }
}

#[cfg(sample_pair_identity_poc)]
unsafe fn run_pair_full_scan(
    stock_load: usize,
    success_is_zero: bool,
    argument: u32,
) -> u32 {
    type PackLoad = unsafe extern "C" fn(u32) -> u32;

    let state = pair_state();
    if state.is_null() {
        return function::<PackLoad>(stock_load)(argument);
    }
    if read_u8(state, STATE_SCAN_MODE) != SCAN_MODE_IDLE {
        return function::<PackLoad>(stock_load)(argument);
    }
    let inactive = (read_u8(state, STATE_PAIR_ACTIVE_BANK) ^ 1) & 1;
    let bank = registry_bank_mut(state, inactive);
    let mut offset = 0;
    while offset < PAIR_BANK_SIZE {
        write_u8(bank, offset, 0);
        offset += 1;
    }
    write_u8(state, STATE_SCAN_MODE, SCAN_MODE_FULL);
    write_u8(state, STATE_SCAN_BANK, inactive);
    write_u8(state, STATE_SCAN_PENDING_LEFT, 0xff);
    let epoch = read_u32(state, STATE_MUTATION_EPOCH);
    write_u32(state, STATE_SCAN_EPOCH, epoch);

    let result = function::<PackLoad>(stock_load)(argument);
    let failed = if success_is_zero {
        result != 0
    } else {
        result == 0
    };
    if !failed && read_u32(state, STATE_MUTATION_EPOCH) == epoch {
        // Bind the pack from the load argument, which is authoritative here.
        // The previous source - byte 0 of the active content store - is the
        // read side of a double-buffered observable that the stock loader only
        // publishes *after* this call returns (see FUN_08039c70), so it still
        // held the previous pack at commit time. That left the pack unbound
        // (0xff) and pushed every later lookup onto a fallback comparison
        // against the same stale byte, which is why a Clear could not
        // recognise a pair until something else happened to rebind the pack.
        let active_pack = if argument < 0x20 {
            argument as u8
        } else {
            0xff
        };
        write_u8(
            state,
            STATE_PAIR_ACTIVE_PACK,
            active_pack,
        );
        write_u8(state, STATE_PAIR_VALID, 1);
        write_u32(
            state,
            STATE_REGISTRY_GENERATION,
            read_u32(state, STATE_REGISTRY_GENERATION).wrapping_add(1),
        );
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
        write_u8(state, STATE_PAIR_ACTIVE_BANK, inactive);
    }
    write_u8(state, STATE_SCAN_MODE, SCAN_MODE_IDLE);
    write_u8(state, STATE_SCAN_BANK, SCAN_BANK_NONE);
    write_u8(state, STATE_SCAN_PENDING_LEFT, 0xff);
    result
}

#[cfg(sample_pair_identity_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_pack_load(pack: u32) -> u32 {
    run_pair_full_scan(0x0804_60a9, true, pack)
}

#[cfg(sample_pair_identity_poc)]
#[no_mangle]
unsafe extern "C" fn sample_pair_direct_pack_load(pack: u32) -> u32 {
    run_pair_full_scan(0x0805_3e25, false, pack)
}

#[cfg(not(sample_pair_writer_coupling_poc))]
unsafe fn sample_pair_publish_arm_token(_left: u8) {}

#[cfg(sample_pair_writer_coupling_poc)]
unsafe fn sample_pair_clear_arm_token() {
    let state = pair_state();
    if !state.is_null()
        && read_u32(state, STATE_ARM_TOKEN) & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC
    {
        write_u32(state, STATE_ARM_TOKEN, 0);
        #[cfg(native_dual_capture_poc)]
        write_u32(state, STATE_DEFERRED_REFRESH_VIEW, 0);
    }
}

#[cfg(not(sample_pair_writer_coupling_poc))]
unsafe fn sample_pair_clear_arm_token() {}

#[cfg(sample_pair_writer_coupling_poc)]
unsafe fn sample_pair_arm_is_active(left: u8) -> bool {
    let state = pair_state();
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    is_stereo_pair_start(left)
        && !state.is_null()
        && !store.is_null()
        && read_u32(state, STATE_ARM_TOKEN) == encode_arm_token(read_u8(store, 0), left)
}

#[cfg(not(sample_pair_writer_coupling_poc))]
unsafe fn sample_pair_arm_is_active(_left: u8) -> bool {
    true
}

#[cfg(sample_pair_writer_coupling_poc)]
unsafe fn arm_token_matches(state: *const u8, descriptor: *const u8, left: u8) -> bool {
    let token = read_u32(state, STATE_ARM_TOKEN);
    token & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC
        && token == encode_arm_token(read_u8(descriptor, DESCRIPTOR_PACK), left)
        && is_stereo_pair_start(left)
}

#[cfg(native_arm_token_probe_diagnostic)]
unsafe fn arm_token_probe(
    state: *mut u8,
    descriptor: *const u8,
    left: u8,
    capture_slot: u8,
    capture_ui_token: u32,
) -> bool {
    let token = read_u32(state, STATE_ARM_TOKEN);
    let descriptor_pack = read_u8(descriptor, DESCRIPTOR_PACK);
    let token_left = token as u8;
    let token_pack = ((token >> 8) & 0x1f) as u8;
    let encoded = encode_arm_token(descriptor_pack, left);
    let magic = token & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC;
    let exact = token == encoded;
    let left_geometry = is_stereo_pair_start(left);
    let matches = magic && exact && left_geometry;
    let probe = NATIVE_ARM_TOKEN_PROBE_MAGIC
        | 1
        | ((magic as u32) << 1)
        | (((token_left == left) as u32) << 2)
        | (((token_left == capture_slot) as u32) << 3)
        | (((token_pack == descriptor_pack) as u32) << 4)
        | ((((token & 0x0000_e000) == 0) as u32) << 5)
        | ((left_geometry as u32) << 6)
        | ((is_stereo_pair_start(token_left) as u32) << 7)
        | (((capture_slot == left) as u32) << 8)
        | ((exact as u32) << 9)
        | ((matches as u32) << 10)
        | ((((capture_ui_token & ARM_TOKEN_MAGIC_MASK) == ARM_TOKEN_MAGIC) as u32) << 11)
        | (((token == capture_ui_token) as u32) << 12)
        | ((((capture_ui_token as u8) == left) as u32) << 13)
        | (((((capture_ui_token >> 8) & 0x1f) as u8 == descriptor_pack) as u32) << 14);
    write_u32(state, STATE_NATIVE_ARM_TOKEN_PROBE, probe);
    matches
}

#[cfg(native_pack_value_probe_diagnostic)]
unsafe fn pack_value_probe(state: *mut u8, descriptor: *const u8, left: u8) -> bool {
    let token = read_u32(state, STATE_ARM_TOKEN);
    let token_pack = ((token >> 8) & 0x1f) as u8;
    let descriptor_pack = read_u8(descriptor, DESCRIPTOR_PACK);
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    let store_pack = if store.is_null() { 0 } else { read_u8(store, 0) };
    write_u32(
        state,
        STATE_NATIVE_PACK_VALUE_PROBE,
        NATIVE_PACK_VALUE_PROBE_MAGIC
            | token_pack as u32
            | ((descriptor_pack as u32 & 0x1f) << 5)
            | ((store_pack as u32 & 0x1f) << 10)
            | ((!store.is_null() as u32) << 15),
    );
    token & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC
        && token == encode_arm_token(descriptor_pack, left)
        && is_stereo_pair_start(left)
}

#[cfg(native_dual_capture_poc)]
unsafe fn native_stereo_authenticated_pack(
    state: *const u8,
    left: u8,
    capture_slot: u8,
    capture_ui_token: u32,
) -> Option<u8> {
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if store.is_null() {
        return None;
    }
    let canonical_pack = read_u8(store, 0);
    let token = read_u32(state, STATE_ARM_TOKEN);
    let token_pack = ((token >> 8) & 0x1f) as u8;
    if token & ARM_TOKEN_MAGIC_MASK != ARM_TOKEN_MAGIC
        || capture_ui_token != token
        || token as u8 != left
        || capture_slot != left
        || !is_stereo_pair_start(left)
        || canonical_pack >= 0x20
        || token_pack != canonical_pack
    {
        return None;
    }
    Some(canonical_pack)
}

#[cfg(native_pack_correction_probe_diagnostic)]
unsafe fn native_pack_correction_probe_or(state: *mut u8, flags: u32) {
    write_u32(
        state,
        STATE_NATIVE_PACK_CORRECTION_PROBE,
        read_u32(state, STATE_NATIVE_PACK_CORRECTION_PROBE) | flags,
    );
}

#[cfg(native_pack_correction_probe_diagnostic)]
unsafe fn native_pack_correction_probe_clear(state: *mut u8, flags: u32) {
    write_u32(
        state,
        STATE_NATIVE_PACK_CORRECTION_PROBE,
        read_u32(state, STATE_NATIVE_PACK_CORRECTION_PROBE) & !flags,
    );
}

#[cfg(native_dual_capture_poc)]
#[inline(always)]
unsafe fn primary_dsp_program_is_active() -> bool {
    let program = read_volatile(DSP_PROGRAM_STATE) as *const u8;
    !program.is_null() && read_volatile(program) == 0
}

#[cfg(native_dual_capture_poc)]
unsafe fn native_capture_token_slot() -> Option<(u8, u8)> {
    let state = pair_state();
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if state.is_null() || store.is_null() {
        return None;
    }
    let token = read_u32(state, STATE_ARM_TOKEN);
    if token & ARM_TOKEN_MAGIC_MASK != ARM_TOKEN_MAGIC {
        return None;
    }
    Some((((token >> 8) & 0x1f) as u8, token as u8))
}

#[cfg(native_dual_capture_poc)]
#[no_mangle]
#[link_section = ".text.stereo_start_request"]
pub unsafe extern "C" fn native_capture_source_start(
    source: *mut u8,
    slot: u32,
    arg3: u32,
    arg4: u32,
) -> u32 {
    const STOCK_CAPTURE_START: usize = 0x0806_446f;
    const STOCK_CAPTURE_CLEAR: usize = 0x0806_4461;
    const DSP_WRITE: usize = 0x0803_1229;
    const DSP_READ: usize = 0x0803_14d7;
    const SCHEDULER_YIELD: usize = 0x0803_172f;
    const CAPTURE_COMMAND: u32 = 0x0003_72f9;
    const CLEAR_COMMAND: u32 = 0x0003_72fc;

    let state = pair_state();
    let token = native_capture_token_slot();
    if token.is_none() {
        if !state.is_null() {
            stereo_carry_displaced_latch(state, 0);
            write_u32(state, STATE_CAPTURE_UI_TOKEN, 0);
            write_u32(state, STATE_SAVE_PAIR_LATCH, 0);
            write_u32(state, STATE_SAVE_VIEW_LATCH, 0);
            #[cfg(native_capture_preflight_probe_diagnostic)]
            write_u32(
                state,
                STATE_CAPTURE_PREFLIGHT_PROBE,
                CAPTURE_PREFLIGHT_PROBE_MAGIC,
            );
        }
        return function::<CaptureSourceStart>(STOCK_CAPTURE_START)(source, slot, arg3, arg4);
    }
    let (pack, left) = token.unwrap();
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    #[cfg(native_capture_preflight_probe_diagnostic)]
    let mut descriptor = [4u32, left as u32, pack as u32];
    let slot_in_range = slot < 127;
    let slot_matches = slot as u8 == left;
    let pair_start_valid = is_stereo_pair_start(left);
    let state_valid = !state.is_null();
    let store_valid = !store.is_null();
    let pack_matches = store_valid && read_u8(store, 0) == pack;
    let primary_dsp_active = primary_dsp_program_is_active();
    #[cfg(native_capture_preflight_probe_diagnostic)]
    {
        let left_available =
            state_valid
                && store_valid
                && !destination_is_known_occupied(descriptor.as_mut_ptr().cast::<u8>(), left);
        let right_available =
            state_valid
                && store_valid
                && !destination_is_known_occupied(descriptor.as_mut_ptr().cast::<u8>(), left + 1);
        let conditions = (slot_in_range as u32)
            | ((slot_matches as u32) << 1)
            | ((pair_start_valid as u32) << 2)
            | ((state_valid as u32) << 3)
            | ((store_valid as u32) << 4)
            | ((pack_matches as u32) << 5)
            | ((primary_dsp_active as u32) << 6)
            | ((left_available as u32) << 7)
            | ((right_available as u32) << 8);
        write_u32(
            state,
            STATE_CAPTURE_PREFLIGHT_PROBE,
            CAPTURE_PREFLIGHT_PROBE_MAGIC | CAPTURE_PREFLIGHT_PROBE_ENTRY | conditions,
        );
    }
    let valid = slot_in_range
        && slot_matches
        && pair_start_valid
        && state_valid
        && store_valid
        && pack_matches
        && primary_dsp_active;
    if !valid {
        stereo_carry_displaced_latch(state, 0);
        write_u32(state, STATE_CAPTURE_UI_TOKEN, 0);
        write_u32(state, STATE_SAVE_PAIR_LATCH, 0);
        write_u32(state, STATE_SAVE_VIEW_LATCH, 0);
        clear_arm_token(state);
        return arg4;
    }

    write_u32(state, STATE_CAPTURE_UI_TOKEN, encode_arm_token(pack, left));
    // Latch the pair identity and the Sample Rec view for the save pipeline.
    // The gesture-owned token and view pointer are cleared by the next pad
    // press, which routinely lands before the right slot's Save completion.
    stereo_carry_displaced_latch(state, encode_arm_token(pack, left));
    write_u32(state, STATE_SAVE_PAIR_LATCH, encode_arm_token(pack, left));
    write_u32(
        state,
        STATE_SAVE_VIEW_LATCH,
        read_u32(state, STATE_DEFERRED_REFRESH_VIEW),
    );
    write_u8(source, 4, 1);
    function::<CaptureSourceClear>(STOCK_CAPTURE_CLEAR)(source, left as u32 + 1);
    while function::<DspRead>(DSP_READ)(0, CLEAR_COMMAND) != 0 {
        function::<SchedulerYield>(SCHEDULER_YIELD)();
    }
    let command = ((left as u32) << 24) | 0x700;
    function::<DspWrite>(DSP_WRITE)(0, command, CAPTURE_COMMAND);
    while function::<DspRead>(DSP_READ)(0, CAPTURE_COMMAND) != 0 {
        function::<SchedulerYield>(SCHEDULER_YIELD)();
    }
    arg4
}

#[cfg(native_dual_capture_poc)]
core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb
    .section .text.native_dsp_upload_primary_b,"ax",%progbits
    .global native_dsp_upload_primary_b
    .type native_dsp_upload_primary_b,%function
    .thumb_func
native_dsp_upload_primary_b:
    ldr r2, 9f
    cmp r0, r2
    bne 8f
    ldr r2, native_dsp_extension_pointer
    cbz r2, 8f

    push.w {{r4-r8, lr}}
    sub sp, #8
    str r1, [sp]
    mov r4, r0
    ldr r5, 10f
    mov r6, r2
    ldr r7, 11f
    blx r7
    ldr r3, 12f
    ldr r0, [sp]
    blx r3
1:
    movw r8, #0x1fe
    cmp r5, r8
    it lo
    movlo r8, r5
    mov r0, r4
    mov r1, r8
    ldr r3, 13f
    blx r3
    blx r7
    add r4, r8
    subs r5, r8
    bne 1b
    cbz r6, 2f
    mov r4, r6
    movw r5, #1134
    movs r6, #0
    b 1b
2:
    blx r7
    ldr r3, 14f
    ldr r0, [sp]
    blx r3
    add sp, #8
    pop.w {{r4-r8, pc}}
8:
    ldr r3, 15f
    bx r3
    .align 2
9:
    .word 0x080101f0
10:
    .word 42549
11:
    .word 0x08046415
12:
    .word 0x08031049
13:
    .word 0x08046505
14:
    .word 0x0803107b
15:
    .word 0x080310b1
    .global native_dsp_extension_pointer
    .type native_dsp_extension_pointer,%object
native_dsp_extension_pointer:
    .word 0
    .size native_dsp_extension_pointer, 4
    .size native_dsp_upload_primary_b, .-native_dsp_upload_primary_b
"#
);

#[cfg(sample_pair_writer_coupling_poc)]
unsafe fn clear_arm_token(state: *mut u8) {
    write_u32(state, STATE_ARM_TOKEN, 0);
}

/// Index of the latch belonging to the pair `track` leads.
///
/// Pairing is derived from the sample assignment, so any adjacent pair is
/// allowed and a leader may be any track but the last.
#[cfg(paired_playback_poc)]
fn playback_pair_index(track: u32) -> Option<usize> {
    let offset = track.checked_sub(PLAYBACK_LEADER_TRACK)?;
    if offset < 7 {
        Some(offset as usize)
    } else {
        None
    }
}

/// Whether `leader_track` and the track above it hold the two halves of one
/// identified sample pair.
#[cfg(paired_playback_poc)]
unsafe fn paired_pair_enabled(leader_track: u32) -> bool {
    paired_playback_assignment(leader_track)
        .map(|(_, _, enabled)| enabled)
        .unwrap_or(false)
}

/// Resolves a 0-based track/knob index to the 0-based leader index of the
/// derived stereo pair it belongs to, whether it is the leader or the follower.
///
/// A slot is either the left or the right member of exactly one pair, never
/// both, so a track can never be simultaneously a leader and a follower and
/// adjacent pairs can never overlap or chain. That is what makes arbitrary
/// adjacency safe without any separate occupancy bookkeeping.
#[cfg(paired_playback_poc)]
unsafe fn paired_track_leader_index(index: u32) -> Option<u32> {
    if index >= 8 {
        return None;
    }
    if paired_pair_enabled(PLAYBACK_LEADER_TRACK + index) {
        return Some(index);
    }
    let previous = index.checked_sub(1)?;
    if paired_pair_enabled(PLAYBACK_LEADER_TRACK + previous) {
        return Some(previous);
    }
    None
}

#[cfg(paired_playback_poc)]
unsafe fn paired_playback_assignment(leader_track: u32) -> Option<(u8, u8, bool)> {
    const TRACK_PARAMETER_GET: usize = 0x0804_6871;
    const SAMPLE_PARAMETER: u32 = 0;

    playback_pair_index(leader_track)?;
    let state = pair_state();
    if state.is_null() {
        return None;
    }
    // Must be the bound registry pack, not byte 0 of the active content store:
    // that byte is the read side of a double-buffered observable and still names
    // the *previous* pack right after a boot. Keying on it is the defect that
    // made pairs fail to show teal until a Clear forced a republish, and it
    // would make every coupled mixer/FX/sidechain control mis-detect pairing in
    // exactly the same window.
    let pack = registry_pack(state);
    if pack >= 0x20 {
        return None;
    }
    let get = function::<TrackParameterGet>(TRACK_PARAMETER_GET);
    let leader = get(leader_track, SAMPLE_PARAMETER);
    let follower = get(leader_track + 1, SAMPLE_PARAMETER);
    if leader >= 128 || follower >= 128 {
        return None;
    }
    let leader = leader as u8;
    let follower = follower as u8;
    let enabled = leader & 7 != 7
        && follower == leader + 1
        && identified_pair_left(state, pack, leader) == Some(leader);
    Some((leader, follower, enabled))
}

/// Captured at the moment a Clear fails to recognise a pair, so the specific
/// precondition that failed is reported rather than inferred.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
unsafe fn pair_lookup_witness(state: *const u8, pack: u8, slot: u8) {
    if state.is_null() {
        return;
    }
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    let store_pack = if store.is_null() { 0x3f } else { read_u8(store, 0) };
    write_u32(
        state as *mut u8,
        CROP_REG_DIAG,
        (pack as u32 & 0x3f)
            | (store.is_null() as u32) << 6
            | ((read_u8(state, STATE_PAIR_VALID) != 0) as u32) << 7
            | (store_pack as u32 & 0x3f) << 8
            | ((read_u8(state, STATE_PAIR_ACTIVE_PACK) == 0xff) as u32) << 14
    );
}

#[cfg(not(all(native_dual_capture_poc, crop_io_trace)))]
#[inline(always)]
unsafe fn pair_lookup_witness(_state: *const u8, _pack: u8, _slot: u8) {}

#[cfg(sample_pair_identity_poc)]
unsafe fn identified_pair_left(state: *const u8, pack: u8, slot: u8) -> Option<u8> {
    if pack >= 0x20 {
        return None;
    }
    if slot & 7 != 0 {
        let left = slot - 1;
        if let Some(pair) = pair_index(left) {
            if registry_contains(state, pack, pair) {
                return Some(left);
            }
        }
    }
    if slot & 7 != 7 {
        if let Some(pair) = pair_index(slot) {
            if registry_contains(state, pack, pair) {
                return Some(slot);
            }
        }
    }
    None
}

#[cfg(sample_pair_identity_poc)]
unsafe fn clear_pair_identity(state: *mut u8, descriptor: *const u8, slot: u8) {
    let pack = read_u8(descriptor, DESCRIPTOR_PACK);
    registry_clear_slot(state, pack, slot);
}

unsafe fn active_stereo_prepare_matches(descriptor: *const u8) -> bool {
    let manager = read_volatile(SAMPLING_MANAGER) as *const u8;
    if manager.is_null() {
        return false;
    }
    let state = read_u32(manager, 0x04) as *const u8;
    if state.is_null() || read_u8(state, STATE_WRITER_MODE) != WRITER_MODE_CUSTOM_STEREO {
        return false;
    }
    let active_descriptor = state.add(STATE_DESCRIPTOR);
    if read_u32(active_descriptor, DESCRIPTOR_TYPE) != 4
        || read_u32(descriptor, DESCRIPTOR_TYPE) != 4
        || read_u8(active_descriptor, DESCRIPTOR_PACK) != read_u8(descriptor, DESCRIPTOR_PACK)
    {
        return false;
    }
    let active_slot = read_u32(active_descriptor, DESCRIPTOR_SLOT) as u8;
    let pair_start = match read_u32(state, STATE_MAGIC) {
        WRITER_MAGIC_LEFT if is_stereo_pair_start(active_slot) => active_slot,
        WRITER_MAGIC_RIGHT_PENDING | WRITER_MAGIC_RIGHT
            if stereo_left_for_right(active_slot).is_some() =>
        {
            active_slot - 1
        }
        _ => return false,
    };
    let slot = read_u32(descriptor, DESCRIPTOR_SLOT) as u8;
    slot == pair_start || slot == pair_start + 1
}

unsafe fn destination_is_empty(descriptor: *const u8, slot: u8) -> bool {
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if store.is_null() || read_u8(store, 0) != read_u8(descriptor, DESCRIPTOR_PACK) {
        return false;
    }

    type TypeToIndex = unsafe extern "C" fn(u32) -> u32;
    type GetItem = unsafe extern "C" fn(*const u8, u32) -> *const u8;
    let category =
        function::<TypeToIndex>(CONTENT_TYPE_TO_INDEX)(read_u32(descriptor, DESCRIPTOR_TYPE));
    let container = read_u32(store, 0x28 + category as usize * 4) as *const u8;
    !container.is_null()
        && function::<GetItem>(CONTENT_INDEX_GET_ITEM)(container, slot as u32).is_null()
}

unsafe fn destination_is_known_occupied(descriptor: *const u8, slot: u8) -> bool {
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if store.is_null() || read_u8(store, 0) != read_u8(descriptor, DESCRIPTOR_PACK) {
        return true;
    }

    type TypeToIndex = unsafe extern "C" fn(u32) -> u32;
    type GetItem = unsafe extern "C" fn(*const u8, u32) -> *const u8;
    let category =
        function::<TypeToIndex>(CONTENT_TYPE_TO_INDEX)(read_u32(descriptor, DESCRIPTOR_TYPE));
    let container = read_u32(store, 0x28 + category as usize * 4) as *const u8;
    !container.is_null()
        && !function::<GetItem>(CONTENT_INDEX_GET_ITEM)(container, slot as u32).is_null()
}

#[no_mangle]
#[cfg(safe_pair_guard)]
pub unsafe extern "C" fn stereo_prepare_destination(descriptor: *mut u8) -> u32 {
    if read_u32(descriptor, DESCRIPTOR_TYPE) == 4 {
        let slot = read_u32(descriptor, DESCRIPTOR_SLOT) as u8;
        #[cfg(sample_pair_identity_poc)]
        {
            let state = pair_state();
            if !state.is_null() {
                let active_writer = active_stereo_prepare_matches(descriptor);
                if read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_CUSTOM_STEREO
                    && !active_writer
                {
                    return PROTECTED_OCCUPIED_STATUS;
                }
                let clear_phase = read_u8(state, STATE_CLEAR_PHASE);
                if matches!(
                    clear_phase,
                    CLEAR_PHASE_RIGHT | CLEAR_PHASE_LEFT | CLEAR_PHASE_SINGLE
                ) {
                    let pack = read_u8(descriptor, DESCRIPTOR_PACK);
                    let clear_pack = read_u8(state, STATE_CLEAR_PACK);
                    let left = read_u8(state, STATE_CLEAR_LEFT);
                    let allowed = if clear_phase == CLEAR_PHASE_SINGLE {
                        slot
                            == read_u32(
                                state.add(STATE_CLEAR_DESCRIPTOR_LEFT),
                                DESCRIPTOR_SLOT,
                            ) as u8
                    } else {
                        slot == left || slot == left.wrapping_add(1)
                    };
                    if pack != clear_pack || !allowed {
                        return PROTECTED_OCCUPIED_STATUS;
                    }
                }
            }
        }
    }
    stock_prepare_destination_trampoline(descriptor)
}

#[cfg(sample_pair_identity_poc)]
unsafe fn copy_storage_descriptor(destination: *mut u8, source: *const u8, slot: u8) {
    write_u32(destination, DESCRIPTOR_TYPE, read_u32(source, DESCRIPTOR_TYPE));
    write_u32(destination, DESCRIPTOR_SLOT, slot as u32);
    write_u32(destination, DESCRIPTOR_PACK, 0);
    write_u8(
        destination,
        DESCRIPTOR_PACK,
        read_u8(source, DESCRIPTOR_PACK),
    );
}

#[cfg(sample_pair_identity_poc)]
unsafe fn reject_clear_request(
    coordinator: *mut u8,
    descriptor: *const u8,
    completion: *mut u8,
) {
    if !coordinator.is_null() {
        write_u32(coordinator, 4, 0);
    }
    if !descriptor.is_null() && read_u32(descriptor, DESCRIPTOR_TYPE) == 4 {
        let _ = reload_sample_slot(
            read_u32(descriptor, DESCRIPTOR_SLOT) as u8,
            read_u8(descriptor, DESCRIPTOR_PACK),
        );
    }
    if completion.is_null() {
        return;
    }
    let vtable = read_u32(completion, 0) as *const u8;
    if !vtable.is_null() {
        function::<CompletionObjectCall>(read_u32(vtable, 0) as usize)(
            completion,
            descriptor as u32,
        );
    }
}

#[cfg(sample_pair_identity_poc)]
unsafe fn complete_single_clear(state: *mut u8, descriptor: u32, status: u32) {
    const ASYNC_SYNC: usize = 0x0804_60c9;

    let pack = read_u8(state.add(STATE_CLEAR_DESCRIPTOR_LEFT), DESCRIPTOR_PACK);
    let slot = read_u32(state.add(STATE_CLEAR_DESCRIPTOR_LEFT), DESCRIPTOR_SLOT) as u8;
    if status == 0 {
        registry_clear_slot(state, pack, slot);
    } else {
        let _ = reload_sample_slot(slot, pack);
    }
    let sync_status = function::<AsyncStorageRequest>(ASYNC_SYNC)(
        state.add(STATE_CLEAR_DESCRIPTOR_LEFT),
        sample_pair_clear_sync_callback as *const () as usize,
        state,
    );
    if sync_status != 0 {
        let token = read_u32(state, STATE_REFRESH_TOKEN);
        if token & ARM_TOKEN_MAGIC_MASK != REFRESH_TOKEN_MAGIC
            || token & REFRESH_TOKEN_STEREO == 0
        {
            write_u32(
                state,
                STATE_REFRESH_TOKEN,
                REFRESH_DIAGNOSTIC_LOAD_FAILED | ((pack as u32) << 8) | slot as u32,
            );
        }
    }
    publish_sample_slot(slot);
    let coordinator = read_u32(state, STATE_CLEAR_COORDINATOR) as *mut u8;
    let completion = read_u32(state, STATE_CLEAR_COMPLETION) as *mut u8;
    if !coordinator.is_null() {
        write_u32(coordinator, 4, 0);
    }
    write_u8(state, STATE_CLEAR_PHASE, CLEAR_PHASE_IDLE);
    write_u8(state, STATE_CLEAR_PACK, 0xff);
    write_u8(state, STATE_CLEAR_LEFT, 0xff);
    write_u32(state, STATE_CLEAR_COORDINATOR, 0);
    write_u32(state, STATE_CLEAR_COMPLETION, 0);
    if !completion.is_null() {
        let vtable = read_u32(completion, 0) as *const u8;
        if !vtable.is_null() {
            function::<CompletionObjectCall>(read_u32(vtable, 0) as usize)(
                completion,
                descriptor,
            );
        }
    }
}

#[cfg(sample_pair_identity_poc)]
unsafe fn resume_deferred_clear(state: *mut u8) {
    const ASYNC_PREPARE: usize = 0x0804_6099;

    if read_u8(state, STATE_CLEAR_PHASE) != CLEAR_PHASE_DEFERRED {
        return;
    }
    write_u8(state, STATE_CLEAR_PHASE, CLEAR_PHASE_SINGLE);
    let accepted = function::<AsyncStorageRequest>(ASYNC_PREPARE)(
        state.add(STATE_CLEAR_DESCRIPTOR_LEFT),
        sample_pair_clear_callback as *const () as usize,
        state,
    );
    if accepted != 0 {
        complete_single_clear(
            state,
            state.add(STATE_CLEAR_DESCRIPTOR_LEFT) as u32,
            accepted,
        );
    }
}

#[cfg(sample_pair_identity_poc)]
unsafe fn release_committed_writer(state: *mut u8) {
    write_u8(state, STATE_WRITER_MODE, WRITER_MODE_IDLE);
    write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
    write_u8(state, STATE_WRITER_WAIT_TICKS, 0);
    if read_u8(state, STATE_CLEAR_PHASE) == CLEAR_PHASE_DEFERRED {
        resume_deferred_clear(state);
    }
}

/// Arms the deferred Clear of a stereo pair's other member.
///
/// Stock Clear is a single-slot transaction: `SampleClear_OnComplete`
/// (`0x0803A07C`) clears exactly one occupancy bit, taken from the descriptor
/// it was given. Pair-wide Clear therefore needs two complete transactions.
///
/// Every earlier attempt failed by trying to coordinate two deletions inside
/// one transaction -- recursing into the storage worker from its own callback,
/// which stalled the durable-deletion lifecycle and left both files on disk and
/// the UI wedged. Instead this records the peer and lets
/// `service_pending_peer_clear` replay the ordinary Sample Clear UI entry for
/// it once the first transaction has gone idle, which is exactly what a second
/// pad press would do.
///
/// `completion` is `ui + 0x0c` at the stock call site `0x0803A1A4`, so the UI
/// object is recovered by subtracting that offset.
#[cfg(all(sample_pair_identity_poc, native_dual_capture_poc))]
unsafe fn arm_peer_clear(state: *mut u8, completion: *mut u8, left: u8, slot: u8) {
    const COMPLETION_OFFSET: usize = 0x0c;

    if completion.is_null() {
        return;
    }
    let peer = if slot == left { left + 1 } else { left };
    if peer >= 128 {
        return;
    }
    write_u32(
        state,
        STATE_PEER_CLEAR_UI,
        (completion as usize - COMPLETION_OFFSET) as u32,
    );
    write_u8(state, STATE_PEER_CLEAR_SLOT, peer);
    write_u8(state, STATE_PEER_CLEAR_ATTEMPTS, 0);
    write_u16(state, STATE_PEER_CLEAR_TICKS, 0);
    write_u16(state, STATE_PEER_CLEAR_BUSY_TICKS, 0);
}

/// Retires a deferred peer Clear.
#[cfg(all(sample_pair_identity_poc, native_dual_capture_poc))]
unsafe fn finish_peer_clear(state: *mut u8) {
    write_u8(state, STATE_PEER_CLEAR_SLOT, PEER_CLEAR_NONE);
    write_u8(state, STATE_PEER_CLEAR_ATTEMPTS, 0);
    write_u16(state, STATE_PEER_CLEAR_TICKS, 0);
    write_u16(state, STATE_PEER_CLEAR_BUSY_TICKS, 0);
    write_u32(state, STATE_PEER_CLEAR_UI, 0);
}

/// Releases a Sample Clear transaction that can never complete.
///
/// Mirrors the tail of `SampleClear_OnComplete` exactly -- take the write-side
/// buffer for element 1 of the transaction observable, zero it, publish -- so
/// the view is left in precisely the state a normal completion would leave it,
/// minus the occupancy edit that the dead transaction never earned.
#[cfg(all(sample_pair_identity_poc, native_dual_capture_poc))]
unsafe fn release_wedged_clear_transaction(ui: *mut u8) {
    const TRANSACTION_OBSERVABLE: usize = 0x20;
    const BUSY_ELEMENT: u32 = 1;
    const OBSERVABLE_VALUE: usize = 0x0803_fd27;
    const OBSERVABLE_PUBLISH: usize = 0x0803_fbed;

    let observable = read_u32(ui, TRANSACTION_OBSERVABLE);
    if observable == 0 {
        return;
    }
    let busy = function::<ObservableValue>(OBSERVABLE_VALUE)(observable, BUSY_ELEMENT);
    if busy.is_null() {
        return;
    }
    write_u8(busy, 0, 0);
    function::<FirmwareMethod4>(OBSERVABLE_PUBLISH)(observable, 0, 0, 0);
}

/// True while the Sample view's Clear transaction is in progress.
///
/// Element 1 of the transaction observable at `ui + 0x20` is the byte
/// `0x0803A10E` tests on entry (returning 1 when set) and
/// `SampleClear_OnComplete` resets. An unreadable observable is reported busy
/// so the deferred request waits rather than guessing.
#[cfg(all(sample_pair_identity_poc, native_dual_capture_poc))]
unsafe fn sample_clear_transaction_busy(ui: *const u8) -> bool {
    const TRANSACTION_OBSERVABLE: usize = 0x20;
    const BUSY_ELEMENT: u32 = 1;
    const OBSERVABLE_READ: usize = 0x0803_fc59;

    let observable = read_u32(ui, TRANSACTION_OBSERVABLE);
    if observable == 0 {
        return true;
    }
    let busy = function::<ObservableValue>(OBSERVABLE_READ)(observable, BUSY_ELEMENT);
    busy.is_null() || read_u8(busy, 0) != 0
}

/// Reads a slot's bit from the Sample view's occupancy bitmap at `ui + 0x14`.
///
/// This is the same observable `SampleClear_OnComplete` publishes, so it is the
/// authoritative answer to "has this slot actually been cleared yet?".
#[cfg(all(sample_pair_identity_poc, native_dual_capture_poc))]
unsafe fn sample_slot_occupied(ui: *const u8, slot: u8) -> bool {
    const OCCUPANCY_OBSERVABLE: usize = 0x14;
    const OBSERVABLE_READ: usize = 0x0803_fc59;

    let observable = read_u32(ui, OCCUPANCY_OBSERVABLE);
    if observable == 0 {
        return true;
    }
    let word = function::<ObservableValue>(OBSERVABLE_READ)(observable, (slot >> 5) as u32);
    if word.is_null() {
        return true;
    }
    read_u32(word, 0) >> (slot & 31) & 1 != 0
}

/// Issues the deferred peer Clear once the stock transaction has settled.
///
/// Runs from the main loop, never from a storage callback, so the second
/// request is submitted from the same context as an ordinary user press.
///
/// Success is *observed*, not assumed: `0x0803A10E` returns 0 both when it
/// submitted the deletion and when it decided to do nothing, and the submit
/// itself is fire-and-forget, so the only trustworthy completion signal is the
/// peer's occupancy bit going clear. Until it does, the request is re-sent
/// after each settle window, up to `PEER_CLEAR_MAX_ATTEMPTS`.
/// Records which branch of the Clear path ran, so the "first pair Clear after a
/// boot misses the partner" symptom can be attributed to a specific early exit
/// instead of inferred. Accumulates; never cleared except by construction.
#[cfg(all(native_dual_capture_poc, crop_io_trace))]
unsafe fn clear_witness(state: *const u8, bit: u32) {
    if !state.is_null() {
        write_u32(
            state as *mut u8,
            CROP_REG_BITS,
            read_u32(state, CROP_REG_BITS) | 1 << bit,
        );
    }
}

#[cfg(not(all(native_dual_capture_poc, crop_io_trace)))]
#[inline(always)]
unsafe fn clear_witness(_state: *const u8, _bit: u32) {}

#[cfg(all(sample_pair_identity_poc, native_dual_capture_poc))]
unsafe fn service_pending_peer_clear() {
    const SAMPLE_CLEAR_UI_ENTRY: usize = 0x0803_a10f;
    type SampleClearEntry = unsafe extern "C" fn(*mut u8, u32) -> u32;

    let state = pair_state();
    if state.is_null() {
        return;
    }
    let peer = read_u8(state, STATE_PEER_CLEAR_SLOT);
    if peer == PEER_CLEAR_NONE {
        return;
    }
    let ui = read_u32(state, STATE_PEER_CLEAR_UI) as *mut u8;
    if ui.is_null() {
        finish_peer_clear(state);
        return;
    }
    // The peer is gone: either this request completed or it was never
    // occupied. Either way there is nothing left to do.
    if !sample_slot_occupied(ui, peer) {
        finish_peer_clear(state);
        return;
    }
    // Storage is still working. Restart the settle window rather than counting
    // ticks spent waiting, so the delay below always measures quiet time.
    //
    // A transaction that never completes parks here forever, which is exactly
    // the session-wide "Clear does nothing until reboot" wedge, so this is also
    // where it is detected and released.
    if sample_clear_transaction_busy(ui) {
        write_u16(state, STATE_PEER_CLEAR_TICKS, 0);
        let busy_ticks = read_u16(state, STATE_PEER_CLEAR_BUSY_TICKS);
        if busy_ticks >= PEER_CLEAR_WEDGE_TICKS {
            write_u16(state, STATE_PEER_CLEAR_BUSY_TICKS, 0);
            release_wedged_clear_transaction(ui);
        } else {
            write_u16(state, STATE_PEER_CLEAR_BUSY_TICKS, busy_ticks + 1);
        }
        return;
    }
    write_u16(state, STATE_PEER_CLEAR_BUSY_TICKS, 0);
    let ticks = read_u16(state, STATE_PEER_CLEAR_TICKS);
    if ticks < PEER_CLEAR_SETTLE_TICKS {
        write_u16(state, STATE_PEER_CLEAR_TICKS, ticks + 1);
        return;
    }
    write_u16(state, STATE_PEER_CLEAR_TICKS, 0);
    let attempts = read_u8(state, STATE_PEER_CLEAR_ATTEMPTS);
    if attempts >= PEER_CLEAR_MAX_ATTEMPTS {
        finish_peer_clear(state);
        return;
    }
    write_u8(state, STATE_PEER_CLEAR_ATTEMPTS, attempts + 1);
    function::<SampleClearEntry>(SAMPLE_CLEAR_UI_ENTRY)(ui, peer as u32);
}

#[cfg(sample_pair_identity_poc)]
#[no_mangle]
pub unsafe extern "C" fn sample_pair_clear_request(
    coordinator: *mut u8,
    descriptor: *const u8,
    completion: *mut u8,
) {
    const STOCK_CLEAR_COORDINATOR: usize = 0x0804_7333;

    let state = pair_state();
    let pack = if descriptor.is_null() {
        0xff
    } else {
        read_u8(descriptor, DESCRIPTOR_PACK)
    };
    let slot = if descriptor.is_null() {
        0xff
    } else {
        read_u32(descriptor, DESCRIPTOR_SLOT) as u8
    };
    clear_witness(state, 0);
    if state.is_null()
        || coordinator.is_null()
        || descriptor.is_null()
        || read_u32(descriptor, DESCRIPTOR_TYPE) != 4
    {
        clear_witness(state, 1);
        function::<StockClearCoordinator>(STOCK_CLEAR_COORDINATOR)(
            coordinator,
            descriptor,
            completion,
        );
        return;
    }
    if read_u8(state, STATE_CLEAR_PHASE) != CLEAR_PHASE_IDLE {
        clear_witness(state, 2);
        reject_clear_request(coordinator, descriptor, completion);
        return;
    }
    let pair_left = identified_pair_left(state, pack, slot);
    if pair_left.is_some() {
        clear_witness(state, 3);
    }
    if pair_left.is_none()
        && read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_CUSTOM_STEREO
    {
        copy_storage_descriptor(state.add(STATE_CLEAR_DESCRIPTOR_LEFT), descriptor, slot);
        write_u8(state, STATE_CLEAR_PACK, pack);
        write_u8(state, STATE_CLEAR_LEFT, 0xff);
        write_u32(state, STATE_CLEAR_COORDINATOR, coordinator as u32);
        write_u32(state, STATE_CLEAR_COMPLETION, completion as u32);
        write_u32(coordinator, 4, completion as u32);
        write_u8(state, STATE_CLEAR_PHASE, CLEAR_PHASE_DEFERRED);
        return;
    };
    let Some(left) = pair_left else {
        clear_witness(state, 4);
        pair_lookup_witness(state, pack, slot);
        function::<StockClearCoordinator>(STOCK_CLEAR_COORDINATOR)(
            coordinator,
            descriptor,
            completion,
        );
        return;
    };
    if read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_CUSTOM_STEREO {
        clear_witness(state, 5);
        reject_clear_request(coordinator, descriptor, completion);
        return;
    }
    // Pair-wide Clear. The pressed pad was already canonicalised to the left
    // member by `sample_pair_clear_canonical_pad`, so the stock transaction
    // below deletes whichever member was addressed; arm the other one to be
    // deleted by an independent, equally stock transaction from the main loop.
    arm_peer_clear(state, completion, left, slot);
    clear_witness(state, 6);
    if read_u8(state, STATE_PEER_CLEAR_SLOT) != PEER_CLEAR_NONE {
        clear_witness(state, 7);
    }
    registry_clear_slot(state, pack, left);
    function::<StockClearCoordinator>(STOCK_CLEAR_COORDINATOR)(
        coordinator,
        descriptor,
        completion,
    );
}

#[cfg(sample_pair_identity_poc)]
#[no_mangle]
pub unsafe extern "C" fn sample_pair_clear_callback(
    descriptor: u32,
    _arg2: u32,
    status: u32,
    _arg4: u32,
    _arg5: u32,
    state: *mut u8,
) {
    if state.is_null() {
        return;
    }
    if read_u8(state, STATE_CLEAR_PHASE) == CLEAR_PHASE_SINGLE {
        complete_single_clear(state, descriptor, status);
    }
}

#[cfg(sample_pair_identity_poc)]
#[no_mangle]
pub unsafe extern "C" fn sample_pair_clear_sync_callback(
    _arg1: u32,
    _arg2: u32,
    _status: u32,
    _arg4: u32,
    _arg5: u32,
    _context: *mut u8,
) {
}

#[cfg(sample_cache_refresh_poc)]
#[no_mangle]
pub unsafe extern "C" fn sample_pair_save_publish(observers: u32, slot: u32) -> u32 {
    type StockPublish = unsafe extern "C" fn(u32, u32) -> u32;
    const STOCK_PUBLISH: usize = 0x0803_1783;

    #[cfg(native_publish_boundary_probe_diagnostic)]
    let (boundary_active, mut boundary_flags) = {
        let pre_state = pair_state();
        if pre_state.is_null() {
            (false, 0)
        } else {
            let probe = read_u32(pre_state, STATE_NATIVE_PUBLISH_BOUNDARY_PROBE);
            let active = probe & 0xff01_0000
                == (NATIVE_PUBLISH_BOUNDARY_PROBE_MAGIC | (1 << 16));
            let mut flags = (probe & 0x3f) | (1 << 6);
            let token = read_u32(pre_state, STATE_REFRESH_TOKEN);
            if token & ARM_TOKEN_MAGIC_MASK == REFRESH_TOKEN_MAGIC
                && token & REFRESH_TOKEN_STEREO != 0
            {
                flags |= 1 << 7;
            }
            if read_u8(pre_state, STATE_WRITER_PHASE) == WRITER_PHASE_COMMITTED_WAIT_UI {
                flags |= 1 << 8;
            }
            if read_u8(pre_state, STATE_WRITER_MODE) == WRITER_MODE_NATIVE_STEREO {
                flags |= 1 << 12;
            }
            (active, flags)
        }
    };
    // A slot only becomes playable in the running session once its bit is set in
    // the occupancy bitmap the UI queries through `0x0805985C` (`view + 0x30`).
    // Stock sets that bit from `0x08039FEE`, driven by the recording controller
    // reaching state 3, and the recording controller only ever knows about the
    // single slot it captured into - so the right slot stays "empty" until a reboot
    // rebuilds the model from storage. Publish it here, inside the right slot's own
    // Save completion and before the broadcast stock was already about to make, so
    // observers see the occupancy change and the slot change together.
    #[cfg(stereo_stock_save_queue)]
    if slot < 128 && armed_stereo_slot_side(slot as u8) == 2 {
        let armed_state = pair_state();
        if !armed_state.is_null() {
            let mut view = read_u32(armed_state, STATE_SAVE_VIEW_LATCH) as *mut u8;
            if view.is_null() {
                view = read_u32(armed_state, STATE_DEFERRED_REFRESH_VIEW) as *mut u8;
            }
            if !view.is_null() {
                publish_sample_view_occupancy(view, slot as u8);
            }
        }
    }
    let result = function::<StockPublish>(STOCK_PUBLISH)(observers, slot);
    #[cfg(native_publish_boundary_probe_diagnostic)]
    if boundary_active {
        boundary_flags |= 1 << 9;
        let post_state = pair_state();
        if !post_state.is_null() {
            let token = read_u32(post_state, STATE_REFRESH_TOKEN);
            if token & ARM_TOKEN_MAGIC_MASK == REFRESH_TOKEN_MAGIC
                && token & REFRESH_TOKEN_STEREO != 0
            {
                boundary_flags |= 1 << 10;
            }
            if read_u8(post_state, STATE_WRITER_PHASE) == WRITER_PHASE_COMMITTED_WAIT_UI {
                boundary_flags |= 1 << 11;
            }
            if read_u8(post_state, STATE_WRITER_MODE) == WRITER_MODE_NATIVE_STEREO {
                boundary_flags |= 1 << 13;
            }
            let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
            if !store.is_null() {
                boundary_flags |= 1 << 14;
                if token & ARM_TOKEN_MAGIC_MASK == REFRESH_TOKEN_MAGIC
                    && ((token >> 8) & 0x1f) as u8 == read_u8(store, 0)
                {
                    boundary_flags |= 1 << 15;
                }
            }
            write_u32(
                post_state,
                STATE_NATIVE_PUBLISH_BOUNDARY_PROBE,
                NATIVE_PUBLISH_BOUNDARY_PROBE_MAGIC | (1 << 16) | boundary_flags,
            );
        }
    }
    let state = pair_state();
    let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
    if state.is_null() || store.is_null() {
        return result;
    }
    #[cfg(stereo_stock_save_queue)]
    {
        let published_slot = slot as u8;
        if let Some((token, carry_index)) = stereo_token_for_slot(published_slot) {
            let left = token as u8;
            if published_slot == left + 1 {
                let pack = ((token >> 8) & 0x1f) as u8;
                // Prefer the pack healed from the save descriptor, but fall back
                // to the pack the registry is actually keyed to. The old
                // fallback used byte 0 of the content store, which lags the
                // real pack; `registry_bind_active_pack` no longer consults it.
                let linked = registry_set_pair(state, pack, left) || {
                    let bound = registry_pack(state);
                    bound != pack && registry_set_pair(state, bound, left)
                };
                if linked {
                    match carry_index {
                        // Retire only the carried identity; the primary token
                        // belongs to a capture that is still running.
                        Some(index) => write_u32(state, STATE_SAVE_PAIR_CARRY + index * 4, 0),
                        None => {
                            write_u32(state, STATE_ARM_TOKEN, 0);
                            write_u32(state, STATE_CAPTURE_UI_TOKEN, 0);
                            write_u32(state, STATE_SAVE_PAIR_LATCH, 0);
                            write_u32(state, STATE_SAVE_VIEW_LATCH, 0);
                        }
                    }
                }
            }
        }
        return result;
    }
    #[cfg(native_publish_slot_probe_diagnostic)]
    {
        let probe = read_u32(state, STATE_NATIVE_PUBLISH_SLOT_PROBE);
        if probe & 0xff01_0000 == (NATIVE_PUBLISH_SLOT_PROBE_MAGIC | (1 << 16))
            && probe & (1 << 23) == 0
        {
            let token = read_u32(state, STATE_REFRESH_TOKEN);
            let token_valid = token & ARM_TOKEN_MAGIC_MASK == REFRESH_TOKEN_MAGIC
                && token & REFRESH_TOKEN_STEREO != 0;
            let pack_matches =
                token_valid && ((token >> 8) & 0x1f) as u8 == read_u8(store, 0);
            write_u32(
                state,
                STATE_NATIVE_PUBLISH_SLOT_PROBE,
                NATIVE_PUBLISH_SLOT_PROBE_MAGIC
                    | (1 << 23)
                    | (1 << 16)
                    | (slot & 0x7f)
                    | (((token as u8 & 0x7f) as u32) << 7)
                    | ((token_valid as u32) << 14)
                    | ((pack_matches as u32) << 15),
            );
        }
    }
    #[cfg(native_publication_probe_diagnostic)]
    native_publication_probe_or(state, 1 << 2);
    let store_pack = read_u8(store, 0);
    let published_slot = slot as u8;
    let phase = read_u8(state, STATE_WRITER_PHASE);
    let token = read_u32(state, STATE_REFRESH_TOKEN);
    if token & ARM_TOKEN_MAGIC_MASK == REFRESH_TOKEN_MAGIC {
        let pack = ((token >> 8) & 0x1f) as u8;
        let left = token as u8;
        let stereo = token & REFRESH_TOKEN_STEREO != 0 && left < 127;
        if store_pack == pack
            && (published_slot == left || (stereo && published_slot == left + 1))
        {
            if stereo {
                #[cfg(native_publication_probe_diagnostic)]
                {
                #[cfg(native_publication_probe_diagnostic)]
                if published_slot == left {
                    native_publication_probe_or(state, 1 << 3);
                } else {
                    native_publication_probe_or(state, 1 << 11);
                }
                if phase == WRITER_PHASE_COMMITTED_WAIT_UI && published_slot == left {
                    #[cfg(native_publication_probe_diagnostic)]
                    native_publication_probe_or(state, (1 << 4) | (1 << 5));
                    let queued = queue_stereo_right_publication(left + 1);
                    #[cfg(native_publication_probe_diagnostic)]
                    if queued {
                        native_publication_probe_or(state, 1 << 6);
                    }
                    if queued {
                        write_u8(
                            state,
                            STATE_WRITER_PHASE,
                            WRITER_PHASE_RIGHT_PUBLICATION_PENDING,
                        );
                        #[cfg(native_publication_probe_diagnostic)]
                        native_publication_probe_or(state, 1 << 7);
                        write_u8(state, STATE_WRITER_WAIT_TICKS, 0);
                        return result;
                    }
                    #[cfg(native_publication_probe_diagnostic)]
                    native_publication_probe_or(state, 1 << 15);
                    write_u32(state, STATE_REFRESH_TOKEN, 0);
                    release_committed_writer(state);
                } else if phase == WRITER_PHASE_RIGHT_PUBLICATION_ACTIVE
                    && published_slot == left + 1
                {
                    registry_set_pair(state, pack, left);
                    #[cfg(native_publication_probe_diagnostic)]
                    native_publication_probe_or(state, (1 << 12) | (1 << 13));
                    write_u32(state, STATE_REFRESH_TOKEN, 0);
                    release_committed_writer(state);
                    return result;
                }
                }
            } else {
                write_u32(state, STATE_REFRESH_TOKEN, 0);
                registry_clear_slot(state, pack, left);
                if phase == WRITER_PHASE_COMMITTED_WAIT_UI {
                    release_committed_writer(state);
                }
            }
        }
    }
    result
}

unsafe fn delete_destination(state: *mut u8, slot: u8) -> bool {
    type DeleteItem = unsafe extern "C" fn(u32, u32, u32, u32) -> u32;
    let descriptor = state.add(STATE_DESCRIPTOR);
    let store = read_volatile(ACTIVE_CONTENT_STORE);
    let filesystem = read_volatile(FILESYSTEM_INTERFACE);
    if store == 0
        || filesystem == 0
        || read_u8(store as *const u8, 0) != read_u8(descriptor, DESCRIPTOR_PACK)
    {
        return false;
    }

    type TypeToIndex = unsafe extern "C" fn(u32) -> u32;
    let category =
        function::<TypeToIndex>(CONTENT_TYPE_TO_INDEX)(read_u32(descriptor, DESCRIPTOR_TYPE));
    function::<DeleteItem>(CONTENT_STORE_DELETE_ITEM)(
        store,
        filesystem,
        category,
        slot as u32,
    ) != 0
}

unsafe fn rollback_closed_files(state: *mut u8) {
    #[cfg(abort_path_hang_diagnostic)]
    {
        write_u8(state, STATE_ASYNC, 0xfe);
        return;
    }

    #[cfg(not(abort_path_hang_diagnostic))]
    {
    #[cfg(skip_content_index_diagnostic)]
    {
        write_u8(state, STATE_ASYNC, 3);
        return;
    }

    #[cfg(not(skip_content_index_diagnostic))]
    {
    let slot = read_u32(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT) as u8;
    #[cfg(sample_pair_identity_poc)]
    clear_pair_identity(state, state.add(STATE_DESCRIPTOR), slot);
    match read_u32(state, STATE_MAGIC) {
        WRITER_MAGIC_LEFT if is_stereo_pair_start(slot) => {
            let _ = delete_destination(state, slot);
        }
        WRITER_MAGIC_RIGHT_PENDING
            if stereo_left_for_right(slot).is_some() =>
        {
            let _ = delete_destination(state, slot - 1);
        }
        WRITER_MAGIC_RIGHT => {
            let left_slot = slot.wrapping_sub(1);
            if is_stereo_pair_start(left_slot) {
                let _ = delete_destination(state, left_slot);
                let _ = delete_destination(state, slot);
            }
        }
        _ => {}
    }
    write_u32(state, STATE_MAGIC, 0);
    write_u8(state, STATE_WRITER_MODE, WRITER_MODE_IDLE);
    write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
    write_u8(state, STATE_ASYNC, 3);
    #[cfg(sample_pair_identity_poc)]
    if read_u8(state, STATE_CLEAR_PHASE) == CLEAR_PHASE_DEFERRED {
        resume_deferred_clear(state);
    }
    }
    }
}

unsafe fn defer_left_rollback(state: *mut u8) {
    write_u32(state, STATE_MAGIC, WRITER_MAGIC_LEFT_ROLLBACK_PENDING);
    write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_ROLLBACK);
    write_u8(state, STATE_ASYNC, 1);
}

#[cfg(not(stereo_stock_save_queue))]
unsafe fn close_before_rollback(state: *mut u8) {
    #[cfg(abort_path_hang_diagnostic)]
    {
        write_u8(state, STATE_ASYNC, 0xfe);
        return;
    }

    #[cfg(not(abort_path_hang_diagnostic))]
    {
    write_u8(state, STATE_ASYNC, 9);
    function::<AsyncClose>(ASYNC_CLOSE)(
        state.add(STATE_DESCRIPTOR),
        stereo_file_callback as *const () as usize,
        state,
    );
    }
}

#[cfg(not(stereo_stock_save_queue))]
unsafe fn fail_data_path(state: *mut u8) {
    #[cfg(data_path_abort_passthrough_diagnostic)]
    {
        write_u8(state, STATE_ASYNC, 3);
    }

    #[cfg(not(data_path_abort_passthrough_diagnostic))]
    close_before_rollback(state);
}

#[cfg(not(stereo_stock_save_queue))]
unsafe fn fail_data_callback(state: *mut u8) {
    #[cfg(any(
        data_path_abort_passthrough_diagnostic,
        data_callback_abort_passthrough_diagnostic
    ))]
    {
        write_u8(state, STATE_ASYNC, 3);
    }

    #[cfg(not(any(
        data_path_abort_passthrough_diagnostic,
        data_callback_abort_passthrough_diagnostic
    )))]
    close_before_rollback(state);
}

// Stock-save-queue retention: the custom dual-WAV writer exists only to serve
// the superseded interleaved capture architecture. Native dual capture already
// leaves native-rate mono PCM in both DSP slots, so each channel is written by
// the unmodified stock writer under its own stock save transaction. The writer
// is never claimed, so this variant carries only the hand-off, which lets the
// custom pump, file callback, open callback, terminal publication, refresh
// token, deferred refresh and watchdog drop out of the image entirely.
#[cfg(stereo_stock_save_queue)]
#[no_mangle]
pub unsafe extern "C" fn stereo_start_request(
    manager: *mut u8,
    capture_slot: u32,
    descriptor: *const u8,
    completion: usize,
    completion_context: u32,
) -> u32 {
    if !manager.is_null() {
        let state = read_u32(manager, 0x04) as *mut u8;
        if !state.is_null() {
            write_u8(state, STATE_WRITER_MODE, WRITER_MODE_STOCK);
            write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
        }
    }
    stock_start_request_trampoline(
        manager,
        capture_slot,
        descriptor,
        completion,
        completion_context,
    )
}

#[cfg(not(stereo_stock_save_queue))]
#[no_mangle]
#[cfg_attr(
    occupancy_probe_diagnostic,
    allow(unreachable_code, unused_variables, unused_assignments)
)]
pub unsafe extern "C" fn stereo_start_request(
    manager: *mut u8,
    capture_slot: u32,
    descriptor: *const u8,
    completion: usize,
    completion_context: u32,
) -> u32 {
    #[cfg(reject_recording_diagnostic)]
    if capture_slot < 0x80 {
        return 0;
    }

    let source = if manager.is_null() {
        core::ptr::null_mut()
    } else {
        source_from_manager(manager)
    };
    let state = if manager.is_null() {
        core::ptr::null_mut()
    } else {
        read_u32(manager, 0x04) as *mut u8
    };
    #[cfg(native_dual_capture_poc)]
    let capture_ui_token = if state.is_null() {
        0
    } else {
        read_u32(state, STATE_CAPTURE_UI_TOKEN)
    };
    let source_active = !source.is_null() && read_u8(source, 4) != 0;
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    let mut acquisition_probe = NATIVE_WRITER_ACQUISITION_PROBE_MAGIC | 1;
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    {
        acquisition_probe |= ((!source.is_null() as u32) << 1)
            | ((source_active as u32) << 2)
            | ((!state.is_null() as u32) << 3);
        if !state.is_null() {
            write_u32(
                state,
                STATE_NATIVE_WRITER_ACQUISITION_PROBE,
                acquisition_probe,
            );
        }
    }
    #[cfg(native_writer_readiness_probe_diagnostic)]
    if !state.is_null() {
        write_u32(
            state,
            STATE_NATIVE_WRITER_PROBE,
            NATIVE_WRITER_PROBE_MAGIC | 1,
        );
    }
    #[cfg(all(sample_cache_refresh_poc, native_publication_probe_diagnostic))]
    if !state.is_null()
        && !descriptor.is_null()
        && read_u8(state, STATE_WRITER_PHASE) == WRITER_PHASE_RIGHT_PUBLICATION_PENDING
    {
        #[cfg(native_publication_probe_diagnostic)]
        native_publication_probe_or(state, 1 << 8);
        let token = read_u32(state, STATE_REFRESH_TOKEN);
        let pack = ((token >> 8) & 0x1f) as u8;
        let left = token as u8;
        let right = left.wrapping_add(1);
        let store = read_volatile(ACTIVE_CONTENT_STORE) as *const u8;
        let canonical_pack = if store.is_null() {
            0xff
        } else {
            read_u8(store, 0)
        };
        if token & ARM_TOKEN_MAGIC_MASK == REFRESH_TOKEN_MAGIC
            && token & REFRESH_TOKEN_STEREO != 0
            && is_stereo_pair_start(left)
            && canonical_pack < 0x20
            && pack == canonical_pack
            && capture_slot as u8 == right
            && read_u32(descriptor, DESCRIPTOR_TYPE) == 4
            && read_u32(descriptor, DESCRIPTOR_SLOT) as u8 == right
        {
            #[cfg(native_publication_probe_diagnostic)]
            native_publication_probe_or(state, 1 << 9);
            let mut descriptor_byte = 0;
            while descriptor_byte < 9 {
                write_u8(
                    state,
                    STATE_DESCRIPTOR + descriptor_byte,
                    read_u8(descriptor, descriptor_byte),
                );
                descriptor_byte += 1;
            }
            write_u32(
                state,
                STATE_DESCRIPTOR + DESCRIPTOR_PACK,
                canonical_pack as u32,
            );
            write_u32(state, STATE_COMPLETION, completion as u32);
            write_u32(state, STATE_COMPLETION_CONTEXT, completion_context);
            write_u32(state, STATE_TOTAL_FRAMES, 0);
            write_u32(state, STATE_WRITTEN_FRAMES, 0);
            write_u16(state, STATE_CAPTURE_SLOT, right as u16);
            write_u32(state, STATE_BUFFERED_FRAMES, 0);
            write_u8(state, STATE_BUFFER_OFFSET, 0);
            write_u32(state, STATE_BUFFER, 0);
            write_u32(state, STATE_MAGIC, 0);
            write_u8(state, STATE_WRITER_MODE, WRITER_MODE_CUSTOM_STEREO);
            write_u8(
                state,
                STATE_WRITER_PHASE,
                WRITER_PHASE_RIGHT_PUBLICATION_ACTIVE,
            );
            #[cfg(native_publication_probe_diagnostic)]
            native_publication_probe_or(state, 1 << 10);
            write_u8(state, STATE_WRITER_WAIT_TICKS, 0);
            write_u8(state, STATE_ASYNC, 7);
            return 1;
        }
        write_u8(state, STATE_WRITER_MODE, WRITER_MODE_STOCK);
        return stock_start_request_trampoline(
            manager,
            capture_slot,
            descriptor,
            completion,
            completion_context,
        );
    }
    if !source_active {
        #[cfg(sample_pair_identity_poc)]
        if !source.is_null()
            && !state.is_null()
            && !descriptor.is_null()
            && read_u32(descriptor, DESCRIPTOR_TYPE) == 4
        {
            let pack = read_u8(descriptor, DESCRIPTOR_PACK);
            let slot = read_u32(descriptor, DESCRIPTOR_SLOT) as u8;
            if identified_pair_left(state, pack, slot).is_some() {
                return 0;
            }
        }
        if !state.is_null() {
            write_u8(state, STATE_WRITER_MODE, WRITER_MODE_STOCK);
            write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
        }
        return stock_start_request_trampoline(
            manager,
            capture_slot,
            descriptor,
            completion,
            completion_context,
        );
    }

    if state.is_null() {
        return 0;
    }
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    if read_u32(state, 0x14) != u32::MAX {
        write_u32(
            state,
            STATE_NATIVE_WRITER_ACQUISITION_PROBE,
            acquisition_probe,
        );
        return 0;
    }
    #[cfg(not(native_writer_acquisition_probe_diagnostic))]
    if read_u32(state, 0x14) != u32::MAX {
        return 0;
    }
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    {
        acquisition_probe |= 1 << 4;
    }
    if read_u8(state, 0x18) != 0xff {
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_WRITER_ACQUISITION_PROBE,
            acquisition_probe,
        );
        return 0;
    }
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    {
        acquisition_probe |= 1 << 5;
    }
    if read_u32(state, 0x10) != u32::MAX {
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_WRITER_ACQUISITION_PROBE,
            acquisition_probe,
        );
        return 0;
    }
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    {
        acquisition_probe |= 1 << 6;
    }
    if capture_slot >= 0x80 {
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_WRITER_ACQUISITION_PROBE,
            acquisition_probe,
        );
        return 0;
    }
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    {
        acquisition_probe |= 1 << 7;
    }
    if descriptor.is_null() {
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_WRITER_ACQUISITION_PROBE,
            acquisition_probe,
        );
        return 0;
    }
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    {
        acquisition_probe |= 1 << 8;
    }
    if read_u32(descriptor, DESCRIPTOR_TYPE) != 4 {
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_WRITER_ACQUISITION_PROBE,
            acquisition_probe,
        );
        return 0;
    }
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    {
        acquisition_probe |= 1 << 9;
    }
    if read_u32(descriptor, DESCRIPTOR_SLOT) >= 0x80 {
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_WRITER_ACQUISITION_PROBE,
            acquisition_probe,
        );
        return 0;
    }
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    {
        acquisition_probe |= 1 << 10;
    }
    if read_u8(descriptor, DESCRIPTOR_PACK) >= 0x20 {
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        write_u32(
            state,
            STATE_NATIVE_WRITER_ACQUISITION_PROBE,
            acquisition_probe,
        );
        return 0;
    }
    #[cfg(native_writer_acquisition_probe_diagnostic)]
    {
        acquisition_probe |= 1 << 11;
    }

    let left_slot = read_u32(descriptor, DESCRIPTOR_SLOT) as u8;
    #[cfg(native_dual_capture_poc)]
    let mut canonical_pack = read_u8(descriptor, DESCRIPTOR_PACK);
    #[cfg(sample_pair_gesture_poc)]
    let _ = left_slot;
    #[cfg(occupancy_probe_diagnostic)]
    {
        let _ = destination_is_empty(descriptor, left_slot);
        return 0;
    }

    #[cfg(all(
        not(force_stereo_diagnostic),
        not(sample_pair_gesture_poc),
        not(sample_pair_writer_coupling_poc)
    ))]
    let stereo = is_stereo_pair_start(left_slot);
    #[cfg(all(
        not(force_stereo_diagnostic),
        sample_pair_gesture_poc,
        not(sample_pair_writer_coupling_poc)
    ))]
    let stereo = false;
    #[cfg(all(
        not(force_stereo_diagnostic),
        sample_pair_writer_coupling_poc,
        not(native_dual_capture_poc),
        not(any(
            native_arm_token_probe_diagnostic,
            native_pack_value_probe_diagnostic
        ))
    ))]
    let stereo = arm_token_matches(state, descriptor, left_slot);
    #[cfg(all(
        not(force_stereo_diagnostic),
        sample_pair_writer_coupling_poc,
        native_arm_token_probe_diagnostic
    ))]
    let stereo = arm_token_probe(
        state,
        descriptor,
        left_slot,
        capture_slot as u8,
        capture_ui_token,
    );
    #[cfg(all(
        not(force_stereo_diagnostic),
        sample_pair_writer_coupling_poc,
        native_pack_value_probe_diagnostic
    ))]
    let stereo = pack_value_probe(state, descriptor, left_slot);
    #[cfg(all(
        not(force_stereo_diagnostic),
        native_dual_capture_poc,
        sample_pair_writer_coupling_poc,
        not(any(
            native_arm_token_probe_diagnostic,
            native_pack_value_probe_diagnostic
        ))
    ))]
    let stereo = match native_stereo_authenticated_pack(
        state,
        left_slot,
        capture_slot as u8,
        capture_ui_token,
    ) {
        Some(pack) => {
            canonical_pack = pack;
            true
        }
        None => false,
    };
    #[cfg(native_dual_capture_poc)]
    {
        let armed =
            read_u32(state, STATE_ARM_TOKEN) & ARM_TOKEN_MAGIC_MASK == ARM_TOKEN_MAGIC;
        if !armed {
            #[cfg(native_writer_acquisition_probe_diagnostic)]
            write_u32(
                state,
                STATE_NATIVE_WRITER_ACQUISITION_PROBE,
                acquisition_probe,
            );
            write_u8(state, STATE_WRITER_MODE, WRITER_MODE_STOCK);
            write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
            return stock_start_request_trampoline(
                manager,
                capture_slot,
                descriptor,
                completion,
                completion_context,
            );
        }
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        {
            acquisition_probe |= 1 << 12;
            acquisition_probe |= (stereo as u32) << 13;
            acquisition_probe |= ((capture_slot as u8 == left_slot) as u32) << 14;
        }
        if !stereo || capture_slot as u8 != left_slot {
            #[cfg(native_writer_acquisition_probe_diagnostic)]
            write_u32(
                state,
                STATE_NATIVE_WRITER_ACQUISITION_PROBE,
                acquisition_probe,
            );
            return 0;
        }
    }
    #[cfg(all(sample_pair_writer_coupling_poc, not(native_dual_capture_poc)))]
    clear_arm_token(state);
    #[cfg(force_stereo_diagnostic)]
    let stereo = left_slot < 127;
    #[cfg(all(
        not(skip_content_index_diagnostic),
        not(safe_pair_guard)
    ))]
    if stereo
        && (!destination_is_empty(descriptor, left_slot)
            || !destination_is_empty(descriptor, left_slot + 1))
    {
        return 0;
    }

    write_u8(state, STATE_ASYNC, 0);
    let mut descriptor_byte = 0;
    while descriptor_byte < 9 {
        write_u8(
            state,
            STATE_DESCRIPTOR + descriptor_byte,
            read_u8(descriptor, descriptor_byte),
        );
        descriptor_byte += 1;
    }
    #[cfg(native_dual_capture_poc)]
    if stereo {
        write_u32(
            state,
            STATE_DESCRIPTOR + DESCRIPTOR_PACK,
            canonical_pack as u32,
        );
    }
    write_u32(state, STATE_COMPLETION, completion as u32);
    write_u32(state, STATE_COMPLETION_CONTEXT, completion_context);
    write_u32(state, STATE_TOTAL_FRAMES, 0);
    write_u32(state, STATE_WRITTEN_FRAMES, 0);
    write_volatile(
        state.add(STATE_CAPTURE_SLOT).cast::<u16>(),
        capture_slot as u16,
    );
    write_u32(state, STATE_BUFFERED_FRAMES, 0);
    write_u8(state, STATE_BUFFER_OFFSET, 0);
    write_u32(state, STATE_BUFFER, 0);
    #[cfg(synthetic_commit_probe_diagnostic)]
    initialize_extension(state, false);
    #[cfg(not(synthetic_commit_probe_diagnostic))]
    initialize_extension(state, stereo);
    write_u8(
        state,
        STATE_WRITER_MODE,
        if stereo {
            #[cfg(native_dual_capture_poc)]
            {
                WRITER_MODE_NATIVE_STEREO
            }
            #[cfg(not(native_dual_capture_poc))]
            {
            WRITER_MODE_CUSTOM_STEREO
            }
        } else {
            WRITER_MODE_CUSTOM_MONO
        },
    );
    write_u8(
        state,
        STATE_WRITER_PHASE,
        if stereo {
            #[cfg(native_dual_capture_poc)]
            {
                WRITER_PHASE_WAIT_NATIVE_NORMALIZATION
            }
            #[cfg(not(native_dual_capture_poc))]
            {
            WRITER_PHASE_PREPARE_LEFT
            }
        } else {
            WRITER_PHASE_WRITE_LEFT
        },
    );

    #[cfg(native_dual_capture_poc)]
    {
        write_u8(state, STATE_ASYNC, 1);
        if stereo {
            #[cfg(native_pack_correction_probe_diagnostic)]
            native_pack_correction_probe_or(state, (1 << 0) | (1 << 1));
            #[cfg(native_finalization_probe_diagnostic)]
            write_u32(
                state,
                STATE_NATIVE_FINALIZATION_PROBE,
                NATIVE_FINALIZATION_PROBE_MAGIC | (1 << 16),
            );
            #[cfg(native_publication_probe_diagnostic)]
            write_u32(
                state,
                STATE_NATIVE_PUBLICATION_PROBE,
                NATIVE_PUBLICATION_PROBE_MAGIC | (1 << 16),
            );
            #[cfg(native_terminal_publish_probe_diagnostic)]
            write_u32(
                state,
                STATE_NATIVE_TERMINAL_PUBLISH_PROBE,
                NATIVE_TERMINAL_PUBLISH_PROBE_MAGIC | (1 << 16),
            );
            #[cfg(native_publish_boundary_probe_diagnostic)]
            write_u32(
                state,
                STATE_NATIVE_PUBLISH_BOUNDARY_PROBE,
                NATIVE_PUBLISH_BOUNDARY_PROBE_MAGIC | (1 << 16),
            );
            #[cfg(native_publish_slot_probe_diagnostic)]
            write_u32(
                state,
                STATE_NATIVE_PUBLISH_SLOT_PROBE,
                NATIVE_PUBLISH_SLOT_PROBE_MAGIC | (1 << 16),
            );
            clear_arm_token(state);
            write_u32(state, STATE_CAPTURE_UI_TOKEN, 0);
        }
        #[cfg(native_writer_readiness_probe_diagnostic)]
        if stereo {
            write_u32(
                state,
                STATE_NATIVE_WRITER_PROBE,
                NATIVE_WRITER_PROBE_MAGIC | 3,
            );
        }
        #[cfg(native_writer_acquisition_probe_diagnostic)]
        if stereo {
            write_u32(
                state,
                STATE_NATIVE_WRITER_ACQUISITION_PROBE,
                acquisition_probe | (1 << 15),
            );
        }
        #[cfg(native_arm_token_probe_diagnostic)]
        if stereo {
            write_u32(
                state,
                STATE_NATIVE_ARM_TOKEN_PROBE,
                read_u32(state, STATE_NATIVE_ARM_TOKEN_PROBE) | (1 << 15),
            );
        }
        return 1;
    }

    #[cfg(not(native_dual_capture_poc))]
    {
    #[cfg(accepted_idle_probe_diagnostic)]
    return 1;

    function::<AsyncOpen>(ASYNC_OPEN)(
        state.add(STATE_DESCRIPTOR),
        stereo_open_callback as *const () as usize,
        state,
    );
    1
    }
}

#[cfg(not(stereo_stock_save_queue))]
#[no_mangle]
pub unsafe extern "C" fn stereo_open_callback(
    _arg1: u32,
    _arg2: u32,
    status: u32,
    _arg4: u32,
    _arg5: u32,
    state: *mut u8,
) {
    #[cfg(native_pack_correction_probe_diagnostic)]
    if !state.is_null() && read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_NATIVE_STEREO {
        native_pack_correction_probe_clear(state, 1 << 10);
        if status == 5 {
            native_pack_correction_probe_or(state, 1 << 10);
        }
    }
    #[cfg(open_close_probe_diagnostic)]
    {
        if status != 0 {
            write_u8(state, STATE_ASYNC, 3);
            return;
        }
        write_u8(state, STATE_ASYNC, 6);
        function::<AsyncClose>(ASYNC_CLOSE)(
            state.add(STATE_DESCRIPTOR),
            probe_close_callback as *const () as usize,
            state,
        );
        return;
    }

    #[cfg(not(open_close_probe_diagnostic))]
    {
        #[cfg(safe_pair_guard)]
        if status == PROTECTED_OCCUPIED_STATUS {
            if read_u32(state, STATE_MAGIC) == WRITER_MAGIC_RIGHT_PENDING {
                defer_left_rollback(state);
            } else {
                write_u32(state, STATE_MAGIC, 0);
                write_u8(state, STATE_WRITER_MODE, WRITER_MODE_IDLE);
                write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
                write_u8(state, STATE_ASYNC, 3);
                #[cfg(sample_pair_identity_poc)]
                if read_u8(state, STATE_CLEAR_PHASE) == CLEAR_PHASE_DEFERRED {
                    resume_deferred_clear(state);
                }
            }
            return;
        }
        if read_u32(state, STATE_MAGIC) == WRITER_MAGIC_RIGHT_PENDING {
            if status != 5 {
                defer_left_rollback(state);
                return;
            }
            write_u32(state, STATE_MAGIC, WRITER_MAGIC_RIGHT);
            #[cfg(native_dual_capture_poc)]
            write_u8(
                state,
                STATE_WRITER_PHASE,
                WRITER_PHASE_WRITE_NATIVE_RIGHT,
            );
            #[cfg(not(native_dual_capture_poc))]
            write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_WRITE_RIGHT);
        }
        if writer_magic_is_valid(state) {
            if read_u32(state, STATE_MAGIC) == WRITER_MAGIC_LEFT {
                #[cfg(native_dual_capture_poc)]
                write_u8(
                    state,
                    STATE_WRITER_PHASE,
                    WRITER_PHASE_WRITE_NATIVE_LEFT,
                );
                #[cfg(not(native_dual_capture_poc))]
                write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_WRITE_LEFT);
            }
            write_u8(state, STATE_ASYNC, 1);
            #[cfg(native_pack_correction_probe_diagnostic)]
            if read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_NATIVE_STEREO {
                native_pack_correction_probe_or(state, 1 << 11);
            }
        } else {
            rollback_closed_files(state);
        }
    }
}

#[no_mangle]
#[cfg(any(open_close_probe_diagnostic, pump_close_probe_diagnostic))]
pub unsafe extern "C" fn probe_close_callback(
    _arg1: u32,
    _arg2: u32,
    status: u32,
    _arg4: u32,
    _arg5: u32,
    state: *mut u8,
) {
    write_u8(state, STATE_ASYNC, if status == 0 { 7 } else { 3 });
}

#[no_mangle]
#[cfg(any(
    hang_after_first_close_diagnostic,
    data_callback_close_probe_diagnostic,
    all_data_callback_close_probe_diagnostic
))]
pub unsafe extern "C" fn probe_hang_close_callback(
    _arg1: u32,
    _arg2: u32,
    _status: u32,
    _arg4: u32,
    _arg5: u32,
    state: *mut u8,
) {
    write_u8(state, STATE_ASYNC, 0xfe);
}

// Stock-save-queue pass-through. The full pump below exists only to drive the
// custom dual-WAV writer, which is never claimed under the stock save queue, so
// every call took its pass-through path. That path is reproduced exactly here -
// including forwarding zero for the third and fourth arguments, which
// Sampling_PumpCaptureToFile hands on to the capture-source read method. Keeping
// the hook installed with these zeros preserves the stock writer's observed
// behaviour; dropping the hook entirely would let stock's own values through
// and changes how the capture source is read.
#[cfg(stereo_stock_save_queue)]
#[no_mangle]
pub unsafe extern "C" fn stereo_pump(manager: *mut u8, state: *mut u8) -> u32 {
    stock_pump_trampoline(manager, state, 0, 0)
}

#[cfg(not(stereo_stock_save_queue))]
#[no_mangle]
#[cfg_attr(
    any(
        accepted_idle_probe_diagnostic,
        open_close_probe_diagnostic,
        pump_close_probe_diagnostic
    ),
    allow(unreachable_code, unused_variables)
)]
pub unsafe extern "C" fn stereo_pump(manager: *mut u8, state: *mut u8) -> u32 {
    if !state.is_null()
        && read_u8(state, STATE_WRITER_PHASE) == WRITER_PHASE_COMMITTED_WAIT_UI
    {
        let ticks = read_u8(state, STATE_WRITER_WAIT_TICKS).saturating_add(1);
        write_u8(state, STATE_WRITER_WAIT_TICKS, ticks);
        #[cfg(sample_pair_identity_poc)]
        if ticks == u8::MAX {
            #[cfg(native_publication_probe_diagnostic)]
            native_publication_probe_or(state, 1 << 14);
            write_u32(state, STATE_REFRESH_TOKEN, 0);
            release_committed_writer(state);
        }
        return stock_pump_trampoline(manager, state, 0, 0);
    }
    if state.is_null()
        || !matches!(
            read_u8(state, STATE_WRITER_MODE),
            WRITER_MODE_CUSTOM_MONO | WRITER_MODE_CUSTOM_STEREO
        )
    {
        return stock_pump_trampoline(manager, state, 0, 0);
    }

    #[cfg(any(accepted_idle_probe_diagnostic, open_close_probe_diagnostic))]
    return 0;

    #[cfg(pump_close_probe_diagnostic)]
    {
        if read_u8(state, STATE_ASYNC) == 1 {
            write_u8(state, STATE_ASYNC, 6);
            function::<AsyncClose>(ASYNC_CLOSE)(
                state.add(STATE_DESCRIPTOR),
                probe_close_callback as *const () as usize,
                state,
            );
        }
        return 0;
    }

    #[cfg(synthetic_commit_probe_diagnostic)]
    {
        if read_u8(state, STATE_ASYNC) == 1 {
            let written = read_u32(state, STATE_WRITTEN_FRAMES);
            write_u32(state, STATE_TOTAL_FRAMES, SYNTHETIC_TOTAL_SAMPLES);
            if written >= SYNTHETIC_TOTAL_SAMPLES {
                write_u8(state, STATE_ASYNC, 4);
                return 1;
            }
            write_u32(state, STATE_BUFFERED_FRAMES, SYNTHETIC_CHUNK_SAMPLES);
            write_u8(state, STATE_ASYNC, 2);
            function::<AsyncWrite>(ASYNC_WRITE)(
                state.add(STATE_DESCRIPTOR),
                written * 2 + 0x2c,
                state.add(STATE_SYNTHETIC_BUFFER).cast::<i16>(),
                SYNTHETIC_CHUNK_SAMPLES * 2,
                stereo_file_callback as *const () as usize,
                state,
            );
        }
        return 1;
    }

    if read_u32(state, STATE_MAGIC) == WRITER_MAGIC_LEFT_ROLLBACK_PENDING {
        let right_slot = read_u32(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT) as u8;
        if stereo_left_for_right(right_slot).is_some() {
            let _ = delete_destination(state, right_slot - 1);
        }
        write_u32(state, STATE_MAGIC, 0);
        write_u8(state, STATE_WRITER_MODE, WRITER_MODE_IDLE);
        write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
        write_u8(state, STATE_ASYNC, 3);
        #[cfg(sample_pair_identity_poc)]
        if read_u8(state, STATE_CLEAR_PHASE) == CLEAR_PHASE_DEFERRED {
            resume_deferred_clear(state);
        }
        return 0;
    }

    if !writer_magic_is_valid(state) {
        write_u8(state, STATE_WRITER_MODE, WRITER_MODE_IDLE);
        write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
        write_u8(state, STATE_WRITER_WAIT_TICKS, 0);
        write_u8(state, STATE_ASYNC, 3);
        #[cfg(sample_pair_identity_poc)]
        if read_u8(state, STATE_CLEAR_PHASE) == CLEAR_PHASE_DEFERRED {
            resume_deferred_clear(state);
        }
        return 0;
    }

    #[cfg(native_dual_capture_poc)]
    if read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_NATIVE_STEREO {
        return native_stereo_pump(manager, state);
    }

    let source = source_from_manager(manager);
    let slot = read_u16(state, STATE_CAPTURE_SLOT);
    let source_ended_before_total =
        source_method::<SourceStatus>(source, 0x44)(source, slot) == 0;
    let mut raw_total = source_method::<SourceTotal>(source, 0)(source, slot);
    let source_ended = source_ended_before_total
        && source_method::<SourceStatus>(source, 0x44)(source, slot) == 0;
    if raw_total == 0 {
        return 0;
    }
    if source_ended && raw_total & 1 != 0 {
        #[cfg(terminal_odd_word_clamp_diagnostic)]
        {
            raw_total &= !1;
        }
        #[cfg(not(terminal_odd_word_clamp_diagnostic))]
        {
        close_before_rollback(state);
        return 0;
        }
    }
    let total_frames = raw_total / 2;
    write_u32(state, STATE_TOTAL_FRAMES, total_frames);
    let written_frames = read_u32(state, STATE_WRITTEN_FRAMES);
    let mut buffered_frames = read_u32(state, STATE_BUFFERED_FRAMES);
    let retained_bytes = read_u8(state, STATE_BUFFER_OFFSET) as u32;
    if written_frames >= total_frames && buffered_frames == 0 && retained_bytes == 0 {
        if source_ended {
            write_u8(state, STATE_ASYNC, 4);
            return 1;
        }
        return 0;
    }

    if source_method::<SourceStatus>(source, 0x34)(source, slot) != 0 {
        return 1;
    }
    let available_words = source_method::<SourceTotal>(source, 0x38)(source, slot) & !1;
    let mut buffer = read_u32(state, STATE_BUFFER) as *mut i16;
    if buffer.is_null() {
        buffer = source_method::<SourceBuffer>(source, 0x4c)(source, slot);
        write_u32(state, STATE_BUFFER, buffer as u32);
    }
    if buffer.is_null() {
        return 0;
    }

    if retained_bytes != 0 && buffered_frames == 0 {
        let retained_frames = retained_bytes / 2;
        let source_offset = OUTPUT_BLOCK_SPAN - retained_frames;
        let mut frame = 0;
        while frame < retained_frames {
            let value = read_volatile(buffer.add((source_offset + frame) as usize));
            write_volatile(buffer.add(frame as usize), value);
            frame += 1;
        }
        buffered_frames = retained_frames;
        write_u32(state, STATE_BUFFERED_FRAMES, buffered_frames);
        write_u8(state, STATE_BUFFER_OFFSET, 0);
    }

    let raw_cursor = (written_frames + buffered_frames) * 2;
    if available_words > raw_cursor && buffered_frames < OUTPUT_BLOCK_SPAN {
        let raw_capacity = (OUTPUT_BLOCK_SPAN - buffered_frames) * 2;
        let raw_words = core::cmp::min(
            core::cmp::min(available_words - raw_cursor, raw_capacity),
            RAW_READ_MAX_WORDS,
        ) & !1;
        #[cfg(source_read_retry_diagnostic)]
        if raw_words == 0 {
            return 1;
        }
        let raw_buffer = buffer.add(RAW_SCRATCH_WORD_OFFSET);
        let words_read = source_method::<SourceRead>(source, 0x18)(
            source,
            slot,
            raw_cursor,
            raw_buffer,
            raw_words,
        );
        #[cfg(source_read_retry_diagnostic)]
        let usable_words = core::cmp::min(words_read, raw_words) & !1;
        #[cfg(source_read_retry_diagnostic)]
        if usable_words == 0 {
            return 1;
        }
        #[cfg(not(source_read_retry_diagnostic))]
        if words_read == 0 || words_read & 1 != 0 {
            source_method::<SourceBuffer>(source, 0x50)(source, slot);
            write_u32(state, STATE_BUFFER, 0);
            close_before_rollback(state);
            return 0;
        }
        #[cfg(not(source_read_retry_diagnostic))]
        let usable_words = words_read;
        let transform = match read_u32(state, STATE_MAGIC) {
            WRITER_MAGIC_LEFT => TRANSFORM_LEFT,
            WRITER_MAGIC_RIGHT => TRANSFORM_RIGHT,
            _ => TRANSFORM_MONO,
        };
        let produced_frames = capture_transform_to(
            raw_buffer,
            buffer.add(buffered_frames as usize),
            usable_words,
            transform,
        );
        if produced_frames <= 0 {
            source_method::<SourceBuffer>(source, 0x50)(source, slot);
            write_u32(state, STATE_BUFFER, 0);
            fail_data_path(state);
            return 0;
        }
        buffered_frames += produced_frames as u32;
        write_u32(state, STATE_BUFFERED_FRAMES, buffered_frames);
    }

    let remaining_frames = total_frames.saturating_sub(written_frames);
    let write_frames = if written_frames == 0 && buffered_frames >= OUTPUT_BLOCK_SPAN {
        core::cmp::min(remaining_frames, FIRST_OUTPUT_SPAN)
    } else if buffered_frames >= OUTPUT_BLOCK_SPAN {
        core::cmp::min(remaining_frames, OUTPUT_BLOCK_SPAN)
    } else if source_ended && written_frames + buffered_frames >= total_frames {
        core::cmp::min(remaining_frames, buffered_frames)
    } else {
        return 1;
    };
    if write_frames == 0 {
        return 1;
    }

    write_u8(state, STATE_ASYNC, 2);
    function::<AsyncWrite>(ASYNC_WRITE)(
        state.add(STATE_DESCRIPTOR),
        written_frames * 2 + 0x2c,
        buffer,
        write_frames * 2,
        stereo_file_callback as *const () as usize,
        state,
    );
    1
}

#[cfg(native_dual_capture_poc)]
#[inline(always)]
unsafe fn native_slot_extent(slot: u16, metadata_offset: u32) -> u32 {
    const DSP_READ: usize = 0x0803_14d7;

    function::<DspRead>(DSP_READ)(
        0,
        0x0003_0000 + slot as u32 * 4 + metadata_offset,
    ) >> 8
}

#[cfg(native_finalization_probe_diagnostic)]
#[inline(always)]
unsafe fn native_dsp_y_word(address: u32) -> u32 {
    const DSP_READ: usize = 0x0803_14d7;
    function::<DspRead>(DSP_READ)(0, address) >> 8
}

#[cfg(native_finalization_probe_diagnostic)]
unsafe fn update_native_finalization_probe(state: *mut u8, left: u16) {
    let current_slot = native_dsp_y_word(0x0003_71d5);
    let capture_state = native_dsp_y_word(0x0003_71d4);
    let cursor = native_dsp_y_word(0x0003_71d7);
    let left_logical = native_slot_extent(left, 1);
    let left_readable = native_slot_extent(left, 3);
    let right_logical = native_slot_extent(left + 1, 1);
    let right_readable = native_slot_extent(left + 1, 3);
    let flags = ((current_slot == left as u32) as u32)
        | (((current_slot == left as u32 + 1) as u32) << 1)
        | (((current_slot > 127) as u32) << 2)
        | ((((capture_state & 4) != 0) as u32) << 3)
        | (((cursor != 0) as u32) << 4)
        | (((left_logical != 0) as u32) << 5)
        | (((right_logical != 0) as u32) << 6)
        | (((left_logical != 0 && left_logical == right_logical) as u32) << 7)
        | (((left_logical != 0 && left_readable >= left_logical) as u32) << 8)
        | (((right_logical != 0 && right_readable >= right_logical) as u32) << 9);
    write_u32(
        state,
        STATE_NATIVE_FINALIZATION_PROBE,
        NATIVE_FINALIZATION_PROBE_MAGIC | (1 << 16) | flags,
    );
}

#[cfg(native_publication_probe_diagnostic)]
#[inline(always)]
unsafe fn native_publication_probe_or(state: *mut u8, flags: u32) {
    write_u32(
        state,
        STATE_NATIVE_PUBLICATION_PROBE,
        read_u32(state, STATE_NATIVE_PUBLICATION_PROBE) | flags,
    );
}

#[cfg(native_terminal_publish_probe_diagnostic)]
#[inline(always)]
unsafe fn native_terminal_publish_probe_or(state: *mut u8, flags: u32) {
    write_u32(
        state,
        STATE_NATIVE_TERMINAL_PUBLISH_PROBE,
        read_u32(state, STATE_NATIVE_TERMINAL_PUBLISH_PROBE) | flags,
    );
}

#[cfg(native_publish_boundary_probe_diagnostic)]
#[inline(always)]
unsafe fn native_publish_boundary_probe_or(state: *mut u8, flags: u32) {
    write_u32(
        state,
        STATE_NATIVE_PUBLISH_BOUNDARY_PROBE,
        read_u32(state, STATE_NATIVE_PUBLISH_BOUNDARY_PROBE) | flags,
    );
}

#[cfg(native_dual_capture_poc)]
#[cfg(native_writer_readiness_probe_diagnostic)]
unsafe fn native_writer_probe_base(state: *mut u8) -> u32 {
    let phase = read_u8(state, STATE_WRITER_PHASE);
    let async_state = read_u8(state, STATE_ASYNC);
    NATIVE_WRITER_PROBE_MAGIC
        | (read_u32(state, STATE_NATIVE_WRITER_PROBE) & 3)
        | (1 << 2)
        | ((matches!(
            phase,
            WRITER_PHASE_WRITE_NATIVE_LEFT | WRITER_PHASE_WRITE_NATIVE_RIGHT
        ) as u32)
            << 9)
        | (((phase as u32) & 0x0f) << 10)
        | (((async_state as u32) & 0x0f) << 14)
}

#[cfg(native_dual_capture_poc)]
unsafe fn native_pair_pcm_ready(
    source: *mut u8,
    left: u16,
    #[cfg(any(
        native_writer_readiness_probe_diagnostic,
        native_pack_correction_probe_diagnostic
    ))]
    state: *mut u8,
) -> Option<u32> {
    #[cfg(native_writer_readiness_probe_diagnostic)]
    let mut probe = native_writer_probe_base(state);
    if left >= 127 {
        #[cfg(native_writer_readiness_probe_diagnostic)]
        write_u32(state, STATE_NATIVE_WRITER_PROBE, probe);
        return None;
    }
    let source_released = source_method::<SourceStatus>(source, 0x44)(source, left) == 0;
    #[cfg(native_pack_correction_probe_diagnostic)]
    if source_released {
        native_pack_correction_probe_or(state, 1 << 3);
    }
    #[cfg(native_writer_readiness_probe_diagnostic)]
    if source_released {
        probe |= 1 << 3;
    }
    if !source_released {
        #[cfg(native_writer_readiness_probe_diagnostic)]
        write_u32(state, STATE_NATIVE_WRITER_PROBE, probe);
        return None;
    }
    let left_total = native_slot_extent(left, 1);
    let right_total = native_slot_extent(left + 1, 1);
    #[cfg(native_pack_correction_probe_diagnostic)]
    {
        native_pack_correction_probe_or(
            state,
            (((left_total != 0) as u32) << 4)
                | (((right_total != 0) as u32) << 5)
                | (((left_total != 0 && left_total == right_total) as u32) << 6),
        );
    }
    #[cfg(native_writer_readiness_probe_diagnostic)]
    {
        probe |= ((left_total != 0) as u32) << 4;
        probe |= ((right_total != 0) as u32) << 5;
        probe |= ((left_total != 0 && left_total == right_total) as u32) << 6;
    }
    if left_total == 0 || left_total != right_total {
        #[cfg(native_writer_readiness_probe_diagnostic)]
        write_u32(state, STATE_NATIVE_WRITER_PROBE, probe);
        return None;
    }
    if native_slot_extent(left, 3) < left_total {
        #[cfg(native_writer_readiness_probe_diagnostic)]
        write_u32(state, STATE_NATIVE_WRITER_PROBE, probe);
        return None;
    }
    #[cfg(native_pack_correction_probe_diagnostic)]
    native_pack_correction_probe_or(state, 1 << 7);
    #[cfg(native_writer_readiness_probe_diagnostic)]
    {
        probe |= 1 << 7;
    }
    if native_slot_extent(left + 1, 3) < right_total {
        #[cfg(native_writer_readiness_probe_diagnostic)]
        write_u32(state, STATE_NATIVE_WRITER_PROBE, probe);
        return None;
    }
    #[cfg(native_pack_correction_probe_diagnostic)]
    native_pack_correction_probe_or(state, 1 << 8);
    #[cfg(native_writer_readiness_probe_diagnostic)]
    write_u32(state, STATE_NATIVE_WRITER_PROBE, probe | (1 << 8));
    Some(left_total)
}

#[cfg(native_dual_capture_poc)]
#[cfg(not(stereo_stock_save_queue))]
unsafe fn native_stereo_pump(manager: *mut u8, state: *mut u8) -> u32 {
    let source = source_from_manager(manager);
    let slot = read_u16(state, STATE_CAPTURE_SLOT);
    #[cfg(native_finalization_probe_diagnostic)]
    update_native_finalization_probe(state, slot);
    #[cfg(native_pack_correction_probe_diagnostic)]
    native_pack_correction_probe_or(state, 1 << 2);
    if read_u8(state, STATE_WRITER_PHASE) == WRITER_PHASE_WAIT_NATIVE_NORMALIZATION {
        let Some(total) = native_pair_pcm_ready(
            source,
            slot,
            #[cfg(any(
                native_writer_readiness_probe_diagnostic,
                native_pack_correction_probe_diagnostic
            ))]
            state,
        ) else {
            return 0;
        };
        write_u32(state, STATE_TOTAL_FRAMES, total);
        write_u8(
            state,
            STATE_WRITER_PHASE,
            WRITER_PHASE_PREPARE_NATIVE_LEFT,
        );
        write_u8(state, STATE_ASYNC, 0);
        #[cfg(native_pack_correction_probe_diagnostic)]
        {
            native_pack_correction_probe_clear(state, 1 << 10);
            native_pack_correction_probe_or(state, 1 << 9);
        }
        function::<AsyncOpen>(ASYNC_OPEN)(
            state.add(STATE_DESCRIPTOR),
            stereo_open_callback as *const () as usize,
            state,
        );
        return 1;
    }
    #[cfg(native_writer_readiness_probe_diagnostic)]
    write_u32(
        state,
        STATE_NATIVE_WRITER_PROBE,
        native_writer_probe_base(state)
            | (read_u32(state, STATE_NATIVE_WRITER_PROBE) & 0x1f8),
    );

    let source_ended = source_method::<SourceStatus>(source, 0x44)(source, slot) == 0;
    let total_frames = native_slot_extent(slot, 1);
    if total_frames == 0 {
        return 0;
    }
    write_u32(state, STATE_TOTAL_FRAMES, total_frames);
    let written_frames = read_u32(state, STATE_WRITTEN_FRAMES);
    let mut buffered_frames = read_u32(state, STATE_BUFFERED_FRAMES);
    let retained_bytes = read_u8(state, STATE_BUFFER_OFFSET) as u32;
    if written_frames >= total_frames && buffered_frames == 0 && retained_bytes == 0 {
        if source_ended {
            write_u8(state, STATE_ASYNC, 4);
            return 1;
        }
        return 0;
    }
    if source_method::<SourceStatus>(source, 0x34)(source, slot) != 0 {
        return 1;
    }
    let available_frames = native_slot_extent(slot, 3);
    let mut buffer = read_u32(state, STATE_BUFFER) as *mut i16;
    if buffer.is_null() {
        buffer = source_method::<SourceBuffer>(source, 0x4c)(source, slot);
        write_u32(state, STATE_BUFFER, buffer as u32);
    }
    if buffer.is_null() {
        return 0;
    }

    if retained_bytes != 0 && buffered_frames == 0 {
        let retained_frames = retained_bytes / 2;
        let mut frame = 0;
        while frame < retained_frames {
            write_volatile(
                buffer.add(frame as usize),
                read_volatile(
                    state
                        .add(STATE_NATIVE_RETAINED_PCM)
                        .cast::<i16>()
                        .add(frame as usize),
                ),
            );
            frame += 1;
        }
        buffered_frames = retained_frames;
        write_u32(state, STATE_BUFFERED_FRAMES, buffered_frames);
        write_u8(state, STATE_BUFFER_OFFSET, 0);
    }

    let cursor = written_frames + buffered_frames;
    if available_frames > cursor && buffered_frames < OUTPUT_BLOCK_SPAN {
        let page_frames_remaining = DSP_PAGE_FRAMES - cursor % DSP_PAGE_FRAMES;
        let requested = core::cmp::min(
            core::cmp::min(
                core::cmp::min(available_frames - cursor, RAW_READ_MAX_WORDS),
                page_frames_remaining,
            ),
            OUTPUT_BLOCK_SPAN - buffered_frames,
        );
        let destination = buffer.add(buffered_frames as usize);
        let physical =
            source_method::<SourcePosition>(source, 0x08)(source, slot, cursor);
        let prefix = if physical & 0xff == 0 {
            core::cmp::min(requested, 2)
        } else {
            0
        };
        let bulk_requested = requested - prefix;
        let bulk_read = if bulk_requested == 0 {
            0
        } else {
            source_method::<SourceRead>(source, 0x18)(
                source,
                slot,
                cursor + prefix,
                destination.add(prefix as usize),
                bulk_requested,
            )
        };
        if prefix != 0 && bulk_read != 0 {
            let sample_2 = read_volatile(destination.add(prefix as usize)) as i32;
            let sample_3 = if bulk_read > 1 {
                read_volatile(destination.add(prefix as usize + 1)) as i32
            } else {
                sample_2
            };
            let sample_1 = (sample_2 * 2 - sample_3)
                .clamp(i16::MIN as i32, i16::MAX as i32);
            let sample_0 = (sample_1 * 2 - sample_2)
                .clamp(i16::MIN as i32, i16::MAX as i32);
            write_volatile(destination, sample_0 as i16);
            if prefix > 1 {
                write_volatile(destination.add(1), sample_1 as i16);
            }
        }
        let frames_read = prefix + bulk_read;
        if frames_read == 0 || frames_read > requested {
            source_method::<SourceBuffer>(source, 0x50)(source, slot);
            write_u32(state, STATE_BUFFER, 0);
            fail_data_path(state);
            return 0;
        }
        buffered_frames += frames_read;
        write_u32(state, STATE_BUFFERED_FRAMES, buffered_frames);
    }

    let remaining_frames = total_frames.saturating_sub(written_frames);
    let write_frames = if written_frames == 0 && buffered_frames >= OUTPUT_BLOCK_SPAN {
        core::cmp::min(remaining_frames, FIRST_OUTPUT_SPAN)
    } else if buffered_frames >= OUTPUT_BLOCK_SPAN {
        core::cmp::min(remaining_frames, OUTPUT_BLOCK_SPAN)
    } else if source_ended && written_frames + buffered_frames >= total_frames {
        core::cmp::min(remaining_frames, buffered_frames)
    } else {
        return 1;
    };
    if write_frames == 0 {
        return 1;
    }
    write_u8(state, STATE_ASYNC, 2);
    function::<AsyncWrite>(ASYNC_WRITE)(
        state.add(STATE_DESCRIPTOR),
        written_frames * 2 + 0x2c,
        buffer,
        write_frames * 2,
        stereo_file_callback as *const () as usize,
        state,
    );
    1
}

// Stock-save-queue pass-through, matching the guarded path the full callback
// below always took once the custom writer stopped being claimed.
#[cfg(stereo_stock_save_queue)]
#[no_mangle]
pub unsafe extern "C" fn stereo_file_callback(
    arg1: u32,
    arg2: u32,
    status: u32,
    arg4: u32,
    bytes_written: u32,
    state: *mut u8,
) {
    stock_file_callback_trampoline(arg1, arg2, status, arg4, bytes_written, state);
}

#[cfg(not(stereo_stock_save_queue))]
#[no_mangle]
pub unsafe extern "C" fn stereo_file_callback(
    _arg1: u32,
    _arg2: u32,
    status: u32,
    _arg4: u32,
    bytes_written: u32,
    state: *mut u8,
) {
    if state.is_null()
        || !matches!(
            read_u8(state, STATE_WRITER_MODE),
            WRITER_MODE_CUSTOM_MONO | WRITER_MODE_CUSTOM_STEREO
        )
    {
        stock_file_callback_trampoline(
            _arg1,
            _arg2,
            status,
            _arg4,
            bytes_written,
            state,
        );
        return;
    }

    match read_u8(state, STATE_ASYNC) {
        2 => {
            #[cfg(synthetic_commit_probe_diagnostic)]
            {
                #[cfg(data_callback_close_probe_diagnostic)]
                {
                    write_u8(state, STATE_ASYNC, 6);
                    function::<AsyncClose>(ASYNC_CLOSE)(
                        state.add(STATE_DESCRIPTOR),
                        probe_hang_close_callback as *const () as usize,
                        state,
                    );
                    return;
                }

                #[cfg(not(data_callback_close_probe_diagnostic))]
                {
                    if status == 0 && bytes_written == SYNTHETIC_CHUNK_SAMPLES * 2 {
                        let written = read_u32(state, STATE_WRITTEN_FRAMES)
                            + SYNTHETIC_CHUNK_SAMPLES;
                        write_u32(state, STATE_TOTAL_FRAMES, SYNTHETIC_TOTAL_SAMPLES);
                        write_u32(state, STATE_WRITTEN_FRAMES, written);
                        #[cfg(all_data_callback_close_probe_diagnostic)]
                        if written >= SYNTHETIC_TOTAL_SAMPLES {
                            write_u8(state, STATE_ASYNC, 6);
                            function::<AsyncClose>(ASYNC_CLOSE)(
                                state.add(STATE_DESCRIPTOR),
                                probe_hang_close_callback as *const () as usize,
                                state,
                            );
                        } else {
                            write_u8(state, STATE_ASYNC, 1);
                        }
                        #[cfg(not(all_data_callback_close_probe_diagnostic))]
                        write_u8(
                            state,
                            STATE_ASYNC,
                            if written >= SYNTHETIC_TOTAL_SAMPLES {
                                4
                            } else {
                                1
                            },
                        );
                    } else {
                        write_u8(state, STATE_ASYNC, 3);
                    }
                    return;
                }
            }

            #[cfg(not(synthetic_commit_probe_diagnostic))]
            {
            let source = writer_source(state);
            let slot = read_u16(state, STATE_CAPTURE_SLOT);
            if status == 0 && bytes_written & 1 == 0 {
                let submitted_frames = bytes_written / 2;
                let buffered_frames = read_u32(state, STATE_BUFFERED_FRAMES);
                if submitted_frames > buffered_frames {
                    fail_data_callback(state);
                    source_method::<SourceBuffer>(source, 0x50)(source, slot);
                    write_u32(state, STATE_BUFFERED_FRAMES, 0);
                    write_u32(state, STATE_BUFFER, 0);
                    return;
                }
                let retained_frames = buffered_frames - submitted_frames;
                let retained_bytes = retained_frames * 2;
                #[cfg(native_dual_capture_poc)]
                let retained_limit = 44;
                #[cfg(not(native_dual_capture_poc))]
                let retained_limit = u8::MAX as u32;
                if retained_bytes > retained_limit {
                    fail_data_callback(state);
                    source_method::<SourceBuffer>(source, 0x50)(source, slot);
                    write_u32(state, STATE_BUFFERED_FRAMES, 0);
                    write_u32(state, STATE_BUFFER, 0);
                    return;
                }
                let written = read_u32(state, STATE_WRITTEN_FRAMES) + submitted_frames;
                write_u32(state, STATE_WRITTEN_FRAMES, written);
                write_u8(state, STATE_BUFFER_OFFSET, retained_bytes as u8);
                write_u8(state, STATE_ASYNC, 1);
                #[cfg(native_dual_capture_poc)]
                let total = native_slot_extent(slot, 1);
                #[cfg(native_dual_capture_poc)]
                {
                    if retained_frames != 0 {
                        let buffer = read_u32(state, STATE_BUFFER) as *mut i16;
                        let mut frame = 0;
                        while frame < retained_frames {
                            write_volatile(
                                state
                                    .add(STATE_NATIVE_RETAINED_PCM)
                                    .cast::<i16>()
                                    .add(frame as usize),
                                read_volatile(
                                    buffer.add((submitted_frames + frame) as usize),
                                ),
                            );
                            frame += 1;
                        }
                    }
                    write_u32(state, STATE_TOTAL_FRAMES, total);
                    if source_method::<SourceStatus>(source, 0x44)(source, slot) == 0
                        && written >= total
                        && retained_frames == 0
                    {
                        write_u8(state, STATE_ASYNC, 4);
                    }
                }
                #[cfg(not(native_dual_capture_poc))]
                {
                let source_ended_before_total =
                    source_method::<SourceStatus>(source, 0x44)(source, slot) == 0;
                let mut raw_total = source_method::<SourceTotal>(source, 0)(source, slot);
                #[cfg(terminal_odd_word_clamp_diagnostic)]
                if source_ended_before_total
                    && source_method::<SourceStatus>(source, 0x44)(source, slot) == 0
                {
                    raw_total &= !1;
                }
                write_u32(state, STATE_TOTAL_FRAMES, raw_total / 2);
                if source_ended_before_total
                    && source_method::<SourceStatus>(source, 0x44)(source, slot) == 0
                    && (raw_total & 1 == 0)
                    && written >= raw_total / 2
                    && retained_frames == 0
                {
                    write_u8(state, STATE_ASYNC, 4);
                }
                }
            } else {
                fail_data_callback(state);
            }
            source_method::<SourceBuffer>(source, 0x50)(source, slot);
            write_u32(state, STATE_BUFFERED_FRAMES, 0);
            write_u32(state, STATE_BUFFER, 0);
            }
        }
        5 => {
            #[cfg(native_terminal_publish_probe_diagnostic)]
            {
                let mut flags = 1 << 0;
                if read_u32(state, STATE_MAGIC) == WRITER_MAGIC_RIGHT {
                    flags |= 1 << 1;
                }
                native_terminal_publish_probe_or(state, flags);
            }
            if status != 0 || bytes_written != 0x2c {
                close_before_rollback(state);
                return;
            }
            #[cfg(sample_pair_identity_poc)]
            if matches!(
                read_u32(state, STATE_MAGIC),
                WRITER_MAGIC_LEFT | WRITER_MAGIC_RIGHT
            ) {
                let marker = if read_u32(state, STATE_MAGIC) == WRITER_MAGIC_LEFT {
                    WAV_BYTE_RATE_STEREO_LEFT
                } else {
                    WAV_BYTE_RATE_STEREO_RIGHT
                };
                write_u32(state, STATE_HEADER_WRITE_MARKER, marker);
                write_u8(state, STATE_ASYNC, 10);
                function::<AsyncWrite>(ASYNC_WRITE)(
                    state.add(STATE_DESCRIPTOR),
                    WAV_BYTE_RATE_OFFSET as u32,
                    state.add(STATE_HEADER_WRITE_MARKER).cast::<i16>(),
                    4,
                    stereo_file_callback as *const () as usize,
                    state,
                );
                return;
            }
            write_u8(state, STATE_ASYNC, 6);
            #[cfg(hang_after_first_close_diagnostic)]
            let close_callback = probe_hang_close_callback as *const () as usize;
            #[cfg(not(hang_after_first_close_diagnostic))]
            let close_callback = stereo_file_callback as *const () as usize;
            function::<AsyncClose>(ASYNC_CLOSE)(
                state.add(STATE_DESCRIPTOR),
                close_callback,
                state,
            );
        }
        #[cfg(sample_pair_identity_poc)]
        10 => {
            #[cfg(native_terminal_publish_probe_diagnostic)]
            {
                let mut flags = 1 << 2;
                if read_u32(state, STATE_MAGIC) == WRITER_MAGIC_RIGHT {
                    flags |= 1 << 3;
                }
                native_terminal_publish_probe_or(state, flags);
            }
            if status != 0 || bytes_written != 4 {
                close_before_rollback(state);
                return;
            }
            write_u8(state, STATE_ASYNC, 6);
            #[cfg(hang_after_first_close_diagnostic)]
            let close_callback = probe_hang_close_callback as *const () as usize;
            #[cfg(not(hang_after_first_close_diagnostic))]
            let close_callback = stereo_file_callback as *const () as usize;
            function::<AsyncClose>(ASYNC_CLOSE)(
                state.add(STATE_DESCRIPTOR),
                close_callback,
                state,
            );
        }
        6 => {
            #[cfg(native_terminal_publish_probe_diagnostic)]
            {
                let mut flags = 1 << 4;
                if status == 0 {
                    flags |= 1 << 5;
                }
                native_terminal_publish_probe_or(state, flags);
            }
            if status != 0 {
                rollback_closed_files(state);
                return;
            }
            #[cfg(rollback_after_right_sync_diagnostic)]
            if read_u32(state, STATE_MAGIC) == WRITER_MAGIC_RIGHT {
                rollback_closed_files(state);
                return;
            }
            if read_u32(state, STATE_MAGIC) == WRITER_MAGIC_LEFT
                && !cfg!(single_pass_left_diagnostic)
            {
                let right_slot =
                    (read_u32(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT) as u8).wrapping_add(1);
                #[cfg(all(
                    not(skip_content_index_diagnostic),
                    not(safe_pair_guard)
                ))]
                if !destination_is_empty(state.add(STATE_DESCRIPTOR), right_slot) {
                    rollback_closed_files(state);
                    return;
                }
                write_u32(state, STATE_MAGIC, WRITER_MAGIC_RIGHT_PENDING);
                #[cfg(native_dual_capture_poc)]
                write_u8(
                    state,
                    STATE_WRITER_PHASE,
                    WRITER_PHASE_PREPARE_NATIVE_RIGHT,
                );
                #[cfg(not(native_dual_capture_poc))]
                write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_PREPARE_RIGHT);
                write_u8(
                    state,
                    STATE_DESCRIPTOR + DESCRIPTOR_SLOT,
                    right_slot,
                );
                write_u8(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT + 1, 0);
                write_u8(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT + 2, 0);
                write_u8(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT + 3, 0);
                #[cfg(native_dual_capture_poc)]
                write_u16(state, STATE_CAPTURE_SLOT, right_slot as u16);
                reset_pass_counters(state);
                write_u8(state, STATE_ASYNC, 0);
                #[cfg(native_pack_correction_probe_diagnostic)]
                if read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_NATIVE_STEREO {
                    native_pack_correction_probe_clear(state, 1 << 10);
                    native_pack_correction_probe_or(state, 1 << 9);
                }
                function::<AsyncOpen>(ASYNC_OPEN)(
                    state.add(STATE_DESCRIPTOR),
                    stereo_open_callback as *const () as usize,
                    state,
                );
            } else {
                let terminal_magic = read_u32(state, STATE_MAGIC);
                #[cfg(native_terminal_publish_probe_diagnostic)]
                {
                    let magic_flag = match terminal_magic {
                        WRITER_MAGIC_RIGHT => 1 << 7,
                        WRITER_MAGIC_RIGHT_PENDING => 1 << 8,
                        WRITER_MAGIC_LEFT => 1 << 9,
                        WRITER_MAGIC_MONO => 1 << 10,
                        0 => 1 << 11,
                        _ => 1 << 12,
                    };
                    let mut flags = (1 << 6) | magic_flag;
                    if read_u8(state, STATE_WRITER_MODE) == WRITER_MODE_NATIVE_STEREO {
                        flags |= 1 << 13;
                    }
                    if read_u8(state, STATE_WRITER_PHASE) == WRITER_PHASE_WRITE_NATIVE_RIGHT {
                        flags |= 1 << 14;
                    }
                    if stereo_left_for_right(
                        read_u32(state, STATE_DESCRIPTOR + DESCRIPTOR_SLOT) as u8,
                    )
                    .is_some()
                    {
                        flags |= 1 << 15;
                    }
                    native_terminal_publish_probe_or(state, flags);
                }
                write_u32(state, STATE_MAGIC, 0);
                if terminal_magic == WRITER_MAGIC_RIGHT {
                    #[cfg(any(
                        native_publication_probe_diagnostic,
                        native_publish_boundary_probe_diagnostic,
                        native_publish_slot_probe_diagnostic
                    ))]
                    {
                    write_u8(
                        state,
                        STATE_WRITER_PHASE,
                        WRITER_PHASE_COMMITTED_WAIT_UI,
                    );
                    #[cfg(native_publication_probe_diagnostic)]
                    native_publication_probe_or(state, 1 << 1);
                    #[cfg(native_publish_boundary_probe_diagnostic)]
                    native_publish_boundary_probe_or(state, 1 << 5);
                    write_u8(state, STATE_WRITER_WAIT_TICKS, 0);
                    }
                    #[cfg(not(any(
                        native_publication_probe_diagnostic,
                        native_publish_boundary_probe_diagnostic,
                        native_publish_slot_probe_diagnostic
                    )))]
                    release_committed_writer(state);
                } else {
                    write_u8(state, STATE_WRITER_MODE, WRITER_MODE_IDLE);
                    write_u8(state, STATE_WRITER_PHASE, WRITER_PHASE_IDLE);
                    write_u8(state, STATE_WRITER_WAIT_TICKS, 0);
                }
                #[cfg(sample_cache_refresh_poc)]
                if matches!(terminal_magic, WRITER_MAGIC_MONO | WRITER_MAGIC_RIGHT) {
                    sample_pair_publish_refresh_token(
                        state,
                        terminal_magic == WRITER_MAGIC_RIGHT,
                    );
                }
                #[cfg(native_deferred_refresh_probe_diagnostic)]
                if terminal_magic == WRITER_MAGIC_RIGHT {
                    write_u32(
                        state,
                        STATE_CAPTURE_UI_TOKEN,
                        DEFERRED_REFRESH_READY_MAGIC,
                    );
                }
                #[cfg(direct_terminal_completion_diagnostic)]
                {
                    let completion = read_u32(state, STATE_COMPLETION) as usize;
                    let completion_context = read_u32(state, STATE_COMPLETION_CONTEXT);
                    let owner = read_u32(state, STATE_OWNER) as *mut u8;
                    function::<ResetWriter>(RESET_WRITER)(owner);
                    write_u8(state, STATE_ASYNC, 8);
                    if completion != 0 {
                        function::<Completion>(completion)(0, completion_context);
                    }
                    return;
                }
                #[cfg(not(direct_terminal_completion_diagnostic))]
                write_u8(state, STATE_ASYNC, 7);
            }
        }
        9 => rollback_closed_files(state),
        _ => {}
    }
}

unsafe fn capture_transform_to(
    input: *const i16,
    output: *mut i16,
    word_count: u32,
    transform: u32,
) -> i32 {
    if input.is_null() || output.is_null() || word_count & 1 != 0 {
        return -1;
    }
    if transform > TRANSFORM_RIGHT {
        return -2;
    }

    let frame_count = word_count / 2;
    let mut frame = 0;
    while frame < frame_count {
        let source = input.add((frame * 2) as usize);
        let left = source.read() as i32;
        let right = source.add(1).read() as i32;
        let value = if transform == TRANSFORM_MONO {
            ((left + right) >> 1) as i16
        } else if transform == TRANSFORM_LEFT {
            left as i16
        } else {
            right as i16
        };
        write_volatile(output.add(frame as usize), value);
        frame += 1;
    }
    frame_count as i32
}

#[no_mangle]
#[cfg(final_write_tone_diagnostic)]
pub unsafe extern "C" fn diagnostic_fatfs_write(
    file: *mut u8,
    buffer: *mut i16,
    length: u32,
    written: *mut u32,
) -> u32 {
    let address = buffer as usize;
    if address >= 0x2000_3f94
        && address < 0x2000_4f94
        && length & 1 == 0
    {
        let samples = length / 2;
        let mut sample = 0;
        while sample < samples {
            write_volatile(
                buffer.add(sample as usize),
                if sample & 31 < 16 { 0x2000 } else { -0x2000 },
            );
            sample += 1;
        }
    }
    function::<FatFsWrite>(0x0802_20fd)(file, buffer, length, written)
}
