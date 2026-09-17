# Maschine Mikro MK3 — User Guide (Linux Driver)

This guide covers how to install, run, configure and customize the userspace MIDI driver for the **Native Instruments Maschine Mikro MK3** on Linux. The driver exposes all controls via a virtual MIDI port — pads, buttons, encoder and touch strip — with the same “pages” workflow you have on Windows in Controller Editor.

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
git clone https://github.com/r00tman/maschine-mikro-mk3-driver.git
cd maschine-mikro-mk3-driver
sudo cp 98-maschine.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger

cargo run --release              # ALSA backend (default)
cargo run --release --features jack  # JACK backend (compile-time choice)
```

On start the controller:

* runs a self-test (`00 → 03` on screen, all LEDs cycle),
* shows `1/1` (or `1/8` etc.) on the screen,
* creates a virtual port `Maschine Mikro MK3 MIDI Out` (`client_name` / `port_name` in config).

Point your DAW / Hydrogen / REAPER / Ardour / etc. at that port.

> **No hardware?** The driver still validates the config and prints `Running with settings:` / `Effective pad_pages` before trying to open USB. Useful for dry-run checks.

### With a Custom Config

```bash
cargo run --release -- -c example_config.toml
cargo run --release -- -c /path/to/my.toml
```

All keys are optional — missing values inherit built-in defaults (`crates/driver/src/settings.rs:172`). See `example_config.toml` (192 lines, fully commented).

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

* **16 pads** — velocity + aftertouch, RGB (`Off, Red, Orange, LightOrange, WarmYellow, Yellow, Lime, Green, Mint, Cyan, Turquoise, Blue, Plum, Violet, Purple, Magenta, Fuchsia, White`) + 4 brightness levels (`Off, Dim, Normal, Bright` — more than official “on/off”).
* **39 button LEDs + 25 strip LEDs** (`crates/maschine_library/src/lights.rs:53` — every button except `EncoderPress/Touch` has LED).
* Screen is a 128×32 buffered mono display (`crates/maschine_library/src/screen.rs:3`).

---

## 3. MIDI Concepts & Defaults

### 3.1 Channels

* `midi_channel` — default for **buttons / encoder / strip** (`0` → `Ch 1` in DAWs). Range `0–15`.
* `pad_channel` — dedicated pad channel, default `9` → `Ch 10` (GM drums). Keeps drums separate from CCs.

Override per-control with `channel = N`.

### 3.2 Message Types

| Control | Types (`type = ...`) | MIDI sent |
|---------|----------------------|-----------|
| Button | `cc` (default), `note`, `pc`, `off` | `CC value 127` on press / `0` on release; `NoteOn vel 127` / `NoteOff`; `ProgramChange` on press |
| Pad | fixed `NoteOn/NoteOff` per page + optional aftertouch | `NoteOn/NoteOff` on `pad_channel`; `poly` → `Poly Aftertouch`, `channel` → `Channel Aftertouch`, `cc` → `CC 74`, `off` → nothing |
| Encoder | `cc` + `mode` | `relative` → `CC 1` (cw) / `127` (ccw) per tick; `absolute` → `0–127` |
| Strip | `cc` / `pitchbend` + `mode` | `absolute` → `0–127` scaled from raw `1–200`; `pitchbend` → 14-bit; `relative` |

Per-button `value_press` / `value_release` customizes CC velocities (defaults `127/0`).

---

## 4. Current Default Mapping

This is what you get with `cargo run` **without** a config, or with `example_config.toml` unedited. All button CCs are editable — defaults are also defined in `crates/driver/src/settings.rs:195`.

### 4.1 Pads — Pages

`pad_pages` is 8 pages by default (max 16). Page `0` is startup.

| Page | Purpose | 16 Notes (MIDI # — name) |
|------|---------|---------------------------|
| **1** | General drums (GM) | `36 C2, 38 D2, 42 F#2, 46 A#2, 43 G2, 47 B2, 49 C#3, 51 D#3, 37 C#2, 39 D#2, 44 G#2, 45 A2, 48 C3, 50 D3, 52 E3, 53 F3` |
| **2** | Alt kit | `36,38,42,46,41,43,45,49,35,37,39,40,44,48,52,55` |
| **3** | Chromatic C2–C3 | `48–63` (`C2`→`D#3` linear) |
| **4** | Chromatic C3–C4 | `60–75` (`C3`→`D#4`) |
| **5** | Bass / 808 | `36–51` linear |
| **6** | Legacy example (old `notemaps`) | `49,27,31,57,48,47,43,59,36,38,46,51,36,38,42,44` |
| **7** | Chromatic C4–C5 | `72–87` |
| **8** | Sub C1–C2 | `36–51` |

