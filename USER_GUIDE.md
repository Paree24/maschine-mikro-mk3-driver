# Maschine Mikro MK3 — User Guide (Linux Driver)

This guide covers install, run, configure and customize the userspace MIDI driver for the **Native Instruments Maschine Mikro MK3** on Linux. The driver exposes all controls via a virtual MIDI port — pads, buttons, encoder and touch strip — with the same “pages” workflow you have on Windows in Controller Editor, plus extended scales/chords/arp.

---

## 1. Quick Start

### Dependencies

```bash
# Debian / Ubuntu
sudo apt install build-essential pkg-config libasound2-dev libjack-dev libusb-1.0-0-dev libudev-dev
# Fedora / RHEL
sudo dnf install @development-tools alsa-lib-devel jack-audio-connection-kit-devel libusb-devel systemd-devel
# Arch
sudo pacman -S base-devel alsa-lib pipewire-jack libusb systemd-libs
```

### Install & Run

```bash
git clone https://github.com/Paree24/maschine-mikro-mk3-driver.git
cd maschine-mikro-mk3-driver
sudo cp 98-maschine.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger

cargo run --release              # ALSA backend (default)
cargo run --release --features jack  # JACK backend (compile-time choice)
```

On start the controller:

* runs a self-test (`00 → 03` on screen, all LEDs cycle),
* shows `1/64` (or `1/48` etc.) on the screen,
* creates a virtual port `Maschine Mikro MK3 MIDI Out` (`client_name` / `port_name` in config).

Point your DAW / Hydrogen / REAPER / Ardour at that port.

> **No hardware?** The driver still validates the config and prints `Running with settings:` / `Effective pad_pages (64 pages)` before trying to open USB. Useful for dry-run checks.

### With a Custom Config

```bash
cargo run --release -- -c example_config.toml
cargo run --release -- -c /path/to/my.toml
```

All keys are optional — missing values inherit built-in defaults (`crates/driver/src/settings.rs:248`). See `example_config.toml` (now 64 pages, fully commented). Systemd user service: `~/.config/systemd/user/maschine-mikro.service` → `cargo build --release && systemctl --user restart maschine-mikro.service`.

---

## 2. Hardware Overview

```
[ Maschine] [Star] [Browse] [Volume] | [Swing] [Tempo] [Plugin] [Sampling]
[ Left ] [Right] [Pitch] [Mod] | [Perform] [Notes] [Group] [Auto]
[ Lock] [NoteRepeat] [Restart] [Erase] | [Tap] [Follow] [Play] [Rec] [Stop]
[ Shift] [FixedVol] [PadMode] [Keyboard] [Chords] [Step] [Scene] [Pattern] [Events] [Variation] [Duplicate] [Select] [Solo] [Mute]
                         [ Push-Encoder (press + touch) ]   [ Touch Strip 25 LEDs ]
                         [ 16 Velocity Pads with RGB + 4-level brightness ]
                         [ 128×64 mono screen ]
```

* **16 pads** — velocity + aftertouch, RGB (`Off, Red, Orange, LightOrange, WarmYellow, Yellow, Lime, Green, Mint, Cyan, Turquoise, Blue, Plum, Violet, Purple, Magenta, Fuchsia, White`) + 4 brightness levels (`Off, Dim, Normal, Bright`).
* **39 button LEDs + 25 strip LEDs** (`crates/maschine_library/src/lights.rs:53` — every button except `EncoderPress/Touch` has LED).
* Screen is a 128×32 buffered mono display (`crates/maschine_library/src/screen.rs:3`).

---

## 3. MIDI Concepts & Defaults

### 3.1 Channels

* `midi_channel` — default for **buttons / encoder / strip** (`0` → `Ch 1`). Range `0–15`.
* `pad_channel` — dedicated pad channel, default `9` → `Ch 10` (GM drums). Keeps drums separate.

Override per-control with `channel = N`.

### 3.2 Message Types

| Control | Types (`type = ...`) | MIDI sent |
|---------|----------------------|-----------|
| Button | `cc` (default), `note`, `pc`, `off` | `CC` `127` on press / `0` on release; `NoteOn vel 127` / `NoteOff`; `ProgramChange` on press |
| Pad | fixed `NoteOn/NoteOff` per page + optional aftertouch | `NoteOn/NoteOff` on `pad_channel`; `poly` → `Poly Aftertouch`, `channel` → `Channel Aftertouch`, `cc` → `CC 74`, `off` → nothing |
| Encoder | `cc` + `mode` | `relative` → `CC 1` (cw) / `127` (ccw) per tick; `absolute` → `0–127` |
| Strip | `cc` / `pitchbend` + `mode` | `PitchBend` → 14-bit `0..16383` center `8192`; `ModWheel` `CC1` hold; `Free` `CC16` hold (see §4.5) |

