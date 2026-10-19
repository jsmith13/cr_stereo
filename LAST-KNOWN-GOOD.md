# LAST KNOWN GOOD

Build: build\series-b (released as v0.2.0)
Image: circuitrhythm-firmware-5706-stereo-poc.bin
Size:  391436 bytes
SHA-256: 8f0a427e17a7f274e8e07ed1ba1f78c5a367bc99cc03d2d97557a51542b53217
Pinned: 2026-10-18 (hardware-confirmed by user)

Build command for this pin (the v0.1.0 command plus -KeygroupPoc):
  pwsh tools\build_stereo_firmware.ps1 -OutputDirectory build\series-b `
    -NativeDualCapturePoc -SafePairGuard -SamplePairLedPoc -SamplePairIdentityPoc `
    -SamplePairGesturePoc -SamplePairWriterCouplingPoc -SampleCacheRefreshPoc `
    -PairedPlaybackPoc -StereoPositionPoc -StereoStockSaveQueue -CropMirrorWindow `
    -KeygroupPoc

Confirmed working on hardware (new since memfull-c):
  * Auto-captured series: mono series tan, stereo series sky blue on the
    Sample Rec page and the Sample page bank; selecting a member lights the
    whole series; Clear on any member clears the whole series.
  * Key-group pitch: unity is note 60 (key groups off, Note view C4 matches
    the Sample Rec preview); every key now matches the sampled synth. The
    earlier 84 was wrong. Pitch-shift noise far from a root (spacing 24) is
    stock DSP resampling, not a payload defect.
  * Memory gauge (pad 22 hold) and the old page-8 green paired-playback
    diagnostic removed.

Previous pin: build\memfull-c (390920 B, SHA-256
afddd6da4b22415c9e3971f55bde2e0b237a024cdc2b0dc3497d9f8beedfa2a2).

Confirmed working on hardware (carried from memfull-c; new since
sample-bank-b; details and evidence in reverse-engineering-log.md):
  * Key groups: tagged sample sets play across the keys (nearest root);
    tags live in the WAV byte-rate field and survive reboot;
    Sample Mode pad 2 switches a track's key group off (saved per project).
  * Auto-sampler (Sample Rec row-3 pad 8): drives an external synth over MIDI,
    one slot per note, mono or stereo; trims silence before the first save,
    and records the next take while the previous one saves.
  * Auto-sampler settings: row-3 pad 6 + row 4 picks the semitone step; pad 7
    toggles a 1 s / 4 s hold.
  * Stereo DSP writer bounds: the writer stops at the stock discard slot $7F,
    not $80. Before this, a failed paired allocation left it writing past the
    page tables, which hung saves once memory was full. Clear does return
    pages to the DSP free ring (y:$371d1), confirmed with the memory gauge.
  * Sample memory: Record is refused (rows 3-4 flash red) with under ~1 s
    per channel free; a take stops itself 16 pages before memory runs out;
    the auto-sampler refuses runs that will not fit (amber row-4 pad 4).
  * Stereo with memory well used: the pair allocator rejected every page
    >= $800 (sign-extended tst); fixed. Abandoned pairs no longer link the
    next take into their slots (stereo_carry_forget_slots).

Previous pin: build\memgauge-a (390368 B, SHA-256
73a1d048dc063611dfab18abcb4944b8ae858d7a052a28c0dcc9c4298db05ee2), and before
it build\sample-bank-b (384796 B, SHA-256
e5463e328811c577d5afa94dbb5b26b88381a386281fd81b4c7bef17a4e925b6; v0.1.0).

Confirmed working on hardware (carried from sample-bank-b):
  * Sample bank colours on the track Sample page (bottom two rows): empty off,
    mono dim white, stereo half dim teal; selected slot bright white / teal,
    selected empty dim red; a paired track lights both halves. Page group
    compose slot 0x080678F8 -> sample_bank_compose wraps 0x0804DD1D; occupancy
    model found by SRAM scan, cached at 0x2004FC44.

Previous pin: build\pair-unpair-b (384188 B, SHA-256
52c7289b3560e4c15a1e6687f7c10bbc653ef9b8ac816d8cca9abd1bbe98f089).