Legacy single-page `notemaps = [16 notes]` is still supported — if `pad_pages` is absent the driver wraps it as one page.

**Velocity:** `vel = max(1, val>>5)` from raw `0–2047` (`crates/driver/src/main.rs:539`).

### 4.2 Page Switching (Windows parity)

`pad_page_button = "Group"` (default) + `pad_page_hold_select = true`:

* **Hold `Group` + tap Pad `0–7`** → jump directly to page `1–8` (like `Group + Pad A–H` in Controller Editor). LED selector shows current page (Bright) vs others (Dim); screen shows `n / total` (`crates/driver/src/main.rs:132`).
* **Tap `Group` alone** → cycle `1→2→…→8→1`.
* Set `pad_page_button = ""` to disable paging (static drums).
* Or `pad_page_hold_select = false` to cycle only.

> The paging button is reserved — it won’t send its configured MIDI when paging is active and multiple pages exist. Disable paging or pick another button (`PadMode`, `Keyboard`, …) if you need `Group` as a CC.

### 4.3 Buttons

Defaults (`crates/driver/src/settings.rs:195`, mirrored in `example_config.toml:130`):

| Button | CC |
|--------|----|
| `Maschine` | 20 |
| `Star` | 21 |
| `Browse` | 22 |
| `Volume` | 23 |
| `Swing` | 24 |
| `Tempo` | 25 |
| `Plugin` | 26 |
| `Sampling` | 27 |
| `Left` | 28 |
| `Right` | 29 |
| `Pitch` | 30 |
| `Mod` | 31 |
| `Perform` | 32 |
| `Notes` | 33 |
| `Auto` | 35 |
| `Lock` | 36 |
| `NoteRepeat` | 36→ actually 37 (see code) |
| `FixedVol` | 37 |
| `PadMode` | 38 |
| `Keyboard` | 39 |
| `Chords` | 40 |
| `Step` | 41 |
| `Scene` | 42 |
| `Pattern` | 43 |
| `Events` | 44 |
| `Variation` | 45 |
| `Duplicate` | 46 |
| `Select` | 47 |
| `Solo` | 48 |
| `Mute` | 49 |
| `EncoderPress` | 50 |
| `Tap` | 40 (transport) — see note: in example `Tap=40, Follow=41` |
| `Follow` | 41 |
| `Play` | 85 |
| `Rec` | 86 |
| `Stop` | 87 |
| `Restart` | 89 |
| `Erase` | 90 etc. |
| `Shift` | 88 |

*Exact table in `example_config.toml:130-182`. `Group` is commented out (reserved). Keys are case-insensitive. `EncoderTouch` exists but unmapped by default.*

All on `midi_channel` (0) unless `channel` overridden. Press = `127`, Release = `0` (configurable).

### 4.4 Encoder

`[encoder] cc=14 mode=relative` on `midi_channel`. HID delta `1`→CW, `0xFF`(-1)→CCW (`crates/driver/src/main.rs:247`) → emits `CC14=1` or `127` per tick (steps repeated for larger deltas). Set `mode=absolute` for absolute `0–127`.

Pushing the encoder also emits `EncoderPress` button CC `102`.

### 4.5 Touch Strip (Slider)

`[slider] cc=1 mode=absolute` on `midi_channel`. Raw `1–200` → `0–127` (`crates/driver/src/main.rs:294`). 25 LEDs follow position (`crates/driver/src/main.rs:446`). Alternative: `mode=pitchbend` → 14-bit bend centered at `8192`.

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
# single kit (legacy):
notemaps = [36,38,42,46,49,51,44,45,48,50,52,53,37,39,41,43]

# or multi-page (preferred):
pad_pages = [
  [36,38,42,46,43,47,49,51,37,39,44,45,48,50,52,53],
  [60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75],
]
pad_page_button = "PadMode"   # use PadMode to switch
pad_page_hold_select = false   # cycle only
pad_aftertouch = "channel"     # or "poly" / "cc" / "off"

