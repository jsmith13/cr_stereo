# Micro-looper and grain engine

Requires a build with `-GranularLooper` (the v0.3.0 release command). Pads are
given as (row, column), counted from 1 at the top left, as printed on the
instrument.

## Getting started

1. Load a long sample on a track, for example track 1.
2. Hold **Shift** and press that track's button. The pads switch to the
   **looper page**.
3. Press a pad in rows 1 and 2 to choose where in the sample to play, then a
   pad in row 3 to choose how much of it.
4. Press **Loop** (4,3). It turns yellow and the section loops (a *preview*).
5. Press **Loop** again. It turns green: the loop is now *armed* and follows
   Play/Stop and the notes on the track. Press it a third time to turn it off.
6. Press **Up** for the **grain page**, which turns the same section into a
   cloud of short overlapping grains. **Down** returns to the looper page.
   The lit arrow always points to the other page.

To leave the view, press Shift and the same track button again, or press any
other function button (for example Mixer or Sample). Armed tracks keep
playing after the view closes; previews stop.

Pressing a track button (without Shift) while the view is open selects that
track and shows its settings. Each track has its own looper and grain
settings, so several tracks can loop or play grains at once. A paired stereo
track loops both halves together.

## Modes: Off, Preview and Armed

Each mode pad (One Shot, Gated and Loop on the looper page, Loop on the grain
page) cycles through three states:

| Colour | State | Behaviour |
|---|---|---|
| Dim white | Off | The track plays normally from the sequencer. |
| Yellow | Preview | Plays straight away, whatever the transport is doing. Stops when the view closes or moves to another track or page. On the looper page, how hard you press the pad sets the level. |
| Green | Armed | Plays under the transport and notes (below), and keeps going after the view closes. |
| Dim green | Armed, waiting | Armed, but waiting for Play (looper modes) or for a note (grains). |

Pressing a different mode pad on the same track switches to that mode in
Preview. A track has only one mode at a time.

While a track is in a mode, its ordinary note playback is replaced. The
looper never changes the sample settings saved in the project; when the mode
is turned off, the track plays with its own settings again.

### What Armed does

- **Armed One Shot, Gated and Loop** follow the transport: Stop silences
  them and Play restarts them from the start of the section. Every note the
  track receives (a sequencer step, the keyboard in Note view, or MIDI in)
  re-pitches them. C4 (MIDI note 60) plays at the tuned pitch, and each
  semitone away shifts it by a semitone. The last note's pitch is kept; note
  lengths do not matter.
- **Armed grains** sound only while a note is held, at that note's pitch and
  velocity (relative to C4, as above). Releasing the note stops new grains,
  and those already playing fade out. This works with the transport stopped
  too, from the keyboard or MIDI. To sequence grains, enter notes on the
  track's steps: each step's note, velocity and length set the grains' pitch,
  level and duration.

Tune and note offset together are limited to ±24 semitones.

## Looper page

```
         1       2       3       4       5       6       7       8
Row 1  [ start position: sixteenths 1-8 of the sample                     ]
Row 2  [ start position: sixteenths 9-16                                  ]
Row 3  [whole ] [ 1/2 ] [ 1/4 ] [ 1/8 ] [ 1/16 ] [ 1/32 ] [ 1/64 ] [1/128 ]
Row 4  [ One  ] [Gated] [Loop ] [Rev- ] [ rate bar (tune bar in Loop mode) ]
       [ Shot ] [     ] [     ] [erse ]
```

| Pads | Function | Colours |
|---|---|---|
| Rows 1-2 | Start of the section, in sixteenths of the sample. Hold two pads to start at the first and stretch the section to the last. | Start pad white while a mode is on (sky blue when off); the rest of the section dim sky blue. |
| Row 3 | Section length, as a fraction of the whole sample. | Selected length amber, others dim amber. |
| (4,1) One Shot | Stutter: replays the section from its start at the rate set by the bar. | Mode colours (above). |
| (4,2) Gated | As One Shot, but each repeat is cut off halfway to the next. | Mode colours. |
| (4,3) Loop | Loops the section continuously. Moving the start or length changes the loop without restarting it. | Mode colours. |
| (4,4) Reverse | Plays backwards. A running loop restarts in the new direction. | Tan on, dim tan off. |
| (4,5)-(4,8) | **One Shot / Gated:** repeat rate, slowest on the left: 400, 150, 55 and 25 ms between repeats. **Loop:** tune: -12, 0, +12 and +24 semitones. | A bar in eight half-pad steps: amber full, dim amber half. |