Per-button `value_press` / `value_release` customizes CC velocities (defaults `127/0`).

---

## 4. Current Default Mapping

Defaults defined in `crates/driver/src/settings.rs:277` and `example_config.toml`.

### 4.1 Pads — 80 Pages (48 scales + 16 drums + 16 keyboard)

`pad_pages` is 80 pages by default (max 80). Page `0` is startup. `example_config.toml` ships 48 scales + 16 keyboard scales (from SCALES repo) + 16 drum pages (`65` Micro Drum Sampler, `66` MT Power Kit, `67` General MIDI, `68` Salamander Kit, `69-80` generic).

| Bank | Button | Pages | Purpose |
|------|--------|-------|---------|
| **Group** | `Group+Pad` / tap `Group` | 1-16 (0-15) | Scales 1-16 (Chromatic … Iwato) |
| **Auto** | `Auto+Pad` / tap `Auto` | 17-32 (16-31) | Scales 17-32 (Kumoi … Romanian Minor) |
| **Lock** | `Lock+Pad` / tap `Lock` | 33-48 (32-47) | Scales 33-48 (… up) |
| **PadMode** | `PadMode+Pad` / tap `PadMode` | 65-80 (64-79) | `65` Micro Drum Sampler (exact NCC), `66` MT Power Kit (GM core), `67` General MIDI 35-50, `68` Salamander Kit (Sforzando), `69-80` generic kits — gate unlocks |
| **Keyboard** | `Keyboard+Pad` / tap `Keyboard` | 65-80 (64-79) | Keyboard 1-16 (repo scales: `major blues`/`bebop`/`diminished`/`lydian dominant`/`altered`/`dorian b2`/`ultralocrian`/`augmented heptatonic`/`whole tone`/`locrian major`/`double harmonic lydian`/`enigmatic`/`major augmented`/`messiaen #4`/`composite blues`/`lydian augmented`) — gate unlocks |

`pad_page_button="Group"`, `auto_page_button="Auto"`, `lock_page_button="Lock"`, padmode hard-coded to `48`. Hold-select: `pad_page_hold_select=true` (hold button + tap pad selects directly, tap alone cycles). Set `pad_page_button=""` to disable paging.

Legacy `notemaps = [16 notes]` still supported — wraps as one page.

**Velocity:** `vel = max(1, val>>5)` from raw `0–2047`, or `127` when `FixedVol` toggled. Aftertouch `poly/channel/cc/off`.

**Colors:** `pad_colors` (16 per-pad) or `pad_page_colors` (64 per-page, `Red,Orange,...White` cycle). Pads `Dim` by default, `Normal` on press, `Dim` on release; page selector `Bright` current vs `Dim` others for 700 ms, then `Dim`.

### 4.2 Page Switching (Windows parity)

* **Hold `Group` + tap Pad** → jump to `1-16`. **Tap `Group`** → cycle `1→2→…→16→1` (or next in bank if `hold_select` and already in bank, with correct first-entry to `1`/`17`/`33`/`49`).
* **Hold `Auto` + pad** → `17-32`, tap `Auto` cycles `17-32`.
* **Hold `Lock` + pad** → `33-48`, tap `Lock` cycles `33-48`.
* **Hold `PadMode` + pad** → `49-64` (49th page = index 48, first press from outside goes to 49), tap `PadMode` cycles `49-64`. `PadMode` also gates drum note mapping (see §4.7).
* Screen shows `n/64` (or `n/48` if only 48 pages configured).

Paging buttons are reserved — won’t send MIDI when paging is active.

### 4.3 Buttons — Toggles vs Gates

