# Maschine Mikro MK3 — Button → Functionality Mapping

> Track what is currently mapped, what is free, and what could be added.  
> Driver: `crates/maschine_library/src/controls.rs:4` (`Buttons` 0–40) + `crates/driver/src/main.rs` + `example_config.toml` (80 pages).

## Legend

* **Reserved (driver)** — button is intercepted by driver, does not send MIDI CC (used for paging, transpose, strip, chords, arp, gates). Change via `example_config.toml` or code.
* **Sends CC** — button sends MIDI CC (configurable in `[buttons]`), free to map in DAW.
* **Gate/Toggle** — `Gate` = `Bright` when held `Dim` when released (only active when held); `Toggle` = `Bright` when on `Dim` when off.

---

## Current Mapping (as shipped in `example_config.toml` + hard-coded driver logic)

| # | Button | Type | MIDI CC (example_config) | Status | Notes |
|---|--------|------|--------------------------|--------|-------|
| 0 | **Maschine** | Gate (arp) / CC | `CC 38` | **Reserved when arp on** `faster rate`, else CC | Arp `Maschine` = faster (next pure fraction `3/4..1/96`) `Bright` on press |
| 1 | **Star** | Gate (arp) / CC | `CC 39` | Reserved when arp on `slower rate`, else CC | Arp `Star` = slower |
| 2 | **Browse** | **Gate** | `CC 40` overridden | **Reserved** | Resets `arp` to `1/16` `Straight` `Bright` on press `Dim` on release |
| 3 | **Volume** | **Toggle sustain** | `CC 44` overridden | **Reserved** | `Bright` = `CC64 127` sustain on, `Dim` = `CC64 0` off (pad + midi ch) |
| 4 | **Swing** | **Gate arp swing** | `CC 46` overridden | **Reserved** | Cycles `Straight 50 → Light 55 → Medium 60 → Triplet 66.7` `Bright`/`Dim`, interval `long=rate*swing/50` `short=rate*(100-swing)/50` |
| 5 | **Tempo** | Gate (arp) / CC | `CC 48` | Reserved when arp on `Bright`, else CC | Free when arp off |
| 6 | **Plugin** | **Toggle hold/latch** | `CC 45` overridden | **Reserved** | `Bright` = hold note/chord (suppress `NoteOff`) + arp latch (keeps arpeggiating after release), `Dim` = release all held/latched |
| 7 | **Sampling** | Gate (arp) | `CC 47` | Reserved when arp on | Cycles `arp_octaves 1→2→3→4→1` `Bright`/`Dim` |
| 8 | **Left** | **Transpose −1** | – (reserved) | **Reserved** | `Shift+Left` = −12 octave, range `−48..+48`. Freed from CC. |
| 9 | **Right** | **Transpose +1** | – (reserved) | **Reserved** | `Shift+Right` = +12 |
| 10 | **Pitch** | **Strip → PitchBend** (3-way) | `CC 49` overridden | **Reserved** | `Bright` = PitchBend `0..16383` center `8192` spring dim on release. 3-way exclusive with Mod/Perform (`Dim` inactive). |
| 11 | **Mod** | **Strip → ModWheel** | `CC 50` overridden | **Reserved** | `Bright` = `CC1` hold, LEDs hold last position. |
| 12 | **Perform** | **Strip → Free** | `CC 51` overridden | **Reserved** | `Bright` = `CC16` free assignable hold, LEDs hold. 3-way exclusive. |
| 13 | **Notes** | Gate (arp) | `CC 52` | **Reserved** | Cycles `ArpMode Up→Down→UpDown→DownUp→Random` `Bright`/`Dim` |
| 14 | **Group** | **Page 1-16** | `CC 34` overridden | **Reserved** | `Group` tap cycles `1-16`, hold `Group`+pad selects `1-16`. Screen `n/64`. |
| 15 | **Auto** | **Page 17-32** | `CC 35` overridden | **Reserved** | `Auto` tap cycles `17-32`, hold `Auto`+pad selects `17-32`. Count limited to 16. |
| 16 | **Lock** | **Page 33-48** | `CC 36` overridden | **Reserved** | `Lock` tap cycles `33-48`, hold `Lock`+pad selects `33-48`. |
| 17 | **NoteRepeat** | **Toggle arp** | `CC 37` | **Reserved** | `Bright` = `arp_enabled`, `Dim` = off |
| 18 | **Restart (Loop)** | **Gate sus4** | `CC 53` overridden | **Reserved** | Held `Bright` `root+5+7` sus4, `Dim` off |
| 19 | **Erase** | **Gate sus2** | `CC 54` overridden | **Reserved** | Held `Bright` `root+2+7` sus2 |
| 20 | **Tap** | **Gate dim** | `CC 55` overridden | **Reserved** | Held `Bright` `root+3+6` dim |
| 21 | **Follow** | **Gate aug** | `CC 56` overridden | **Reserved** | Held `Bright` `root+4+8` aug |
| 22 | **Play** | CC / Mackie | `CC 57` | Sends CC or Mackie `Note 94` if `daw_mackie=true` | |
| 23 | **Rec** | CC / Mackie | `CC 58` | Sends CC or `Note 95` | |
| 24 | **Stop** | CC / Mackie | `CC 59` | Sends CC or `Note 93` | |
| 25 | **Shift** | Modifier | – (no CC) | **Reserved** | Held with `Left`/`Right` for octave |
| 26 | **FixedVol** | **Toggle** | `CC 80` | **Reserved** | `Bright` = `127` fixed, `Dim` = velocity `val>>5` |
| 27 | **PadMode** | **Page 65-80 + Gate** | `CC 81` overridden | **Reserved** | Tap cycles `65-80` (65th page = index 64), hold `PadMode`+pad selects `65-80` `Bright`/`Dim`; gate: held `Bright` switches pads to drum kit `drum_pages[current_page%16]` |
| 28 | **Keyboard** | **Page 49-64 + Gate** | `CC 82` overridden | **Reserved** | Tap cycles `49-64` (49th page = index 48), hold `Keyboard`+pad selects `49-64` `Bright`/`Dim`; gate: held `Bright` switches pads to 16 extra scales from SCALES repo (`major blues`/`bebop`/`diminished`/`lydian dominant`/`altered`/`dorian b2`/`ultralocrian`/`augmented heptatonic`/`whole tone`/`locrian major`/`double harmonic lydian`/`enigmatic`/`major augmented`/`messiaen #4`/`composite blues`/`lydian augmented`) |
| 29 | **Chords** | **Toggle** | `CC 84` overridden | **Reserved** | `Bright` = triads/power per `[chord_types]` (7-tone `1-3-5`, non-7 `root+7`), exclusive with `Step` |
| 30 | **Step** | **Toggle** | `CC 83` overridden | **Reserved** | `Bright` = tetrad `1-3-5-7` for 7-tone / `power+oct` `1-5-8` for non-7, exclusive with `Chords` |
| 31 | **Scene** | **Gate** | `CC 85` overridden | **Reserved** | Held `Bright` `triad→tetrad` `Dim` off |
| 32 | **Pattern** | **Gate** | `CC 86` overridden | **Reserved** | `tetrad→triad` |
| 33 | **Events** | **Gate** | `CC 87` overridden | **Reserved** | `major→minor` (3rd `4→3`, tetrad also 7th `11→10`) |
| 34 | **Variation** | **Gate** | `CC 88` overridden | **Reserved** | `minor→major` (`3→4`, `10→11`) |
| 35 | **Duplicate** | **Gate** | `CC 89` overridden | **Reserved** | `any→5ths` `root+7` |
| 36 | **Select** | **Gate** | `CC 90` overridden | **Reserved** | `any→5th+oct` `root+7+12` |
| 37 | **Solo** | **Gate** | `CC 91` overridden | **Reserved** | `+9th` `root+14` additive, sorts/dedups |
| 38 | **Mute** | **Gate** | `CC 92` overridden | **Reserved** | `+11th` `root+17` additive |
| 39 | **EncoderPress** | CC | `CC 8` | Sends CC | Encoder itself `CC 7` `relative` |

