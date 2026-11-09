# Stereo sampling and track pairing

Available in every release since v0.1.0. Pads are given as (row, column),
counted from 1 at the top left, as printed on the instrument.

The Circuit Rhythm stores samples in mono. A stereo recording is therefore
kept as two mono samples next to each other: the **left** channel in slot *N*
and the **right** channel in slot *N+1*. The firmware remembers that the two
belong together, and two adjacent tracks can then play them back as one
stereo sound.

## Getting started

1. Connect a stereo source to the **L** and **R** sample inputs on the back.
2. Open the **Sample Rec** page.
3. Hold an empty pad and press the empty pad next to it. Both pads turn
   **red**: the pair is armed.
4. Press **Record** to start and again to stop, as for a stock recording.
   Both pads flash red while recording.
5. When recording stops, both pads turn **teal**. Each pad previews its own
   channel: the left pad plays the left channel, the right pad the right.
6. Open the **Sample** page for a track, for example track 1, and select the
   left (lower-numbered) half of the recording.
7. Hold track button **1** and press track button **2**. Track 1 now plays the
   left half, track 2 the right half, and the two act as one stereo track.

## Recording in stereo

### Arming a pair

On the Sample Rec page, hold one pad and press a neighbouring pad. The order
does not matter; the lower-numbered slot always gets the left channel.

| Requirement | Why |
|---|---|
| Both slots are empty | A pair is never recorded over an existing sample. |
| The pads are side by side in the same row | A pair cannot wrap from the end of one row to the start of the next. |

If either requirement is not met, nothing is armed and the press behaves like
a stock press. While a pair is armed, both pads stay red after you let go.
Pressing a single pad, or changing page, disarms it. Pressing **Record** with
a single empty pad selected makes an ordinary mono recording, as on stock
firmware.

### While recording

- Both pads flash red, as a stock mono recording does.
- You can arm and record the next pair while the previous one is still
  saving.
- If sample memory is nearly full, Record is refused, and a take that would
  run out of memory stops by itself and is saved intact. Either case flashes
  rows 3 and 4 red. A stereo take uses memory twice as fast as a mono one.

### After recording

The two halves stay linked after a power cycle, so they still show teal (see
the colours below) the next time you switch on.

| Action | Effect on a stereo pair |
|---|---|
| **Clear** | Hold Clear and press either half: both halves are cleared. |
| Crop (trim) | Cropping either half crops the other half the same way. |

To record a stereo multi-sample instrument automatically, arm a pair first
and then start the auto-sampler (row 3, pad 8). See the README's
*Key groups and auto-sampling* section. The auto-sampler needs the pair to
start on an odd-numbered pad (pad 1, 3, 5 or 7 of a row).

## Pad colours

### Sample Rec page

| Colour | Meaning |
|---|---|
| Red (both pads) | Stereo pair armed, ready to record |
| Flashing red | Recording |
| Teal | Half of a stereo recording |
| Sky blue | Half of a stereo series made by the auto-sampler |
| Tan | Member of a mono series made by the auto-sampler |
| Other colours | Stock: empty and mono slots look as on stock firmware |

### Sample page bank (the track's Sample page, rows 3 and 4)

| Colour | Meaning |
|---|---|
| Off | Empty slot |
| Dim white | Mono sample |
| Dim teal | Half of a stereo pair |
| Bright white / bright teal | Selected sample |
| Dim red | Selected slot is empty |

On a paired track, both halves of its pair are lit bright.

## Pairing tracks

### Pairing and unpairing

Hold a track button and press the track button next to it (for example 3
then 4, or 4 then 3). One of the two tracks must already hold the left half
of a stereo recording. The gesture then:

- gives the lower-numbered track the left half and the upper track the right
  half;
- couples the two tracks as a pair.

Any two neighbouring tracks can be paired (1+2, 2+3, and so on). The gesture
does nothing if neither track holds a stereo left half.

To unpair, repeat the gesture on the pair. The upper track goes back to the
sample it had before pairing. After a power cycle that sample is no longer
remembered; the track then gets its default sample instead, or the next
sample that is not half of a stereo pair.

A pairing is simply two neighbouring tracks holding the two halves of one
recording. It is saved with the project as part of the track samples, and
choosing a different sample on either track breaks it.

### How a pair behaves

The lower track is the **leader** and the upper track the **follower**.

- **One track to play:** the leader's pattern plays both halves. The
  follower's own pattern, and notes played on it, are silent while paired.
- **One track to edit:** both track buttons light in the leader's colour,
  and pressing either one opens the leader's views on every page.
- **Shared colour:** the follower shows the leader's colour everywhere. Its
  own colour returns when unpaired.
- **Mixer:** volume and mute are shared. Pan works on the leader only: at
  centre the two halves sit fully left and right, and turning pan slides the
  whole stereo image toward one side. The follower's pan is unlit.
- **FX and sidechain:** the FX sends and sidechain settings are shared.
- **Clean unpairing:** unpairing returns the follower's pan, FX send,
  volume and sidechain to their defaults.
- **Looper and grains:** a paired track loops and plays grains in stereo; see
  [looper-grains.md](looper-grains.md).

A stereo half on a track that is not paired plays as an ordinary mono sample.

## Other changes

- New blank projects start with every track at 70% volume (stock is 100%),
  leaving headroom when tracks are combined.