| Button | Type | MIDI CC (example_config) | Status | Notes |
|--------|------|--------------------------|--------|-------|
| `Maschine` | Gate (arp) / CC | `CC 38` | **Reserved when arp on** `faster rate`, else sends CC | Arp rate `faster` (next pure fraction `3/4..1/96`) |
| `Star` | Gate (arp) / CC | `CC 39` | Reserved when arp on `slower rate`, else CC | Arp rate `slower` |
| `Browse` | **Gate** | `CC 40` overridden | **Reserved** | Resets `arp` to `1/16` `Straight` `Bright` on press `Dim` on release |
| `Volume` | **Toggle sustain** | `CC 44` overridden | **Reserved** | `Bright` = `CC64 127` sustain, `Dim` = off |
| `Swing` | **Gate** arp swing | `CC 46` | **Reserved** | Cycles `Straight 50 / Light 55 / Medium 60 / Triplet 66.7` `Bright` on press `Dim` on release |
| `Tempo` | Gate / CC | `CC 48` | Reserved when arp on (Bright), else CC | Free when arp off |
| `Plugin` | **Toggle hold/latch** | `CC 45` overridden | **Reserved** | `Bright` = hold note/chord + arp latch, `Dim` = release all |
| `Sampling` | Gate (arp) | `CC 47` | Reserved when arp on | Cycles `arp_octaves 1→2→3→4→1` |
| `Left` | **Transpose −1** | – (reserved) | Reserved | `Shift+Left` = −12 octave, range `−48..+48` |
| `Right` | **Transpose +1** | – (reserved) | Reserved | `Shift+Right` = +12 |
| `Pitch` | **Strip → PitchBend** (3-way) | `CC 49` overridden | Reserved | `Bright` = PitchBend spring to `8192` center, 25 LEDs dim on release |
| `Mod` | **Strip → ModWheel** | `CC 50` overridden | Reserved | `Bright` = `CC1` hold, LEDs hold last position |
| `Perform` | **Strip → Free** | `CC 51` overridden | Reserved | `Bright` = `CC16` free assignable hold, LEDs hold; 3-way exclusive with Pitch/Mod (`Dim` when inactive) |
| `Notes` | Gate (arp) | `CC 52` | Reserved | Cycles `ArpMode Up→Down→UpDown→DownUp→Random` `Bright` on press |
| `Group` | **Page 1-16** | `CC 34` overridden | Reserved | See §4.2 |
| `Auto` | **Page 17-32** | `CC 35` overridden | Reserved | |
| `Lock` | **Page 33-48** | `CC 36` overridden | Reserved | |
| `NoteRepeat` | **Toggle** arp | `CC 37` | Reserved | Toggles `arp_enabled` `Bright` on / `Dim` off |
| `Restart (Loop)` | **Gate sus4** | `CC 53` overridden | **Reserved** | Held `Bright` `root+5+7` |
| `Erase` | **Gate sus2** | `CC 54` overridden | **Reserved** | Held `Bright` `root+2+7` |
| `Tap` | **Gate dim** | `CC 55` overridden | **Reserved** | Held `Bright` `root+3+6` |
| `Follow` | **Gate aug** | `CC 56` overridden | **Reserved** | Held `Bright` `root+4+8` |
| `Play/Rec/Stop` | CC / Mackie | `CC 57/58/59` | Sends CC or Mackie `Note 94/95/93` if `daw_mackie=true` | |
| `Shift` | Modifier | – (no CC) | Reserved | Held with `Left/Right` for octave |
| `FixedVol` | **Toggle** | `CC 80` | Reserved | `Bright` = `127` fixed, `Dim` = velocity |
| `PadMode` | **Page 49-64 + Gate** | `CC 81` overridden | Reserved | Tap cycles `49-64`, hold+pad selects `49-64`, gate `Bright` held drums `Dim` released |
| `Keyboard` | **Page 65-80 + Gate** | `CC 82` overridden | **Reserved** | Tap cycles `65-80`, hold `Keyboard`+pad selects `65-80`, gate `Bright` switches pads to 16 extra scales (see §4.1) |
| `Chords` | **Toggle** | `CC 84` overridden | Reserved | `Bright` = triads/power (configurable `[chord_types]`), mutually exclusive with `Step` |
| `Step` | **Toggle** | `CC 83` overridden | Reserved | `Bright` = tetrad `1-3-5-7` for 7-tone / `power+oct` `1-5-8` for non-7, exclusive with `Chords` |
| `Scene` | **Gate** | `CC 85` | Reserved | Held `Bright` `triad→tetrad` `Dim` off |
| `Pattern` | **Gate** | `CC 86` | Reserved | `tetrad→triad` |
| `Events` | **Gate** | `CC 87` | Reserved | `major→minor` (3rd `4→3`, for tetrad also `7th 11→10`) |
| `Variation` | **Gate** | `CC 88` | Reserved | `minor→major` (`3→4`, `10→11`) |
| `Duplicate` | **Gate** | `CC 89` | Reserved | `any→5ths` `root+7` |
| `Select` | **Gate** | `CC 90` | Reserved | `any→5th+oct` `root+7+12` |
| `Solo` | **Gate** | `CC 91` | Reserved | `+9th` `root+14` additive |
| `Mute` | **Gate** | `CC 92` | Reserved | `+11th` `root+17` additive |
| `EncoderPress` | CC | `CC 8` | Sends CC | Encoder itself `CC 7` |