Confirmed working on hardware (carried from pair-unpair-b):
  * Pair button lights: selecting either track of a pair lights both track
    buttons in the leader's colour. Overlay on the track-button LED layer (UI
    root +0x38, vtable 0x080671F8) via vtable slots 0x08067218/0x08067228 plus
    a main-loop re-apply; a former partner cell is always unlit.
  * Follower silencing: the follower's own pattern and live input are dropped
    at the note-on/off adapter hooks while paired; the leader's mirror still
    plays the right half.
  * Shared views: the follower's button presses the leader's on every page.
    The redirect is recorded at 0x2004FC34+button and undone on release (root
    release vtable slot 0x080672F0 -> 0x0803DDB1). A selected follower is
    snapped to the leader from the main loop.
  * Unpair: the pairing gesture on a formed pair splits it. The follower gets
    its pre-pair sample (RAM 0x2004FC3C+track, lost on reboot), else its blank
    default 2t or the next non-stereo-half sample; never a half of the pair.

Previous pin: build\pair-colour-b (383392 B, SHA-256
6dfb773a815600bb484c49c7ef4d891a32d54306878eae4f1917e4d32adc174e).

Confirmed working on hardware (carried from pair-colour-b):
  * Shared pair colour everywhere: the follower shows the leader's colour on
    its track button, Sample/Sample Mode buttons, encoders and sample pads,
    and reverts when unpaired. The colour enum table (0x08062AB0, 568 u16) is
    copied to RAM at 0x2004F760 and all 53 literals are redirected there. The
    per-track palette RGB slots 0x129/0x131/0x149+t (RAM palette 0x20003974)
    and the encoder slots 0xE9+8t+k are rewritten from boot copies saved at
    0x2004FBD0. Hooks: `bl main` at 0x08068E34 and the Main task entry pointer
    at 0x0802614C.

Previous pin: build\track-pair-gesture-f (382904 B, SHA-256
378d054297b492d39069c541469778c27d205b71e375a3a9abd2ad7a20603709).

Confirmed working on hardware (carried from track-pair-gesture-f):
  * Pairing gesture: hold track N and press an adjacent track. If the held (or
    pressed) track holds the left half of an identified stereo recording, the
    lower track gets the left half and the upper track the right half, then
    they couple as a pair. It writes BOTH copies of each track's sample, the
    session observable (session +0x9c+t*4) and track parameter 0 via
    ObservableSet 0x080465DD. Writing either one alone does not work. The
    gesture does nothing on a mono sample (the press is swallowed); on an
    existing pair it now unpairs (see above). Ordinary track presses are unaffected. Hook: UI root vtable
    slot 0x080672EC.

Confirmed working on hardware (carried from track-pair-ui-n):
  * Sidechain residue: splitting a pair resets the old follower to preset 0 and
    source track 1, and forming a pair copies the leader's sidechain to the
    follower at once. Neither needs the sidechain page to have been opened: the
    page is found at boot via the UI root (0x20006EA0 ctx -> +0x3c -> +0x10,
    checked against vtable 0x08067F14) and serviced from the main loop.

Confirmed working on hardware (carried from track-pair-ui-l):
  * A stereo sample can be assigned to ANY two adjacent tracks (N, N+1). The
    follower plays from the leader's sequence, and re-pairing (e.g. 1/2 -> 2/3)
    hands over correctly.
  * Mixer: volume, mute and pan are coupled, with stereo panning; the follower
    takes the leader's official track colour.
  * FX page: the send level is coupled. Sidechain: coupled, including across
    pages, with the follower coloured as the leader.
  * Splitting a pair leaves no residue: the old follower's pan, FX send and
    volume return to their defaults.
  * Default track volume for blank projects is 89 (70%; stock is 100), via the
    template byte at 0x0806880A. Volume is track parameter 16.
  * Everything from the previous pin, crop mirroring included.

ALWAYS build with -CropMirrorWindow: the track-pair-ui-a..k builds omitted it
and silently lost crop mirroring.

