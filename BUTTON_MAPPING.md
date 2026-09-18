# Maschine Mikro MK3 — Button → Functionality Mapping

> Track what is currently mapped, what is free, and what could be added.  
> Driver: `crates/maschine_library/src/controls.rs:4` (`Buttons` 0–40) + `crates/driver/src/main.rs` + `example_config.toml`.

## Legend

* **Reserved (driver)** — button is intercepted by driver, does not send MIDI CC (used for paging, transpose, strip toggle, chords, etc.). Change via `example_config.toml` `[transpose]`, `pad_page_button`, `auto_page_button` or code.
* **Sends CC** — button sends MIDI CC (configurable in `[buttons]`), free to map in DAW.
* **Free** — not mapped in `example_config.toml`; driver falls back to defaults (`crates/driver/src/settings.rs:237`) but still sends CC, or `type = "off"` to disable.

---

## Current Mapping (as shipped in `example_config.toml` + hard-coded driver logic)

| # | Button | Driver Reserved | MIDI CC (example_config) | Status | Notes |
|---|--------|-----------------|--------------------------|--------|-------|
| 0 | **Maschine** | – | `CC 38` | Sends CC | Free for DAW; could be `Shift` modifier for alt paging |
| 1 | **Star** | – | `CC 39` | Sends CC | Free |
| 2 | **Browse** | – | `CC 40` | Sends CC | Could open browser / plugin picker |
| 3 | **Volume** | – | `CC 44` | Sends CC | Free |
| 4 | **Swing** | – | `CC 46` | Sends CC | Free |
| 5 | **Tempo** | – | `CC 48` | Sends CC | Free; *suggested: tap-tempo* |
| 6 | **Plugin** | – | `CC 45` | Sends CC | Free |
| 7 | **Sampling** | – | `CC 47` | Sends CC | Free |
| 8 | **Left** | **Transpose −1 semitone** (`Right` = +1) | – (reserved, no CC) | Reserved | `Shift+Left` = −12 octave (fixed reverse, was `Left=+1` before). Freed from CC. |
| 9 | **Right** | **Transpose +1 semitone** | – (reserved) | Reserved | `Shift+Right` = +12 octave |
| 10 | **Pitch** | **Strip → PitchBend** (latched) | `CC 49` overridden | Reserved | Press `Pitch` → strip = `PitchBend` (center on release), `Pitch` LED `Bright`. Original Maschine: toggles strip. Free `Mod` for modwheel. |
| 11 | **Mod** | **Strip → ModWheel** (latched) | `CC 50` overridden | Reserved | Press `Mod` → strip = `CC1` modwheel, `Mod` LED `Bright`. |
| 12 | **Perform** | – | `CC 51` | Sends CC | Free |
| 13 | **Notes** | – | `CC 52` | Sends CC | Free |
| 14 | **Group** | **Page 1-16** (`Group+Pad`) | `CC 34` overridden | Reserved | `Group` tap cycles `1-16`, hold `Group`+pad `0-15` selects `1-16`. Screen `1/32`. |
| 15 | **Auto** | **Page 17-32** (`Auto+Pad`) | `CC 35` overridden | Reserved | `Auto` tap cycles `17-32`, hold `Auto`+pad selects `17-32`. Requires `pad_pages` ≥17; 32 is hard limit (`settings.rs:345` `>32`). |
| 16 | **Lock** | – | `CC 36` | Sends CC | Free |
| 17 | **NoteRepeat** | – | `CC 37` | Sends CC | Free |
| 18 | **Restart** | – | `CC 53` | Sends CC | Free |
| 19 | **Erase** | – | `CC 54` | Sends CC | Free |
| 20 | **Tap** | – | `CC 55` | Sends CC | Free |
| 21 | **Follow** | – | `CC 56` | Sends CC | Free; could be `follow playhead` |
| 22 | **Play** | – | `CC 57` | Sends CC | Free (DAW transport) |
| 23 | **Rec** | – | `CC 58` | Sends CC | Free (DAW transport) |
| 24 | **Stop** | – | `CC 59` | Sends CC | Free (DAW transport) |
| 25 | **Shift** | **Modifier** for `Shift+Left/Right` octave | – (no CC) | Reserved | Held with `Left`/`Right` to get octave transpose. Could also be `Shift+Pad` for alt functions. |
| 26 | **FixedVol** | – | `CC 80` | Sends CC | Free |
| 27 | **PadMode** | – | `CC 81` | Sends CC | Free; could be `pad page` alt |
| 28 | **Keyboard** | – | `CC 82` | Sends CC | Free |
| 29 | **Chords** | **Chords toggle** (diatonic triads) | `CC 84` overridden | Reserved | Press `Chords` toggles `triads` (`Bright` = triads, `Dim` = single). Uses `pad 1,3,5` for 7-tone, `power` (`root+7`) for non-7 (chromatic/pentatonic) — configurable via `[chord_types]` (see below). Transpose shifts entire triad. |
| 30 | **Step** | – | `CC 83` | Sends CC | Free |
| 31 | **Scene** | – | `CC 85` | Sends CC | Free |
| 32 | **Pattern** | – | `CC 86` | Sends CC | Free |
| 33 | **Events** | – | `CC 87` | Sends CC | Free |
| 34 | **Variation** | – | `CC 88` | Sends CC | Free |
| 35 | **Duplicate** | – | `CC 89` | Sends CC | Free |
| 36 | **Select** | – | `CC 90` | Sends CC | Free |
| 37 | **Solo** | – | `CC 91` | Sends CC | Free |
| 38 | **Mute** | – | `CC 92` | Sends CC | Free |
| 39 | **EncoderPress** | – | `CC 8` | Sends CC | Free; encoder itself sends `CC 7` (`relative` 1/127) |
| 40 | **EncoderTouch** | – | disabled (`CC 9` disabled) | Free (disabled) | Could be used for `encoder touch` modifier |