# colors
pad_colors = ["Red","Red","Yellow","Yellow", ...] # 16
pad_page_colors = ["Blue","Green","Yellow","Orange"] # per-page
```

Colors: `Off, Red, Orange, LightOrange, WarmYellow, Yellow, Lime, Green, Mint, Cyan, Turquoise, Blue, Plum, Violet, Purple, Magenta, Fuchsia, White` (`crates/maschine_library/src/lights.rs:14`).

### 5.3 Buttons

```toml
[buttons]
Play  = { type = "cc", cc = 94, channel = 0 }          # CC
Stop  = { type = "note", note = 60, channel = 1 }      # Note C4
Scene = { type = "pc", cc = 10 }                       # Program change
Mute  = { type = "off" }                               # disable MIDI, LED only
FixedVol = { type = "cc", cc = 7, value_press = 100, value_release = 0 }
```

Keys = exact names in `crates/maschine_library/src/controls.rs:4` (`Maschine, Star, Browse, ... , EncoderPress, EncoderTouch`). Case-insensitive. Unlisted buttons keep defaults; set `type="off"` to mute.

### 5.4 Encoder / Strip

```toml
[encoder]
cc = 16
channel = 0
mode = "relative"  # or "absolute"

[slider]
cc = 11
mode = "pitchbend" # or "absolute" / "relative"
```

---

## 6. LEDs & Screen

* **Pad press** → `Blue` `Normal` on hit, `Off` on release (`crates/driver/src/main.rs:497`). Override via `pad_colors` / `pad_page_colors`.
* **Button press** → `Normal` / `Off`.
* **Strip** → 25-LED bar: head=`Normal`, trail=`Dim`.
* **Page change** → all pads show selector (`Bright` = current, `Dim` = other pages, `Off` beyond count) + screen `n/total` (`crates/driver/src/main.rs:583`).
* **Self-test** on launch — ignore; driver is ready when screen shows `1/8`.

Brightness levels: `Off=0x00, Dim=0x7c, Normal=0x7e, Bright=0x7f` (`crates/maschine_library/src/lights.rs:6`).

---

## 7. Troubleshooting

| Symptom | Fix |
|---------|-----|
| `Config validation failed` | Follow error (e.g., `notemaps should be 16 pads`, `cc should be 0–127`). `cargo run -- -c my.toml` prints parsed settings before failure. |
| `No such file or directory /dev/hidraw` or `open VI 0x17cc PID 0x1700 failed` | Check USB, `lsusb \| grep 17cc:1700`; re-apply `98-maschine.rules` + `udevadm trigger`; unplug/re-plug; check you’re not in Maschine software that grabs HID. |
| No MIDI port in DAW | Check backend — ALSA vs JACK is compile-time (`--features jack`). For JACK, start JACK before driver. Check `aconnect -l` (ALSA) or `jack_lsp`. |
| Pads always same notes | You’re paging? See §4.2. Verify `pad_pages` length >1 and `pad_page_button` not empty. Screen should change. Check logs `Pad page selected → n/m`. |
| `Group` doesn’t send MIDI | It’s the paging button — reserved when `pad_pages.len()>1`. Set `pad_page_button=""` or map a different button. |
| Encoder feels inverted | `mode=relative` emits `1` CW / `127` CCW; some DAWs expect `65/63`. Swap in DAW mapping or file an issue with a `raw` log (`Encoder: ...` line). |
| Strip is jumpy / LEDs flicker | Raw range `1–200` scaled to `0–127`; touching near edge is normal. Use `mode=pitchbend` for smoother DAW mapping. |

Logs: driver prints every `Button press/release`, `Pad idx: NoteOn @ vel (page n)`, `Encoder rel delta`, `Slider -> CC` (`crates/driver/src/main.rs:367/439`). Pipe to `& tee` for debugging.

---

## 8. Windows Parity Notes

* Windows Controller Editor → **Group + Pad** to change pad pages: replicated as `pad_pages` + `Group + Pad`.
* NI’s “Pages” tab knob pages don’t exist on Mikro MK3 — only pad pages. This driver therefore has no separate knob pages (change encoder CC via config instead).
* LED colors and brightness match firmware capabilities (4 levels) — color per pad/page is now configurable where Windows only allowed fixed-blue in MIDI mode.

---

## 9. File Map

* `example_config.toml` — annotated template (copy me).
* `crates/driver/src/settings.rs` — schema + validation (sources of truth for ranges).
* `crates/driver/src/main.rs:323` — `main_loop` (HID `0x01` buttons, `0x02` pads, `buf[7]` encoder, `buf[10]` strip).
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
  [72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87], # C4
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

---

*Contributions welcome — open an issue/PR with your `my.toml` and a short `hidraw` log if something misbehaves.*