Confirmed working on hardware (carried from crop-mirror-save-b):
  * Cropping one half of a stereo pair CROPS BOTH HALVES, and the crop SURVIVES
    A POWER CYCLE on both slots. The partner is rewritten by a second ordinary
    stock save, pumped through the mirrored capture window.
  * That partner save is gated on a consume-once mirror latch
    (STATE_CROP_MIRROR_ARM, magic 0x43520000 | slot), so an ordinary save of a
    paired slot that was NOT just mirrored never rewrites its partner's file.
  * Cropping one half of a stereo pair NO LONGER DISSOCIATES THE PAIR across a
    power cycle. Any ordinary save of a paired slot now restamps the 96001/96002
    sentinel from the durable registry instead of losing it.
  * Stereo pair identity SURVIVES A POWER CYCLE: pairs show teal immediately at
    boot, with no Clear or other action needed to "wake" the registry, and the
    FIRST pair-wide Clear after a boot clears both members.
  * Pair-wide Clear: holding Clear and pressing EITHER member of a stereo pair
    clears both slots, in both directions (N -> N+1 and N+1 -> N), regardless of
    which member was last previewed/selected. Mono Clear is unaffected.
  * Identical on the first pair recorded in a session and on every later pair.
  * Two-pad Sample Rec gesture arms a stereo pair.
  * Left channel -> slot N, right channel -> slot N+1, cleanly separated, no artifacts.
  * Right slot publishes and is playable immediately after save, no reboot required.
  * N and N+1 are recognised as a stereo pair, including on the FIRST recording
    after a cold boot (the long-standing first-recording defect).
  * Pairing survives reboot (96001/96002 WAV markers are stamped).
  * Both pads fill with stereo teal and each channel previews independently the
    moment recording stops, identically on the first and on later recordings.
  * Both pads flash red during recording, mirroring stock mono behaviour.
  * No green save-flash diagnostic.
  * Pairs stay linked across overlapping captures: starting new recordings while
    earlier pairs are still saving no longer unlinks them, verified several deep.

Previous pin: build\track-pair-ui-n (382368 bytes, SHA-256
e9a35409e5d59d61d8a7fa951d648f330977ea82929e7bf26dd0a487824e87f0), and before it
build\track-pair-ui-l (382020 bytes, SHA-256
c9c6943804fa4169ca37e229e2581e875769109452f86abb5e59e4994eeab48d), and before it
build\crop-mirror-save-b (380176 bytes, SHA-256
61a758ef657bf3421019ccb5d4ce94627cfb534cffb545152088133ca0fffd13), and before it
build\pair-sentinel-keep-b (379836 bytes, SHA-256
87a354f8625307804731d0c901afff2dce516a7ea6952bc262d4107364bb8dd1), and before it
build\pair-pack-ui-a (379796 bytes, SHA-256
7b9f85631e422d37a7ebc8938af811cb46ffb3e05c7b30da42bc39e51b323e9a), and before it
build\stereo-pairclear-f (379600 bytes, SHA-256
7365a1f360fcd79f323e4a2d57886a1ee5f08f5a9486413733bd6f8801481cad), and before it
build\stereo-carry-b (379088 bytes, SHA-256
7ae4c3b05c89d6ec3c0c14dc2d3caaa465713cce0ccc741a893f32bfd53815b4), and before it
build\stereo-packheal-a (378700 bytes, SHA-256
1beb5cafcd9b295fd981848a25eecdd8c6eae72d73e3b0203a98706742bb52af). All build
directories are retained; see reverse-engineering-log.md for the candidate history
(stereo-uxfill-a..d, stereo-carry-a/b, stereo-pairclear-a..f, then the
pair-identity series pair-marker-probe-b, pair-scan-gate-a,
pair-commit-snapshot-a, pair-carry-registry-a/b, clear-branch-probe-a/b,
pair-pack-bind-a, pair-pack-ui-a).

Fourth invariant, added by pair-pack-ui-a: **byte 0 of ACTIVE_CONTENT_STORE is
not a pack source.** It is the read side of a double-buffered observable that the
stock loader publishes only AFTER a pack load returns (`FUN_08039c70` calls the
loader with `param_2`, then fetches the write-side buffer via `FUN_0803fd26`,
stores the pack, and publishes with `FUN_0803fbec`). Any code reading it to
decide identity is reading history -- measured as 24 while the live pack was 0.
Four separate defects in this investigation were this same mistake:
  1. The boot scan recorded ACTIVE_PACK = 0xff and let lookups fall back to the
     stale byte. Fixed by binding the pack from the load ARGUMENT.
  2. `registry_contains` compared against the stale byte as a fallback. It now
     fails closed; the scan always binds, so "unbound" genuinely means unknown.
  3. The LED overlay keyed its lookup on the stale byte, so no pair showed teal
     at boot and one Clear (which republishes) healed every other pair.
  4. The clear canonicalisation, gesture rewrite and crop partner lookup all did
     the same.