| Encoder | Function | LED |
|---|---|---|
| 1 Tune | Pitch, ±24 semitones (three detents per semitone). In Loop mode this is also the loop's speed. | Tan when tuned, dim tan at 0 |
| 2 | Section start, in fine steps (the length is kept). | Sky blue |
| 3 | Section length, in fine steps. | Amber |
| 4 | **One Shot / Gated:** repeat interval, 20-1000 ms (clockwise is faster). **Loop:** tune, the same as encoder 1. | Amber |

## Grain page

The grain engine plays many short, overlapping, faded snippets ("grains")
taken from around the chosen position. Settings range from a smooth, frozen
texture to a scattered cloud. Position, tune and Reverse are shared with the
looper page.

```
         1       2       3       4       5       6       7       8
Row 1  [ position: sixteenths 1-8 of the sample                           ]
Row 2  [ position: sixteenths 9-16                                        ]
Row 3  [400 ms] [250 ] [ 160 ] [ 100 ] [  60  ] [  40  ] [  24  ] [16 ms ]
Row 4  [Scat- ] [Shape] [Loop ] [Rev- ] [ density bar                      ]
       [ ter  ] [     ] [     ] [erse ]
```

| Pads | Function | Colours |
|---|---|---|
| Rows 1-2 | Where in the sample the grains come from. | Selected pad white while grains are on (sky blue when off). |
| Row 3 | Grain length. Long grains sound smooth; short ones buzzy and grainy. | Selected amber, others dim amber. |
| (4,1) Pitch scatter | Cycles **off** (dim purple) → **octaves** (purple): about a quarter of the grains play an octave down and a quarter an octave up → **octaves + fifths** (pink): as octaves, plus another quarter a fifth up. | **Red** means the grain engine is not running on the DSP. |
| (4,2) Shape | Each grain's fade. Cycles **smooth** (dim orange): a soft rise and fall, the classic cloud → **percussive** (orange): a fast attack and decaying tail, for plucky, rhythmic grains → **swell** (rose): a slow rise and quick end, like reversed plucks. | As listed. |
| (4,3) Loop | Grains on and off, using the mode cycle. | Mode colours. |
| (4,4) Reverse | Grains play backwards. | Tan on, dim tan off. |
| (4,5)-(4,8) | Density: how many grains overlap, about ½, 1½, 3 and 6, sparsest on the left. | A bar in eight half-pad steps, as on the looper page. |

| Encoder | Function | LED |
|---|---|---|
| 1 Tune | Grain pitch, ±24 semitones. | Tan when tuned, dim tan at 0 |
| 2 | Position, in fine steps. | Sky blue |
| 3 | Grain length, in fine steps (14-400 ms). | Amber |
| 4 | Density, in fine steps (¼ to 8 overlapping grains). | Amber |
| 5 | Spray: moves each grain's start randomly by up to ±1 s around the position. Fully left is off. | Sky blue when on, dim sky blue off |

Up to 32 grains play at once across all tracks. If a dense cloud needs more,
new grains are skipped rather than glitching. Denser settings play each grain
quieter, so the overall level stays roughly even.

## Tips

- **Frozen pad:** grain page, a long grain (row 3, left), high density (4,8),
  the smooth shape and a little spray.
- **Rhythmic glitch:** looper page, Gated at a fast rate; or grain page,
  short percussive grains at low density.
- **Shimmer:** octaves + fifths scatter on a slow, dense cloud.
- **Playable granular instrument:** arm grains (press Loop until it is
  green), close the view, and play the track from Note view or a MIDI
  keyboard.