All gates: `Bright` when held, `Dim` when released, only active when held. Toggles: `Bright` when on, `Dim` when off. Free buttons send CC and can be remapped or repurposed by adding a new check in `main.rs`.

### 4.4 Encoder

`[encoder] cc=7 mode=relative` on `midi_channel`. HID delta `1`→CW, `0xFF`(-1)→CCW → emits `CC7=1` or `127` per tick. `mode=absolute` for `0–127`. `should_handle_encoder_rotation` filters to `±1/±2`.

### 4.5 Touch Strip (Slider) — 3-Way Exclusive

`[slider] cc=1 mode=absolute` on `midi_channel` (but driver overrides).

* **Pitch** `Bright` → `PitchBend` on `pad_channel` + mirror to `midi_channel`, `0..16383` (`val*16383/127`), center `8192` on finger lift, 25 LEDs dim on release (spring).
* **Mod** `Bright` → `CC1` modwheel on `midi_channel`, hold last value, LEDs hold last position (manual reset).
* **Perform** `Bright` → `CC16` free assignable on `midi_channel`, hold, LEDs hold. Inactive modes `Dim`. Exactly one `Bright`.

Raw `1–200` → `0–127`.

### 4.6 Transpose

`[transpose] semitone_up="Right" +1, semitone_down="Left" -1, Shift+Left/Right ±12 octave`, clamped `−48..+48`, shifts pad notes (including chords). `Left/Right` LEDs `Bright` on press `Dim` on release. `Shift` is modifier.

### 4.7 Chords / Scales

80 scales (48 scales + 16 drums + 16 keyboard) base `C1=24` 2 octaves, per-page `pad_pages` 16 notes driver-permuted `[13,14,15,16,12,11,10,9,8,7,6,5,1,2,3,4]` → `phys` sequential. `triad_for_pad` derives intervals from all 16 notes `mod12` sorted.

* **Chords toggle** `Bright`: 7-tone → `triad 1-3-5` (`root, third+2, fifth+4` degrees), non-7 → `power root+7`, configurable via `[chord_types]` `Chromatic="power"`, `Major="triad"` or `"tetrad"` (`1-3-5-7`).
* **Step toggle** `Bright`: 7-tone → `tetrad 1-3-5-7` (via `tetrad` override), non-7 → `power+oct 1-5-8` (`root,fifth,octave`). Mutually exclusive with `Chords`.
* **Gate modifiers** (held): `Scene` `triad→tetrad`, `Pattern` `tetrad→triad`, `Events` `major→minor` (and 7th `11→10` for tetrad), `Variation` `minor→major` (`10→11`), `Duplicate` `any→5ths`, `Select` `any→5th+oct`, `Solo` `+9th`, `Mute` `+11th`. All momentary, sorted/deduped.

### 4.8 Arpeggiator (DAW BPM sync)