All pair lookups now route through a single `registry_pack(state)` helper, which
returns the bound pack whenever the registry is valid and consults the store byte
only while the registry is unpopulated. Add no new reader of that byte.

Also added here: `writer_state_constructor` runs LAZILY, mid-session, and its
reset list zeroes STATE_PAIR_VALID, STATE_REGISTRY_GENERATION,
STATE_PAIR_ACTIVE_BANK and STATE_PAIR_ACTIVE_PACK -- but NOT the bank bytes.
That wiped the boot scan's result after the fact, leaving the signature "pair
bits present, metadata pristine". The constructor now carries the registry
metadata across construction, copying banks pointer-to-pointer (a stack array
emits `__aeabi_memcpy`, which the payload cannot link). Treat EVERY field group
in that reset list as a latent instance of the same bug.

Third invariant, added by stereo-pairclear-f: the sample view only issues a Clear
request when the pressed slot EQUALS the selected slot (`FUN_0804f9d8` compares the
pressed slot against the observable at `view+0x20`). Any pad rewrite performed
before stock runs must therefore move the press TOWARD the selected member, never
onto the left member -- rewriting to the left silently suppresses Clear on an
already-selected right pad. Canonicalising onto the left member for the deletion
itself belongs in `sample_pair_clear_entry` at `0x0803A074`, which sits below that
gate and covers every clear path. Keep those two responsibilities separate.

Pair-wide Clear mechanism (stereo-pairclear-a..f): the left member is deleted by
the stock transaction, and the peer is deleted by a second, equally stock
transaction replayed from the main loop by `service_pending_peer_clear`. Never try
to coordinate both deletions inside one transaction -- that re-enters the storage
worker from its own callback and strands the durable-deletion lifecycle. The peer
replay calls `0x0803A10F` directly and MUST bypass `sample_pair_clear_entry`, or
the peer would be rewritten back onto the member just deleted. It waits for a
quiet settle window (busy observed clear for PEER_CLEAR_SETTLE_TICKS) because
`busy == 0` does not mean storage is idle: the op-8 filesystem sync is still
draining, and the first save of a session mounts the pack, making its sync by far
the longest.

Do not regress the invariant that made this work: no save-path decision may be
keyed on ACTIVE_CONTENT_STORE or on a pack snapshot taken at capture time. The
pack must come from the save descriptor. See reverse-engineering-log.md.

Second invariant, added by stereo-carry-a/b: pair identity for an in-flight save
must never be read from a single global word that the next capture can overwrite.
`native_capture_source_start` displaces the primary latch into the
`STATE_SAVE_PAIR_CARRY` FIFO table, and `stereo_stock_header_write` /
`sample_pair_save_publish` heal and retire exactly the entry that owns the slot --
never the primary on behalf of a carried pair. Violating this strands the right
channel's 96002 marker and unlinks the pair permanently, even across a reboot.

Root cause of the first-recording defect: Content_Write -> Pack_Access lazily
mounts the descriptor's pack and rewrites ACTIVE_CONTENT_STORE byte 0 mid-save.
See the 'WHY the first recording differed' entry in reverse-engineering-log.md.

> **Consolidated reference:** the whole first-recording investigation - symptom,
> root cause, fix, invariant, disproven theories and probe technique - is written up
> in `reports/first-recording-pack-mount-root-cause.md`. Start there.


Build command for this pin:
  pwsh tools\build_stereo_firmware.ps1 -OutputDirectory build\track-pair-gesture-f `
    -NativeDualCapturePoc -SafePairGuard -SamplePairLedPoc -SamplePairIdentityPoc `
    -SamplePairGesturePoc -SamplePairWriterCouplingPoc -SampleCacheRefreshPoc `
    -PairedPlaybackPoc -StereoPositionPoc -StereoStockSaveQueue -CropMirrorWindow

Note: -CropMirrorWindow and -CropIoTrace together still exceed the 381168-byte
ceiling, but only by 48 bytes (381216) after the dead byte-shift mirror was
removed. To trace crop I/O, retire one small diagnostic first.