**Pads 1-16** (`PadEventType:4`) — `pad_pages` 32 scales (`Chromatic` … `Iwato` + `Up` octave), `Group`/`Auto` paging, `transpose_offset` −48..+48 via `Left`/`Right`, `Chords` triads/power. All pads lit `Blue Normal` by default, `Bright` on press, `Dim` on release (selector `Bright`/`Dim` 700 ms).

**Encoder** (`buf[7]`) — `CC 7` `relative` (1 cw / 127 ccw).  
**Touch Strip** (`buf[10]`) — `PitchBend` (on `pad_channel` + mirror to `midi_channel`, full `0..16383`, center `8192` on release) when `Pitch` latched, else `CC 1` modwheel. 25 LEDs follow position.

---

## What is Free?

* **Truly free (no driver logic):** `Maschine`, `Star`, `Browse`, `Volume`, `Swing`, `Tempo`, `Plugin`, `Sampling`, `Perform`, `Notes`, `Lock`, `NoteRepeat`, `Restart`, `Erase`, `Tap`, `Follow`, `Play/Rec/Stop`, `FixedVol`, `PadMode`, `Keyboard`, `Step`, `Scene`, `Pattern`, `Events`, `Variation`, `Duplicate`, `Select`, `Solo`, `Mute`, `EncoderPress/Touch` — all send CC and can be remapped in DAW or repurposed in driver by adding a new reserved check in `main.rs:588` (like `Chords`/`Pitch`).
* **Reserved (driver):** `Group`, `Auto`, `Pitch`, `Mod`, `Left`, `Right`, `Shift` (modifier), `Chords` — do not send MIDI CC when used as configured. Disable by setting `pad_page_button=""`, `auto_page_button=""`, `[transpose] semitone_up=""` etc., or `Pitch`/`Mod` handling in `main.rs:580`.

---

## Suggested Driver-Level Functionality (all possible in `main.rs` + `settings.rs`)

> These are not yet mapped, but trivial to add (follow `Transpose`/`Chords` pattern: add `settings.rs` field, `button_debug_name` check, state var, LED, screen).

| Buttons | Suggested Function | Why / How |
|---------|-------------------|-----------|
| **Tempo + Tap** | Tap-tempo (avg interval of `Tap` presses → set `Clock` or CC) | Already `Tap`/`Tempo` free, driver can compute BPM and optionally send `MIDI Clock`/`CC` |
| **Shift + Encoder** | Fine transpose (±1) vs coarse (±12) | Alternative to `Shift+Left/Right` |
| **Browse + Encoder** | Scale selection (cycle `pad_pages` without `Group`) | `Browse` is free, could be `Browse+Pad` for scale browser |
| **Perform + Pad** | Quick scale preview (play scale notes) | `Perform` free |
| **Notes + Pad** | Toggle `pad_aftertouch` `poly`/`channel`/`cc` | `Notes` free |
| **Solo/Mute + Pad** | Mute/solo per pad (send `CC` per pad or filter) | `Solo`/`Mute` free, driver can filter `NoteOn` |
| **Select + Pad** | Select drum kit / page color per pad | `Select` free |
| **Pattern/Scene + Pad** | Direct page jump 1-16/17-32 without `Group`/`Auto` hold | `Pattern`/`Scene` free, could be `Pattern+Pad` → `C0` etc. |
| **FixedVol** | Toggle velocity fixed 127 vs velocity-sensitive | `FixedVol` free, driver can force `vel=127` when lit |
| **NoteRepeat + Pad** | Arp rate (1/4, 1/8…) | `NoteRepeat` free |
| **Lock** | Lock current `transpose`/`chords` state (prevent accidental change) | `Lock` free |
| **Swing** | Swing amount (CC) via encoder while held | `Swing` free + encoder |
| **Volume + Slider** | Slider controls volume vs modwheel (like `Pitch`/`Mod` toggle) | `Volume` free |
| **Maschine/Star** | Shift modifiers for alt layers (e.g. `Maschine+Left` = transpose reset) | `Maschine`/`Star` free, currently `Maschine` is just CC |
| **EncoderPress** | Reset transpose (`-transpose_offset`) or reset strip to center | Currently `CC 8`, could be `transpose reset` (set `[transpose] reset="EncoderPress"`) |
| **EncoderTouch** | Momentary pitchbend enable (only bend while touching encoder) | Currently disabled, could be used |

**Already implemented via config, no code change:**

* Any free button → assign any `CC`/`Note`/`PC` in `[buttons]` (e.g. `Left = {type="cc", cc=1}` if you disable `[transpose]`).
* `Chords` voicing per scale: `[chord_types]` `Chromatic="power"`, `Major="triad"` (default 7-tone triad, non-7 power), set to `"tetrad"` for 7th chords (`1-3-5-7`).

---

## How to Repurpose a Free Button (example)

To make `Browse` toggle `fixed velocity`:

1. Add to `settings.rs` `browse_fixed: bool` or just handle in `main.rs` like `Chords`:
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

*Last updated: 2026-09-18 — driver `main.rs:580` strip toggle, `transpose` `Left`/`Right` + `Shift` octave, `Chords` triads/power via `[chord_types]`, `Group`/`Auto` 32 pages.*