* New port `Maschine Mikro MK3 MIDI In` — route your DAW's MIDI Clock output to it (`aconnect` / patchbay) and press play. The arp follows DAW BPM (smoothed over 24 clock ticks, `20-300` range); all rates (`3/4..1/96`) scale automatically. Transport Start/Continue resyncs phase, Stop pauses the arp. There is NO silent fallback: without clock lock the arp stays frozen and logs `arp waiting for DAW MIDI Clock` (every 5s); clock loss (>2s no ticks) pauses it. `bpm = 120.0` is used only with `arp_sync = false`.
* `NoteRepeat` toggle `Bright` = `arp_enabled`. `Notes` cycles `Up→Down→UpDown→DownUp→Random`.
* `Maschine` (when arp on) = faster (next pure fraction), `Star` = slower (18 fractions `3/4 … 1/96` grouped descending, `3/4,1/2,3/8,1/3...`).
* `Sampling` cycles `arp_octaves 1→4` (cyclic `C1 G1 C2 G2…` for Up).
* Manual BPM (no DAW needed): `Shift+Maschine` +1, `Shift+Star` −1, `Shift+Swing` +10, `Shift+Tempo` −10 (`20-300`, `Bright`/`Dim`). Explicit user tempo — counts as locked so the arp runs standalone; DAW clock takes over the value when present. Screen shows big BPM for 2s on change (rate shrunk top-left), then returns to the rate display.
* `Swing` cycles `Straight 50 → Light 55 → Medium 60 → Triplet 66.7` — interval `long=rate*swing/50`, `short=rate*(100-swing)/50`, alternated via `arp_pos%2` preserving average.
* Held pads arpeggiated as single notes or triads (if `Chords` on), with octave expansion cyclic, `Random` via `DefaultHasher`. `Chords`/`Step` + gates also affect `held_arp_notes`.

### 4.9 Fixed Velocity

`FixedVol` toggle `Bright` = `127`, `Dim` = velocity `max(1,val>>5)`.

---

## 5. Customizing (`example_config.toml`)

Copy and edit:

```bash
cp example_config.toml my.toml
$EDITOR my.toml
cargo run --release -- -c my.toml
```

### 5.1 Global

```toml
client_name = "My MK3"
port_name   = "My MK3 Out"
midi_channel = 2   # 2 → Ch 3
pad_channel  = 9
```

### 5.2 Pads

```toml
# 80 pages max (48 scales +16 drums +16 keyboard) — example has 80
pad_pages = [
  [36,38,42,46,...], # 0: Chromatic
  ... # 1-47 scales
  [49,57,51,53,...], # 64: Micro Drum Sampler (65)
  ... # 64-79: Keyboard scales
]
pad_page_button = "Group"
auto_page_button = "Auto"
lock_page_button = "Lock"  # PadMode is hard-coded for 49-64
pad_page_hold_select = true
pad_aftertouch = "poly"     # or "channel" / "cc" / "off"

scale_names = ["Chromatic","Major",...,"Micro Drum Sampler","MT Power Kit","General MIDI Kit","Salamander Kit","Drums 5",...,"Drums 16"] # 80
pad_page_colors = ["Red","Orange",...,"White"] # 64

[chord_types]
Chromatic = "power"
Major = "triad" # or "tetrad"
```

Colors: `Off, Red, Orange, LightOrange, WarmYellow, Yellow, Lime, Green, Mint, Cyan, Turquoise, Blue, Plum, Violet, Purple, Magenta, Fuchsia, White` (`crates/maschine_library/src/lights.rs:14`).

Validation: `pad_pages` 1-80 entries, each 16 notes `0-127`; `pad_page_colors` 80 if present.

### 5.3 Buttons

```toml
[buttons]
Play  = { type = "cc", cc = 94, channel = 0 }
Stop  = { type = "note", note = 60, channel = 1 }
Scene = { type = "pc", cc = 10 }
Mute  = { type = "off" }
```

Keys = exact names in `crates/maschine_library/src/controls.rs:4` (`Maschine, Star, Browse, ... , EncoderPress, EncoderTouch`). Case-insensitive. Unlisted keep defaults; `type="off"` to mute. Reserved buttons (paging, strip, transpose, chords, arp, gates) won’t send MIDI even if configured.

### 5.4 Encoder / Strip / Transpose

```toml
[encoder]
cc = 7
channel = 0
mode = "relative"  # or "absolute"

[slider]
cc = 1
mode = "absolute" # or "pitchbend" / "relative" (overridden by Pitch/Mod/Perform)

[transpose]
semitone_up = "Right"
semitone_down = "Left"
octave_up = ""
octave_down = ""
reset = ""
```

---

## 6. LEDs & Screen

* **Pad press** → `Normal` on hit, `Dim` on release. Override via `pad_colors` / `pad_page_colors`.
* **Button press** → Toggles `Bright`/`Dim` (3-way strip `Dim` inactive), gates `Bright` held/`Dim` released.
* **Strip** → 25-LED bar: `Pitch` spring dim on release, `Mod`/`Free` hold last position.
* **Page change** → all pads selector `Bright` current vs `Dim` others for 700 ms, screen `n/80`.
* **Arp on** → screen shows `arp_rate` (`1/16` etc.) + `x octaves`, transpose hidden; arp off shows `n/64`.
* **Self-test** on launch — ignore; ready when screen shows `1/64`.