**Pads 1-16** — `pad_pages` 80 (`Chromatic` … `Romanian Minor` `0-47` + 16 keyboard `48-63` + drums `64-79`: `65` Micro Drum Sampler, `66` MT Power Kit, `67` General MIDI, `68` Salamander Kit, `69-80` generic), `Group`/`Auto`/`Lock`/`PadMode` paging, `transpose_offset` `−48..+48`, `Chords`/`Step` + gates (`Scene` etc.) + `Solo`/`Mute` extensions + `arp` + `FixedVol`. Pads `Dim` default, `Normal` on press, `Dim` on release; page selector `Bright`/`Dim` 700 ms.

**Encoder** (`buf[7]`) — `CC 7` `relative` (1 cw / 127 ccw) filtered `±1/±2`.  
**Touch Strip** (`buf[10]`) — `PitchBend` (on `pad_channel` + mirror, `0..16383`, center `8192`) when `Pitch` latched, else `CC1` modwheel / `CC16` free, 25 LEDs follow position (spring vs hold).

---

## What is Free?

* **Truly free (no driver logic):** `EncoderPress/Touch` — all send CC and can be remapped in DAW or repurposed by adding a new check in `main.rs`.
* **Reserved (driver):** `Group`, `Auto`, `Lock`, `PadMode` (paging + gate), `Pitch`, `Mod`, `Perform` (strip 3-way), `Left`, `Right`, `Shift` (transpose), `Chords`, `Step` (toggles), `Scene`, `Pattern`, `Events`, `Variation`, `Duplicate`, `Select`, `Solo`, `Mute` (gates), `NoteRepeat`, `Notes`, `Maschine`, `Star`, `Sampling`, `Swing` (arp), `FixedVol`, `Tempo` (when arp on). Change via `example_config.toml` or code.

---

## How to Repurpose a Free Button (example)

To make `Browse` toggle `fixed velocity`:

1. Add to `main.rs` like `Chords`:
   ```rust
   } else if status && button == Buttons::Browse {
       fixed_vel = !fixed_vel;
       println!("FixedVol {}", fixed_vel);
       lights.set_button(Buttons::Browse, if fixed_vel { Bright } else { Dim });
   }
   ```
2. In pad handling, `let scaled_vel = if fixed_vel { 127 } else { (val>>5).min(127) };`
3. Or simply map `Browse` to CC and handle in DAW.

Same pattern works for any button.

---

*Last updated: 2026-09-19 — driver `main.rs` 80 pages, `PadMode` 49-64 `Keyboard` 65-80, strip 3-way `Pitch/Mod/Perform` (`Dim` inactive), `Left/Right` transpose + `Shift` octave, `Chords`/`Step` + 8 gates, `Solo`/`Mute` +9th/+11th, `NoteRepeat`/`Notes`/`Maschine`/`Star`/`Sampling`/`Swing` arp (18 rates `3/4..1/96`, swing 50/55/60/66.7), `FixedVol`, `Group`/`Auto`/`Lock`/`PadMode` paging.*