Brightness: `Off=0x00, Dim=0x7c, Normal=0x7e, Bright=0x7f` (`lights.rs:6`).

---

## 7. Troubleshooting

| Symptom | Fix |
|---------|-----|
| `Config validation failed` | Follow error (e.g., `pad_pages should be 64`, `cc 0–127`). Prints parsed settings before failure. |
| `No such file or directory /dev/hidraw` or `open VI 0x17cc PID 0x1700 failed` | Check USB, `lsusb \| grep 17cc:1700`; re-apply `98-maschine.rules` + `udevadm trigger`; unplug/re-plug. |
| No MIDI port in DAW | Check backend — ALSA vs JACK is compile-time (`--features jack`). For JACK, start JACK before driver. Check `aconnect -l` (ALSA) or `jack_lsp`. |
| Pads always same notes | Paging? Verify `pad_pages` len 80 and `pad_page_button` not empty. Screen should change. Check logs `Pad page selected → n/m`. |
| `Group` doesn’t send MIDI | It’s paging button — reserved when `pad_pages.len()>1`. Set `pad_page_button=""` or map different button. Same for `Auto`/`Lock`/`PadMode`/`Left`/`Right`/`Pitch`/`Mod`/`Perform`/`Chords`/`Step`/gates/arp. |
| Encoder feels inverted | `mode=relative` emits `1` CW / `127` CCW; some DAWs expect `65/63`. Swap in DAW mapping or log `Encoder: ...`. |
| Strip is jumpy / LEDs flicker | Raw `1–200` scaled to `0–127`; touching near edge is normal. `Pitch` spring vs `Mod`/`Free` hold is intentional. |
| PadMode doesn’t go to drums | Requires `total_pages>64`. Hold `PadMode` + tap pad `0` → `65`, tap `PadMode` alone cycles `65→66…`. Check logs `Pad page selected via PadMode`. |

Logs: driver prints `Button press/release`, `Pad idx: NoteOn @ vel (page n)`, `Encoder rel delta`, `Slider -> CC/PitchBend`, `Pad page selected`, `Strip mode ->`, `Arp ->`, `Swing ->`, `Transpose ->` (`main.rs`).

---

## 8. Windows Parity Notes

* Windows Controller Editor → **Group + Pad** to change pad pages: replicated as `Group+Pad` (1-16), `Auto+Pad` (17-32), `Lock+Pad` (33-48), `PadMode+Pad` (49-64), `Keyboard+Pad` (65-80).
* NI’s “Pages” tab knob pages don’t exist on Mikro MK3 — only pad pages. This driver has no separate knob pages (change encoder CC via config).
* LED colors and brightness match firmware capabilities (4 levels) — color per page is now configurable where Windows only allowed fixed-blue in MIDI mode.
* Additional performance features (scales, chords, arp, swing, gates, transpose, strip modes) are Linux-only extensions.

---

## 9. File Map

* `example_config.toml` — annotated template (80 pages, copy me).
* `crates/driver/src/settings.rs` — schema + validation (max 80 pages).
* `crates/driver/src/main.rs` — `main_loop` (HID `0x01` buttons, `0x02` pads, `buf[7]` encoder, `buf[10]` strip, gates/arp).
* `crates/maschine_library/src/controls.rs` — button enum + `PadEventType`.
* `crates/maschine_library/src/lights.rs` — `PadColors/Brightness`.
* `98-maschine.rules` — udev.

---

## 10. Examples

**Use pads as octaves for a melodic VST** (like in Windows videos):

```toml
pad_pages = [
  [48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63], # C2
  [60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75], # C3
]
pad_page_button = "Keyboard"
```

**Transport as Notes for Ableton:**

```toml
[buttons]
Play = { type = "note", note = 60, channel = 0 }
Stop = { type = "note", note = 61 }
Rec  = { type = "note", note = 62 }
```

**Disable page switch, make Group a regular CC:**

```toml
pad_page_button = ""
[buttons]
Group = { type = "cc", cc = 34 }
```

**Free strip assignable via Perform:**

Press `Perform` `Bright` → strip sends `CC16` hold (map in DAW), `Pitch`/`Mod` `Dim`. Press `Pitch`/`Mod` to return.

---

*Contributions welcome — open an issue/PR with your `my.toml` and a short `hidraw` log if something misbehaves.*
