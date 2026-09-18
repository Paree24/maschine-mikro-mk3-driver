mod self_test;
mod settings;

use crate::self_test::self_test;
use crate::settings::Settings;
use clap::Parser;
use config::Config;
use hidapi::{HidDevice, HidResult};
use maschine_library::controls::{Buttons, PadEventType};
use maschine_library::font::Font;
use maschine_library::lights::{Brightness, Lights, PadColors};
use maschine_library::screen::Screen;
use midir::os::unix::VirtualOutput;
use midir::{MidiOutput, MidiOutputConnection};
use midly::{MidiMessage, live::LiveEvent};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[clap(
    name = "Maschine Mikro MK3 Userspace MIDI driver",
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
)]
struct Args {
    #[clap(short, long, help = "Config file (see example_config.toml)")]
    config: Option<String>,
}

fn main() -> HidResult<()> {
    let args = Args::parse();

    let mut cfg = Config::builder();

    if let Some(config_fn) = args.config {
        cfg = cfg.add_source(config::File::with_name(config_fn.as_str()));
    }

    let cfg = cfg.build().expect("Can't create settings");
    let settings: Settings = cfg.try_deserialize().expect("Can't parse settings");

    if let Err(e) = settings.validate() {
        eprintln!("Config validation failed: {e}");
        std::process::exit(1);
    }

    println!("Running with settings:");
    println!("{settings:?}");
    let pages = settings.effective_pad_pages();
    println!("Effective pad_pages ({} pages):", pages.len());
    for (i, p) in pages.iter().enumerate() {
        println!("  Page {i}: {:?}", p);
    }

    let output = MidiOutput::new(&settings.client_name).expect("Couldn't open MIDI output");
    let mut port = output
        .create_virtual(&settings.port_name)
        .expect("Couldn't create virtual port");

    let api = hidapi::HidApi::new()?;
    #[allow(non_snake_case)]
    let (VID, PID) = (0x17cc, 0x1700);
    let device = api.open(VID, PID)?;

    device.set_blocking_mode(false)?;

    let mut screen = Screen::new();
    let mut lights = Lights::new();

    self_test(&device, &mut screen, &mut lights)?;

    // Show initial page on screen
    update_screen(&mut screen, &device, 0, pages.len())?;

    main_loop(&device, &mut screen, &mut lights, &mut port, &settings)?;

    Ok(())
}

fn parse_pad_color(s: &str) -> Option<PadColors> {
    match s.to_lowercase().as_str() {
        "off" => Some(PadColors::Off),
        "red" => Some(PadColors::Red),
        "orange" => Some(PadColors::Orange),
        "lightorange" | "light_orange" => Some(PadColors::LightOrange),
        "warmyellow" | "warm_yellow" => Some(PadColors::WarmYellow),
        "yellow" => Some(PadColors::Yellow),
        "lime" => Some(PadColors::Lime),
        "green" => Some(PadColors::Green),
        "mint" => Some(PadColors::Mint),
        "cyan" => Some(PadColors::Cyan),
        "turquoise" => Some(PadColors::Turquoise),
        "blue" => Some(PadColors::Blue),
        "plum" => Some(PadColors::Plum),
        "violet" => Some(PadColors::Violet),
        "purple" => Some(PadColors::Purple),
        "magenta" => Some(PadColors::Magenta),
        "fuchsia" => Some(PadColors::Fuchsia),
        "white" => Some(PadColors::White),
        _ => None,
    }
}

fn button_debug_name(b: Buttons) -> String {
    format!("{b:?}")
}

fn is_pad_page_button(settings: &Settings, button: Buttons) -> bool {
    if let Some(name) = settings.pad_page_button_parsed() {
        let btn_name = button_debug_name(button);
        btn_name.eq_ignore_ascii_case(&name)
    } else {
        false
    }
}

fn is_auto_page_button(settings: &Settings, button: Buttons) -> bool {
    if let Some(name) = settings.auto_page_button_parsed() {
        let btn_name = button_debug_name(button);
        btn_name.eq_ignore_ascii_case(&name)
    } else {
        false
    }
}

fn is_lock_page_button(settings: &Settings, button: Buttons) -> bool {
    if let Some(name) = settings.lock_page_button_parsed() {
        let btn_name = button_debug_name(button);
        btn_name.eq_ignore_ascii_case(&name)
    } else {
        false
    }
}

// Build chord for pad: configurable via [chord_types] in config (power/triad/tetrad)
// pad_notes is driver idx order permuted; convert to phys sequential first
fn triad_for_pad(pad_notes: &[u8], idx: usize, transpose: i32, scale_name: Option<&str>, chord_types: &std::collections::HashMap<String, String>) -> Vec<u8> {
    if pad_notes.is_empty() || idx >= pad_notes.len() {
        return vec![];
    }
    // Convert driver idx order to phys sequential order (bottom->top left->right)
    // driver idx -> Pad: [13,14,15,16,12,11,10,9,8,7,6,5,1,2,3,4]
    // phys[0]=Pad1 bottom-left = driver[12], etc.
    // So phys = [driver[12],driver[13],driver[14],driver[15],driver[11],driver[10],driver[9],driver[8],driver[7],driver[6],driver[5],driver[4],driver[0],driver[1],driver[2],driver[3]]
    let to_phys = |driver: &[u8]| -> Vec<u8> {
        if driver.len() < 16 { return driver.to_vec(); }
        vec![
            driver[12], driver[13], driver[14], driver[15],
            driver[8], driver[9], driver[10], driver[11],
            driver[4], driver[5], driver[6], driver[7],
            driver[0], driver[1], driver[2], driver[3],
        ]
    };
    let phys = to_phys(pad_notes);
    let base = phys[0] as i32;
    // Derive intervals from all 16 notes (mod 12) to correctly handle chromatic (12) vs pentatonic (5)
    let mut set = std::collections::HashSet::new();
    for &note in &phys {
        let off = (note as i32 - base).rem_euclid(12);
        set.insert(off);
    }
    let mut intervals: Vec<i32> = set.into_iter().collect();
    intervals.sort_unstable();
    if intervals.is_empty() {
        intervals = vec![0,2,4,5,7,9,11];
    }
    let n = intervals.len() as i32;
    // Determine chord type: config override else auto (triad for 7, power for others)
    // Config example: [chord_types] "Chromatic" = "power", "Minor Pentatonic" = "power", "Major" = "triad", "Dorian" = "tetrad"
    let chord_type = scale_name
        .and_then(|name| chord_types.get(name).map(|s| s.to_lowercase()))
        .or_else(|| scale_name.and_then(|name| {
            let base_name = name.trim_end_matches(" Up");
            chord_types.get(base_name).map(|s| s.to_lowercase())
        }))
        .unwrap_or_else(|| if n == 7 { "triad".to_string() } else { "power".to_string() });
    if chord_type == "power" || chord_type == "fifth" || chord_type == "5" {
        let phys_idx = [12,13,14,15,8,9,10,11,4,5,6,7,0,1,2,3][idx.min(15)] as i32;
        let root = base + (phys_idx / n) * 12 + intervals[(phys_idx % n) as usize];
        let fifth = root + 7;
        return vec![((root+transpose).clamp(0,127)) as u8, ((fifth+transpose).clamp(0,127)) as u8];
    }
    if chord_type == "tetrad" || chord_type == "seventh" || chord_type == "7" || chord_type == "4" {
        let driver_to_phys_idx = [12,13,14,15,8,9,10,11,4,5,6,7,0,1,2,3];
        let phys_idx = driver_to_phys_idx[idx.min(15)] as i32;
        let root = base + (phys_idx / n) * 12 + intervals[(phys_idx % n) as usize];
        let third = base + ((phys_idx+2)/n)*12 + intervals[((phys_idx+2)%n) as usize];
        let fifth = base + ((phys_idx+4)/n)*12 + intervals[((phys_idx+4)%n) as usize];
        let seventh = base + ((phys_idx+6)/n)*12 + intervals[((phys_idx+6)%n) as usize];
        return vec![((root+transpose).clamp(0,127)) as u8, ((third+transpose).clamp(0,127)) as u8, ((fifth+transpose).clamp(0,127)) as u8, ((seventh+transpose).clamp(0,127)) as u8];
    }
    let driver_to_phys_idx = [12,13,14,15,8,9,10,11,4,5,6,7,0,1,2,3];
    let phys_idx = driver_to_phys_idx[idx.min(15)] as i32;
    let root_deg = phys_idx;
    let third_deg = root_deg + 2;
    let fifth_deg = root_deg + 4;
    let root = base + (root_deg / n) * 12 + intervals[(root_deg % n) as usize];
    let third = base + (third_deg / n) * 12 + intervals[(third_deg % n) as usize];
    let fifth = base + (fifth_deg / n) * 12 + intervals[(fifth_deg % n) as usize];
    vec![
        ((root + transpose).clamp(0,127)) as u8,
        ((third + transpose).clamp(0,127)) as u8,
        ((fifth + transpose).clamp(0,127)) as u8,
    ]
}

#[derive(Debug, Clone, Copy)]
enum TransposeAction {
    SemitoneUp,
    SemitoneDown,
    OctaveUp,
    OctaveDown,
    Reset,
}

fn transpose_action(settings: &Settings, button: Buttons) -> Option<TransposeAction> {
    let name = button_debug_name(button);
    if !settings.transpose.semitone_up.trim().is_empty() && name.eq_ignore_ascii_case(settings.transpose.semitone_up.trim()) {
        return Some(TransposeAction::SemitoneUp);
    }
    if !settings.transpose.semitone_down.trim().is_empty() && name.eq_ignore_ascii_case(settings.transpose.semitone_down.trim()) {
        return Some(TransposeAction::SemitoneDown);
    }
    if !settings.transpose.octave_up.trim().is_empty() && name.eq_ignore_ascii_case(settings.transpose.octave_up.trim()) {
        return Some(TransposeAction::OctaveUp);
    }
    if !settings.transpose.octave_down.trim().is_empty() && name.eq_ignore_ascii_case(settings.transpose.octave_down.trim()) {
        return Some(TransposeAction::OctaveDown);
    }
    if !settings.transpose.reset.trim().is_empty() && name.eq_ignore_ascii_case(settings.transpose.reset.trim()) {
        return Some(TransposeAction::Reset);
    }
    None
}

fn lookup_button_config<'a>(
    norm_map: &'a HashMap<String, crate::settings::ButtonConfig>,
    button: Buttons,
) -> Option<&'a crate::settings::ButtonConfig> {
    let key = button_debug_name(button).to_lowercase();
    norm_map.get(&key)
}

fn normalized_button_map(settings: &Settings) -> HashMap<String, crate::settings::ButtonConfig> {
    let mut m = HashMap::new();
    for (k, v) in &settings.buttons {
        m.insert(k.to_lowercase(), v.clone());
    }
    m
}

fn update_screen(screen: &mut Screen, device: &HidDevice, page: usize, total: usize) -> HidResult<()> {
    update_screen_with_transpose(screen, device, page, total, 0)
}

fn update_screen_with_transpose(screen: &mut Screen, device: &HidDevice, page: usize, total: usize, _transpose: i32) -> HidResult<()> {
    update_screen_arp(screen, device, page, total, 0, false, "", 1)
}

fn should_handle_encoder_rotation(raw: u8) -> bool {
    // Only handle actual rotation deltas, not touch noise
    // Raw 1 = CW tick, 0xFF (-1) = CCW, 0 = idle. Ignore other values (e.g. 0x7F touch noise)
    let delta = raw as i8;
    delta == 1 || delta == -1 || delta == 2 || delta == -2
}

fn update_screen_arp(screen: &mut Screen, device: &HidDevice, page: usize, total: usize, transpose: i32, arp_on: bool, arp_rate: &str, arp_oct: usize) -> HidResult<()> {
    screen.reset();
    if arp_on {
        let mut x = 30;
        for ch in arp_rate.chars() {
            if ch == '/' {
                for i in 0..12 { screen.set(12 + i, x, true); }
                x += 10;
            } else if ch == '.' {
                screen.set(18, x, true);
                screen.set(19, x, true);
                x += 6;
            } else if ch == 'T' {
                for i in 0..8 { screen.set(4 + i, x, true); screen.set(4 + i, x+4, true); }
                for i in 0..4 { screen.set(4, x+i, true); screen.set(8, x+i, true); }
                x += 10;
            } else if let Some(d) = ch.to_digit(10) {
                Font::write_digit(screen, 8, x, d as usize, 2);
                x += 18;
            }
        }
        Font::write_digit(screen, 0, 110, arp_oct, 1);
        let _ = transpose;
        screen.write(device)
    } else {
        // Show "P:x/y" with large digits
        let display_page = page + 1;
        let display_total = total;
        if display_page < 10 {
            Font::write_digit(screen, 8, 20, display_page % 10, 3);
        } else {
            Font::write_digit(screen, 8, 8, display_page / 10, 2);
            Font::write_digit(screen, 8, 32, display_page % 10, 2);
        }
        for i in 0..16 {
            screen.set(10 + i, 58 + i / 2, true);
            screen.set(11 + i, 58 + i / 2, true);
        }
        if display_total < 10 {
            Font::write_digit(screen, 8, 72, display_total % 10, 3);
        } else {
            Font::write_digit(screen, 8, 72, display_total / 10, 2);
            Font::write_digit(screen, 8, 92, display_total % 10, 2);
        }
        if transpose != 0 {
            let sign = if transpose > 0 { 1 } else { 0 };
            for x in 100..126 {
                screen.set(2, x, true);
                screen.set(3, x, true);
            }
            let abs_t = transpose.abs() as usize;
            if abs_t < 10 {
                Font::write_digit(screen, 0, 110, abs_t % 10, 1);
            } else {
                Font::write_digit(screen, 0, 100, (abs_t / 10) % 10, 1);
                Font::write_digit(screen, 0, 110, abs_t % 10, 1);
            }
            if sign == 1 {
                for i in 0..6 {
                    screen.set(2 + i, 100, true);
                }
                for i in 0..6 {
                    screen.set(4, 97 + i, true);
                }
            } else {
                for i in 0..6 {
                    screen.set(4, 97 + i, true);
                }
            }
        }
        screen.write(device)
    }
}

fn send_midi(port: &mut MidiOutputConnection, channel: u8, msg: MidiMessage) {
    let ev = LiveEvent::Midi {
        channel: channel.into(),
        message: msg,
    };
    let mut buf = Vec::new();
    if ev.write(&mut buf).is_ok() {
        let _ = port.send(&buf);
    }
}

fn handle_button_midi(
    port: &mut MidiOutputConnection,
    settings: &Settings,
    norm_map: &HashMap<String, crate::settings::ButtonConfig>,
    button: Buttons,
    pressed: bool,
) {
    let Some(cfg) = lookup_button_config(norm_map, button) else {
        return;
    };
    if cfg.type_ == "off" {
        return;
    }
    let ch = settings.effective_channel(cfg.channel);
    let vel_press = cfg.value_press.unwrap_or(127).min(127);
    let vel_release = cfg.value_release.unwrap_or(0).min(127);

    match cfg.type_.as_str() {
        "cc" => {
            let cc = cfg.cc.unwrap_or(0);
            let val = if pressed { vel_press } else { vel_release };
            println!("Button {:?} -> CC {} ch {} val {}", button, cc, ch, val);
            send_midi(
                port,
                ch,
                MidiMessage::Controller {
                    controller: cc.into(),
                    value: val.into(),
                },
            );
        }
        "note" => {
            let note = cfg.note.unwrap_or(60);
            let vel = if pressed { vel_press } else { 0 };
            let msg = if pressed {
                MidiMessage::NoteOn {
                    key: note.into(),
                    vel: vel.into(),
                }
            } else {
                MidiMessage::NoteOff {
                    key: note.into(),
                    vel: vel.into(),
                }
            };
            println!("Button {:?} -> Note {} ch {} vel {} {}", button, note, ch, vel, if pressed { "ON" } else { "OFF" });
            send_midi(port, ch, msg);
        }
        "pc" => {
            if pressed {
                // ProgramChange only on press
                let prog = cfg.cc.unwrap_or(cfg.note.unwrap_or(0));
                println!("Button {:?} -> ProgramChange {} ch {}", button, prog, ch);
                send_midi(
                    port,
                    ch,
                    MidiMessage::ProgramChange {
                        program: prog.into(),
                    },
                );
            }
        }
        _ => {}
    }
}

fn handle_encoder(port: &mut MidiOutputConnection, settings: &Settings, raw: u8) {
    if raw == 0 {
        return;
    }
    // Interpret raw as signed delta (two's complement)
    let delta = raw as i8;
    if delta == 0 {
        return;
    }
    let ch = settings.effective_channel(settings.encoder.channel);
    let cc = settings.encoder.cc;
    match settings.encoder.mode.as_str() {
        "absolute" => {
            // Map raw 0..255 to 0..127 absolute (rare). Just clamp.
            let val = (raw & 0x7F).min(127);
            println!("Encoder abs -> CC {} ch {} val {}", cc, ch, val);
            send_midi(
                port,
                ch,
                MidiMessage::Controller {
                    controller: cc.into(),
                    value: val.into(),
                },
            );
        }
        _ => {
            // relative: send 1 for + and 127 for - per tick, repeating for magnitude
            // Classic: 1 = +1, 127 = -1 (or 65/63). Use 1/127.
            let steps = delta.abs() as usize;
            let val = if delta > 0 { 1u8 } else { 127u8 };
            for _ in 0..steps.max(1) {
                println!("Encoder rel delta {} -> CC {} ch {} val {}", delta, cc, ch, val);
                send_midi(
                    port,
                    ch,
                    MidiMessage::Controller {
                        controller: cc.into(),
                        value: val.into(),
                    },
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ArpMode { Up, Down, UpDown, DownUp, Random }

impl ArpMode {
    fn next(self) -> Self {
        match self {
            ArpMode::Up => ArpMode::Down,
            ArpMode::Down => ArpMode::UpDown,
            ArpMode::UpDown => ArpMode::DownUp,
            ArpMode::DownUp => ArpMode::Random,
            ArpMode::Random => ArpMode::Up,
        }
    }
    fn name(self) -> &'static str {
        match self {
            ArpMode::Up => "Up",
            ArpMode::Down => "Down",
            ArpMode::UpDown => "UpDown",
            ArpMode::DownUp => "DownUp",
            ArpMode::Random => "Random",
        }
    }
}

// Strip -> arp rate: 1/1 .. 1/64 with dotted/triplet at appropriate positions
// 25 LED positions -> map to 19 rates (slow->fast) plus repeats at ends
fn arp_rate_from_strip(raw: u8) -> (String, Duration) {
    let idx = ((raw as usize * 18) / 201).min(17);
    const RATES: &[(&str, f32)] = &[
        ("3/4", 3.0), ("1/2", 2.0), ("3/8", 1.5), ("1/3", 1.333), ("1/4", 1.0), ("3/16", 0.75), ("1/6", 0.666), ("1/8", 0.5), ("3/32", 0.375), ("1/12", 0.333), ("1/16", 0.25), ("3/64", 0.1875), ("1/24", 0.166), ("1/32", 0.125), ("3/128", 0.09375), ("1/48", 0.0833), ("1/64", 0.0625), ("1/96", 0.0417),
    ];
    let (name, beats) = RATES[idx];
    let bpm = 120.0;
    let secs = 60.0 / bpm * beats;
    (name.to_string(), Duration::from_secs_f32(secs))
}

fn handle_slider(port: &mut MidiOutputConnection, settings: &Settings, raw: u8, is_pitchbend: bool) {
    let ch = if is_pitchbend {
        settings.pad_midi_channel()
    } else {
        settings.effective_channel(settings.slider.channel)
    };
    let cc = settings.slider.cc;
    if raw == 0 {
        if is_pitchbend {
            let bend = midly::PitchBend(midly::num::u14::from(8192));
            println!("Slider PitchBend center ch {}", ch);
            send_midi(port, ch, MidiMessage::PitchBend { bend });
            let ch2 = settings.effective_channel(settings.slider.channel);
            if ch2 != ch {
                send_midi(port, ch2, MidiMessage::PitchBend { bend });
            }
        }
        return;
    }
    let scaled = ((raw as u16 * 127) / 200).min(127) as u8;
    let val = scaled;
    if is_pitchbend {
            let bend_val = (val as u32 * 16383 / 127) as u16;
            let bend = midly::num::u14::from(bend_val);
            println!("Slider -> PitchBend ch {} val {} raw {} bend {}", ch, bend_val, raw, bend_val);
            send_midi(port, ch, MidiMessage::PitchBend { bend: midly::PitchBend(bend) });
            let ch2 = settings.effective_channel(settings.slider.channel);
            if ch2 != ch {
                send_midi(port, ch2, MidiMessage::PitchBend { bend: midly::PitchBend(bend) });
            }
        } else {
            println!("Slider -> CC {} ch {} val {} raw {}", cc, ch, val, raw);
            send_midi(
                port,
                ch,
                MidiMessage::Controller {
                    controller: cc.into(),
                    value: val.into(),
                },
            );
        }
}

// Arpeggiator: NoteRepeat toggles on/off, Notes cycles 5 modes, encoder controls rate when arp enabled
fn arp_rate_from_strip_raw(raw: u8) -> (String, Duration) {
    let idx = ((raw as usize * 18) / 201).min(17);
    const RATES: &[(&str, f32)] = &[
        ("3/4", 3.0), ("1/2", 2.0), ("3/8", 1.5), ("1/3", 1.333), ("1/4", 1.0), ("3/16", 0.75), ("1/6", 0.666), ("1/8", 0.5), ("3/32", 0.375), ("1/12", 0.333), ("1/16", 0.25), ("3/64", 0.1875), ("1/24", 0.166), ("1/32", 0.125), ("3/128", 0.09375), ("1/48", 0.0833), ("1/64", 0.0625), ("1/96", 0.0417),
    ];
    let (name, beats) = RATES[idx];
    let bpm = 120.0;
    let secs = 60.0 / bpm * beats;
    (name.to_string(), Duration::from_secs_f32(secs))
}

fn main_loop(
    device: &HidDevice,
    screen: &mut Screen,
    lights: &mut Lights,
    port: &mut MidiOutputConnection,
    settings: &Settings,
) -> HidResult<()> {
    let pad_pages = settings.effective_pad_pages();
    let total_pages = pad_pages.len();
    let mut current_page: usize = 0;
    let mut pad_page_holding = false;
    let mut page_selected_via_pad = false;
    let mut auto_page_holding = false;
    let mut auto_page_selected_via_pad = false;
    let mut lock_page_holding = false;
    let mut lock_page_selected_via_pad = false;
    let mut chords_active = false;
    let mut tetrad_active = false;
    let mut step_tetrad_active = false;
    let mut button_prev = [false; 64];
    let norm_map = normalized_button_map(settings);
    #[derive(Clone, Copy, PartialEq, Debug)] enum StripMode { PitchBend, ModWheel, Free }
    let mut transpose_offset: i32 = 0;
    let mut strip_mode = if settings.slider.mode == "pitchbend" { StripMode::PitchBend } else { StripMode::ModWheel };
    let mut strip_prev_nonfree = strip_mode;
    let mut prev_slider_touched = false;
    let mut last_slider_cnt: i32 = -1;
    let mut arp_enabled = false;
    let mut arp_mode = ArpMode::Up;
    let mut arp_rate: Duration = Duration::from_secs_f32(60.0/120.0 * 0.25); // 1/16 at 120bpm
    let mut arp_rate_name = "1/16".to_string();
    let mut arp_octaves: usize = 1;
    let mut held_arp_notes: Vec<Vec<u8>> = Vec::new();
    let mut arp_pos: usize = 0;
    let mut arp_dir: i32 = 1;
    let mut arp_last_tick = Instant::now();
    let mut arp_current_notes: Option<Vec<u8>> = None;
    let mut fixed_vel_active = false;

    // For auto-clearing page selector LEDs
    let mut selector_active = false;
    let mut selector_since: Option<Instant> = None;

    // Init Pitch/Mod/Perform LEDs to reflect strip mode - 3-way exclusive, exactly one Bright, others Dim
    {
        if lights.button_has_light(Buttons::Pitch) {
            lights.set_button(Buttons::Pitch, if strip_mode == StripMode::PitchBend { Brightness::Bright } else { Brightness::Dim });
        }
        if lights.button_has_light(Buttons::Mod) {
            lights.set_button(Buttons::Mod, if strip_mode == StripMode::ModWheel { Brightness::Bright } else { Brightness::Dim });
        }
        if lights.button_has_light(Buttons::Perform) {
            lights.set_button(Buttons::Perform, if strip_mode == StripMode::Free { Brightness::Bright } else { Brightness::Dim });
        }
        let _ = lights.write(device);
    }
    // Default: dim by default with per-page color (one color per page, configurable via pad_page_colors)
    {
        let col = if let Some(pc) = &settings.pad_page_colors {
            if !pc.is_empty() { parse_pad_color(&pc[current_page % pc.len()]).unwrap_or(PadColors::Blue) } else { PadColors::Blue }
        } else { PadColors::Blue };
        for p in 0..16 {
            lights.set_pad(p, col, Brightness::Dim);
        }
        for bid in 0..39 {
            if let Some(btn) = num::FromPrimitive::from_usize(bid) {
                if lights.button_has_light(btn) {
                    lights.set_button(btn, Brightness::Dim);
                }
            }
        }
        // Keep Pitch/Mod latched as before
        for i in 0..25 {
            lights.set_slider(i, Brightness::Dim);
        }
        let _ = lights.write(device);
    }

    // Prepare pad color helpers
    let default_pad_color = PadColors::Blue;

    let mut buf = [0u8; 64];
    loop {
        // Arp tick - always synced to clock, even without HID data - cyclic octaves (C1 G1 C2 G2 for Up)
        if arp_enabled && !held_arp_notes.is_empty() && arp_last_tick.elapsed() >= arp_rate {
            if let Some(prev) = arp_current_notes.take() {
                for n in &prev { send_midi(port, settings.pad_midi_channel(), MidiMessage::NoteOff { key: (*n).into(), vel: 0.into() }); }
            }
            // Build full sequence with octaves interleaved
            let mut seq: Vec<Vec<u8>> = Vec::new();
            for oct in 0..arp_octaves {
                for base_vec in &held_arp_notes {
                    for &n in base_vec {
                        let v = ((n as i32 + (oct as i32 * 12)).clamp(0,127)) as u8;
                        if !seq.iter().any(|vv: &Vec<u8>| vv[0]==v) {
                            seq.push(vec![v]);
                        }
                    }
                }
            }
            let len = seq.len();
            if len > 0 {
                let idx = match arp_mode {
                    ArpMode::Up => { let i = arp_pos % len; arp_pos = (arp_pos + 1) % len; i },
                    ArpMode::Down => { let i = (len - 1) - (arp_pos % len); arp_pos = (arp_pos + 1) % len; i },
                    ArpMode::UpDown => {
                        let cycle = if len == 1 { 1 } else { len * 2 - 2 };
                        let pos = arp_pos % cycle;
                        let i = if pos < len { pos } else { cycle - pos };
                        arp_pos = (arp_pos + 1) % cycle;
                        i
                    },
                    ArpMode::DownUp => {
                        let cycle = if len == 1 { 1 } else { len * 2 - 2 };
                        let pos = arp_pos % cycle;
                        let i = if pos < len { len - 1 - pos } else { pos - len + 1 };
                        arp_pos = (arp_pos + 1) % cycle;
                        i
                    },
                    ArpMode::Random => {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut hasher = DefaultHasher::new();
                        arp_pos.hash(&mut hasher);
                        let h = hasher.finish();
                        arp_pos = arp_pos.wrapping_add(1);
                        (h as usize) % len
                    },
                };
                let notes = seq[idx].clone();
                for &n in &notes { send_midi(port, settings.pad_midi_channel(), MidiMessage::NoteOn { key: n.into(), vel: 100.into() }); }
                arp_current_notes = Some(notes);
            }
            arp_last_tick = Instant::now();
        }
        let size = device.read_timeout(&mut buf, 10)?;
        if size < 1 {
            if selector_active {
                if let Some(since) = selector_since {
                    if since.elapsed() > Duration::from_millis(700) {
                        let col = if let Some(pc) = &settings.pad_page_colors {
                            if !pc.is_empty() { parse_pad_color(&pc[current_page % pc.len()]).unwrap_or(PadColors::Blue) } else { PadColors::Blue }
                        } else { PadColors::Blue };
                        for p in 0..16 {
                            lights.set_pad(p, col, Brightness::Dim);
                        }
                        lights.write(device)?;
                        selector_active = false;
                        selector_since = None;
                    }
                }
            }
            continue;
        }

        if selector_active {
            if let Some(since) = selector_since {
                if since.elapsed() > Duration::from_millis(700) {
                    let col = if let Some(pc) = &settings.pad_page_colors {
                        if !pc.is_empty() { parse_pad_color(&pc[current_page % pc.len()]).unwrap_or(PadColors::Blue) } else { PadColors::Blue }
                    } else { PadColors::Blue };
                    for p in 0..16 {
                        lights.set_pad(p, col, Brightness::Dim);
                    }
                    lights.write(device)?;
                    selector_active = false;
                    selector_since = None;
                }
            }
        }
        // Arp tick - clock-synced, cyclic octaves (C1 G1 C2 G2 for Up)
        if arp_enabled && !held_arp_notes.is_empty() && arp_last_tick.elapsed() >= arp_rate {
            if let Some(prev) = arp_current_notes.take() {
                for n in &prev { send_midi(port, settings.pad_midi_channel(), MidiMessage::NoteOff { key: (*n).into(), vel: 0.into() }); }
            }
            let mut seq: Vec<Vec<u8>> = Vec::new();
            for oct in 0..arp_octaves {
                for base_vec in &held_arp_notes {
                    for &n in base_vec {
                        let v = ((n as i32 + (oct as i32 * 12)).clamp(0,127)) as u8;
                        if !seq.iter().any(|vv: &Vec<u8>| vv[0]==v) {
                            seq.push(vec![v]);
                        }
                    }
                }
            }
            let len = seq.len();
            if len > 0 {
                let idx = match arp_mode {
                    ArpMode::Up => { let i = arp_pos % len; arp_pos = (arp_pos + 1) % len; i },
                    ArpMode::Down => { let i = (len - 1) - (arp_pos % len); arp_pos = (arp_pos + 1) % len; i },
                    ArpMode::UpDown => {
                        let cycle = if len == 1 { 1 } else { len * 2 - 2 };
                        let pos = arp_pos % cycle;
                        let i = if pos < len { pos } else { cycle - pos };
                        arp_pos = (arp_pos + 1) % cycle;
                        i
                    },
                    ArpMode::DownUp => {
                        let cycle = if len == 1 { 1 } else { len * 2 - 2 };
                        let pos = arp_pos % cycle;
                        let i = if pos < len { len - 1 - pos } else { pos - len + 1 };
                        arp_pos = (arp_pos + 1) % cycle;
                        i
                    },
                    ArpMode::Random => {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut hasher = DefaultHasher::new();
                        arp_pos.hash(&mut hasher);
                        let h = hasher.finish();
                        arp_pos = arp_pos.wrapping_add(1);
                        (h as usize) % len
                    },
                };
                let notes = seq[idx].clone();
                for &n in &notes { send_midi(port, settings.pad_midi_channel(), MidiMessage::NoteOn { key: n.into(), vel: 100.into() }); }
                arp_current_notes = Some(notes);
            }
            arp_last_tick = Instant::now();
        }

        let mut changed_lights = false;
        let mut page_changed = false;

        if buf[0] == 0x01 {
            // button mode
            // Shift held for transpose octave (Shift+Left/Right)
            let shift_idx = Buttons::Shift as usize;
            let shift_byte = shift_idx / 8 + 1;
            let shift_bit = 1 << (shift_idx % 8);
            let shift_held_current = (buf[shift_byte] & shift_bit) != 0;
            let shift_held = shift_held_current || button_prev[shift_idx];
            for i in 0..6 {
                for j in 0..8 {
                    let idx = i * 8 + j;
                    let button: Option<Buttons> = num::FromPrimitive::from_usize(idx);
                    let button = match button {
                        Some(val) => val,
                        None => continue,
                    };
                    let status = (buf[i + 1] & (1 << j)) != 0;
                    let prev = button_prev[idx];
                    // edge detection for MIDI + paging
                    if status != prev {
                        button_prev[idx] = status;
                        if status {
                            println!("Button press: {:?}", button);
                        } else {
                            println!("Button release: {:?}", button);
                        }

                        // Strip mode: 3-way exclusive Pitch/Mod/Perform - exactly one Bright
                        // Pitch = pitchbend (spring to center 8192), Mod = modwheel CC1 hold, Perform = free CC assignable hold
                        if status && button == Buttons::Pitch {
                            if strip_mode != StripMode::PitchBend {
                                if strip_mode == StripMode::Free { strip_prev_nonfree = StripMode::ModWheel; }
                                // entering Pitch from any other
                                if strip_mode != StripMode::Free { strip_prev_nonfree = strip_mode; }
                                strip_mode = StripMode::PitchBend;
                                println!("Strip mode -> PitchBend");
                            } else {
                                println!("Strip mode -> PitchBend (already)");
                            }
                            if lights.button_has_light(Buttons::Pitch) { lights.set_button(Buttons::Pitch, Brightness::Bright); }
                            if lights.button_has_light(Buttons::Mod) { lights.set_button(Buttons::Mod, Brightness::Dim); }
                            if lights.button_has_light(Buttons::Perform) { lights.set_button(Buttons::Perform, Brightness::Dim); }
                            changed_lights = true;
                        } else if status && button == Buttons::Mod {
                            if strip_mode != StripMode::ModWheel {
                                if strip_mode != StripMode::Free { strip_prev_nonfree = strip_mode; }
                                strip_mode = StripMode::ModWheel;
                                println!("Strip mode -> ModWheel");
                            } else {
                                println!("Strip mode -> ModWheel (already)");
                            }
                            if lights.button_has_light(Buttons::Pitch) { lights.set_button(Buttons::Pitch, Brightness::Dim); }
                            if lights.button_has_light(Buttons::Mod) { lights.set_button(Buttons::Mod, Brightness::Bright); }
                            if lights.button_has_light(Buttons::Perform) { lights.set_button(Buttons::Perform, Brightness::Dim); }
                            changed_lights = true;
                        } else if !status && (button == Buttons::Pitch || button == Buttons::Mod) {
                            // release keep latched
                        } else if status && button == Buttons::NoteRepeat {
                            arp_enabled = !arp_enabled;
                            println!("Arp -> {}", if arp_enabled { "on" } else { "off" });
                            if lights.button_has_light(Buttons::NoteRepeat) {
                                lights.set_button(Buttons::NoteRepeat, if arp_enabled { Brightness::Bright } else { Brightness::Dim });
                            }
                            if !arp_enabled {
                                if let Some(notes) = arp_current_notes.take() {
                                    for n in notes { send_midi(port, settings.pad_midi_channel(), MidiMessage::NoteOff { key: n.into(), vel: 0.into() }); }
                                }
                                held_arp_notes.clear();
                                arp_pos = 0;
                                arp_dir = 1;
                            }
                            changed_lights = true;
                            if arp_enabled {
                                let _ = update_screen_arp(screen, device, current_page, total_pages, transpose_offset, true, &arp_rate_name, arp_octaves);
                            } else {
                                let _ = update_screen_with_transpose(screen, device, current_page, total_pages, transpose_offset);
                            }
                        } else if !status && button == Buttons::NoteRepeat {
                        } else if status && button == Buttons::Notes {
                            arp_mode = arp_mode.next();
                            println!("Arp mode -> {}", arp_mode.name());
                            if lights.button_has_light(Buttons::Notes) {
                                lights.set_button(Buttons::Notes, Brightness::Bright);
                            }
                            changed_lights = true;
                            if arp_enabled {
                                let _ = update_screen_arp(screen, device, current_page, total_pages, transpose_offset, true, &arp_rate_name, arp_octaves);
                            } else {
                                let _ = update_screen_with_transpose(screen, device, current_page, total_pages, transpose_offset);
                            }
                        } else if !status && button == Buttons::Notes {
                            if lights.button_has_light(Buttons::Notes) {
                                lights.set_button(Buttons::Notes, Brightness::Dim);
                                changed_lights = true;
                            }
                        } else if status && button == Buttons::Maschine && arp_enabled {
                            // Maschine = faster (next rate), Star = slower (prev)
                            const RATES: &[(&str, f32)] = &[
                                ("3/4", 3.0), ("1/2", 2.0), ("3/8", 1.5), ("1/3", 1.333), ("1/4", 1.0), ("3/16", 0.75), ("1/6", 0.666), ("1/8", 0.5), ("3/32", 0.375), ("1/12", 0.333), ("1/16", 0.25), ("3/64", 0.1875), ("1/24", 0.166), ("1/32", 0.125), ("3/128", 0.09375), ("1/48", 0.0833), ("1/64", 0.0625), ("1/96", 0.0417),
                            ];
                            let cur_idx = RATES.iter().position(|(n,_)| *n == arp_rate_name).unwrap_or(10);
                            let next_idx = (cur_idx + 1) % RATES.len();
                            let (next_name, beats) = RATES[next_idx];
                            arp_rate = Duration::from_secs_f32(60.0/120.0 * beats);
                            arp_rate_name = next_name.to_string();
                            println!("Arp rate -> {} via Maschine", arp_rate_name);
                            let _ = update_screen_arp(screen, device, current_page, total_pages, transpose_offset, true, &arp_rate_name, arp_octaves);
                            if lights.button_has_light(Buttons::Maschine) {
                                lights.set_button(Buttons::Maschine, Brightness::Bright);
                                changed_lights = true;
                            }
                        } else if !status && button == Buttons::Maschine && arp_enabled {
                            if lights.button_has_light(Buttons::Maschine) {
                                lights.set_button(Buttons::Maschine, Brightness::Dim);
                                changed_lights = true;
                            }
                        } else if status && button == Buttons::Star && arp_enabled {
                            const RATES: &[(&str, f32)] = &[
                                ("3/4", 3.0), ("1/2", 2.0), ("3/8", 1.5), ("1/3", 1.333), ("1/4", 1.0), ("3/16", 0.75), ("1/6", 0.666), ("1/8", 0.5), ("3/32", 0.375), ("1/12", 0.333), ("1/16", 0.25), ("3/64", 0.1875), ("1/24", 0.166), ("1/32", 0.125), ("3/128", 0.09375), ("1/48", 0.0833), ("1/64", 0.0625), ("1/96", 0.0417),
                            ];
                            let cur_idx = RATES.iter().position(|(n,_)| *n == arp_rate_name).unwrap_or(10);
                            let next_idx = (cur_idx as i32 - 1).rem_euclid(RATES.len() as i32) as usize;
                            let (next_name, beats) = RATES[next_idx];
                            arp_rate = Duration::from_secs_f32(60.0/120.0 * beats);
                            arp_rate_name = next_name.to_string();
                            println!("Arp rate -> {} via Star", arp_rate_name);
                            let _ = update_screen_arp(screen, device, current_page, total_pages, transpose_offset, true, &arp_rate_name, arp_octaves);
                            if lights.button_has_light(Buttons::Star) {
                                lights.set_button(Buttons::Star, Brightness::Bright);
                                changed_lights = true;
                            }
                        } else if !status && button == Buttons::Star && arp_enabled {
                            if lights.button_has_light(Buttons::Star) {
                                lights.set_button(Buttons::Star, Brightness::Dim);
                                changed_lights = true;
                            }
                        } else if status && button == Buttons::Tempo && arp_enabled {
                            // Tempo now free (was arp rate) – keep dim, no arp rate here (encoder does rate)
                            if lights.button_has_light(Buttons::Tempo) {
                                lights.set_button(Buttons::Tempo, Brightness::Bright);
                                changed_lights = true;
                            }
                        } else if !status && button == Buttons::Tempo && arp_enabled {
                            if lights.button_has_light(Buttons::Tempo) {
                                lights.set_button(Buttons::Tempo, Brightness::Dim);
                                changed_lights = true;
                            }
                        } else if status && button == Buttons::Sampling && arp_enabled {
                            arp_octaves = if arp_octaves >= 4 { 1 } else { arp_octaves + 1 };
                            println!("Arp octaves -> {}", arp_octaves);
                            if lights.button_has_light(Buttons::Sampling) {
                                lights.set_button(Buttons::Sampling, Brightness::Bright);
                                changed_lights = true;
                            }
                            let _ = update_screen_arp(screen, device, current_page, total_pages, transpose_offset, true, &arp_rate_name, arp_octaves);
                        } else if !status && button == Buttons::Sampling && arp_enabled {
                            if lights.button_has_light(Buttons::Sampling) {
                                lights.set_button(Buttons::Sampling, Brightness::Dim);
                                changed_lights = true;
                            }
                        } else if status && button == Buttons::FixedVol {
                            fixed_vel_active = !fixed_vel_active;
                            println!("FixedVel -> {}", if fixed_vel_active { "127" } else { "vel" });
                            if lights.button_has_light(Buttons::FixedVol) {
                                lights.set_button(Buttons::FixedVol, if fixed_vel_active { Brightness::Bright } else { Brightness::Dim });
                            }
                            changed_lights = true;
                        } else if !status && button == Buttons::FixedVol {
                            // keep LED
                        } else if status && button == Buttons::Chords {
                            chords_active = !chords_active;
                            if chords_active { step_tetrad_active = false; if lights.button_has_light(Buttons::Step) { lights.set_button(Buttons::Step, Brightness::Dim); } }
                            println!("Chords mode -> {}", if chords_active { "triads/power" } else { "single" });
                            if lights.button_has_light(Buttons::Chords) {
                                lights.set_button(Buttons::Chords, if chords_active { Brightness::Bright } else { Brightness::Dim });
                            }
                            if lights.button_has_light(Buttons::Step) {
                                lights.set_button(Buttons::Step, Brightness::Dim);
                            }
                            changed_lights = true;
                            let _ = update_screen_with_transpose(screen, device, current_page, total_pages, transpose_offset);
                        } else if !status && button == Buttons::Chords {
                            // keep LED
                        } else if status && button == Buttons::Step {
                            step_tetrad_active = !step_tetrad_active;
                            if step_tetrad_active { chords_active = false; if lights.button_has_light(Buttons::Chords) { lights.set_button(Buttons::Chords, Brightness::Dim); } }
                            println!("Step mode -> {}", if step_tetrad_active { "tetrad 1-3-5-7 / power+oct" } else { "single" });
                            if lights.button_has_light(Buttons::Step) {
                                lights.set_button(Buttons::Step, if step_tetrad_active { Brightness::Bright } else { Brightness::Dim });
                            }
                            if lights.button_has_light(Buttons::Chords) {
                                lights.set_button(Buttons::Chords, Brightness::Dim);
                            }
                            changed_lights = true;
                        } else if !status && button == Buttons::Step {
                            // keep LED
                        } else if status && button == Buttons::Perform {
                            if strip_mode != StripMode::Free {
                                // enter Free, remember previous
                                strip_prev_nonfree = strip_mode;
                                strip_mode = StripMode::Free;
                                println!("Perform free -> CC assignable (strip free)");
                            } else {
                                // toggle off Free -> back to previous Pitch/Mod
                                strip_mode = strip_prev_nonfree;
                                println!("Perform free -> off (back to {:?})", strip_mode);
                            }
                            if lights.button_has_light(Buttons::Pitch) { lights.set_button(Buttons::Pitch, if strip_mode == StripMode::PitchBend { Brightness::Bright } else { Brightness::Dim }); }
                            if lights.button_has_light(Buttons::Mod) { lights.set_button(Buttons::Mod, if strip_mode == StripMode::ModWheel { Brightness::Bright } else { Brightness::Dim }); }
                            if lights.button_has_light(Buttons::Perform) { lights.set_button(Buttons::Perform, if strip_mode == StripMode::Free { Brightness::Bright } else { Brightness::Dim }); }
                            changed_lights = true;
                        } else if !status && button == Buttons::Perform {
                            // keep LED latched (3-way toggle: release does not change mode)
                        } else if let Some(action) = transpose_action(settings, button) {
                            if status {
                                let delta = match action {
                                    TransposeAction::SemitoneUp => if shift_held { 12 } else { 1 },
                                    TransposeAction::SemitoneDown => if shift_held { -12 } else { -1 },
                                    TransposeAction::OctaveUp => 12,
                                    TransposeAction::OctaveDown => -12,
                                    TransposeAction::Reset => -transpose_offset,
                                };
                                let new_off = (transpose_offset + delta).clamp(-48, 48);
                                if new_off != transpose_offset {
                                    transpose_offset = new_off;
                                    println!("Transpose {:?} -> {}", action, transpose_offset);
                                    if arp_enabled {
                                        let _ = update_screen_arp(screen, device, current_page, total_pages, transpose_offset, true, &arp_rate_name, arp_octaves);
                                    } else {
                                        let _ = update_screen_with_transpose(screen, device, current_page, total_pages, transpose_offset);
                                    }
                                } else if matches!(action, TransposeAction::Reset) {
                                    println!("Transpose reset");
                                    if arp_enabled {
                                        let _ = update_screen_arp(screen, device, current_page, total_pages, transpose_offset, true, &arp_rate_name, arp_octaves);
                                    } else {
                                        let _ = update_screen_with_transpose(screen, device, current_page, total_pages, transpose_offset);
                                    }
                                }
                                if lights.button_has_light(button) {
                                    lights.set_button(button, Brightness::Bright);
                                    changed_lights = true;
                                }
                            } else {
                                if lights.button_has_light(button) {
                                    lights.set_button(button, Brightness::Dim);
                                    changed_lights = true;
                                }
                            }
                        } else if is_auto_page_button(settings, button) && total_pages > 16 {
                            // Auto+Pad for pages 17-32 (index 16..31)
                            if settings.pad_page_hold_select {
                                if status {
                                    auto_page_holding = true;
                                    auto_page_selected_via_pad = false;
                                    if lights.button_has_light(button) {
                                        lights.set_button(button, Brightness::Bright);
                                        changed_lights = true;
                                    }
                                } else {
                                    auto_page_holding = false;
                                    if !auto_page_selected_via_pad {
                                        let base = 16;
                                        let count = total_pages - base;
                                        if count > 0 {
                                            let rel = if current_page >= base { current_page - base } else { 0 };
                                            let next = (rel + 1) % count;
                                            current_page = base + next;
                                            page_changed = true;
                                            println!("Pad page cycled (Auto) -> {}/{}", current_page + 1, total_pages);
                                        }
                                    }
                                    auto_page_selected_via_pad = false;
                                    if lights.button_has_light(button) {
                                        lights.set_button(button, Brightness::Dim);
                                        changed_lights = true;
                                    }
                                }
                            } else if status {
                                let base = 16;
                                let count = total_pages - base;
                                if count > 0 {
                                    let rel = if current_page >= base { current_page - base } else { 0 };
                                    let next = (rel + 1) % count;
                                    current_page = base + next;
                                    page_changed = true;
                                    println!("Pad page (Auto) -> {}/{}", current_page + 1, total_pages);
                                }
                            }
                        } else if is_lock_page_button(settings, button) && total_pages > 32 {
                            // Lock+Pad for pages 33-48 (index 32..47)
                            if settings.pad_page_hold_select {
                                if status {
                                    lock_page_holding = true;
                                    lock_page_selected_via_pad = false;
                                    if lights.button_has_light(button) {
                                        lights.set_button(button, Brightness::Bright);
                                        changed_lights = true;
                                    }
                                } else {
                                    lock_page_holding = false;
                                    if !lock_page_selected_via_pad {
                                        let base = 32;
                                        let count = total_pages - base;
                                        if count > 0 {
                                            let rel = if current_page >= base { current_page - base } else { 0 };
                                            let next = (rel + 1) % count;
                                            current_page = base + next;
                                            page_changed = true;
                                            println!("Pad page cycled (Lock) -> {}/{}", current_page + 1, total_pages);
                                        }
                                    }
                                    lock_page_selected_via_pad = false;
                                    if lights.button_has_light(button) {
                                        lights.set_button(button, Brightness::Dim);
                                        changed_lights = true;
                                    }
                                }
                            } else if status {
                                let base = 32;
                                let count = total_pages - base;
                                if count > 0 {
                                    let rel = if current_page >= base { current_page - base } else { 0 };
                                    let next = (rel + 1) % count;
                                    current_page = base + next;
                                    page_changed = true;
                                    println!("Pad page (Lock) -> {}/{}", current_page + 1, total_pages);
                                }
                            }
                        } else if is_pad_page_button(settings, button) && total_pages > 1 {
                            if settings.pad_page_hold_select {
                                if status {
                                    // press -> start holding
                                    pad_page_holding = true;
                                    page_selected_via_pad = false;
                                    // light the page button bright while holding
                                    if lights.button_has_light(button) {
                                        lights.set_button(button, Brightness::Bright);
                                        changed_lights = true;
                                    }
                                } else {
                                    // release -> if no pad selection happened, cycle
                                    pad_page_holding = false;
                                    if !page_selected_via_pad {
                                        current_page = (current_page + 1) % total_pages;
                                        page_changed = true;
                                        println!("Pad page cycled -> {}/{}", current_page + 1, total_pages);
                                    }
                                    page_selected_via_pad = false;
                                    if lights.button_has_light(button) {
                                        lights.set_button(button, Brightness::Dim);
                                        changed_lights = true;
                                    }
                                }
                            } else {
                                // non-hold mode: page cycle on press only
                                if status {
                                    current_page = (current_page + 1) % total_pages;
                                    page_changed = true;
                                    println!("Pad page -> {}/{}", current_page + 1, total_pages);
                                }
                            }
                            // Don't send MIDI for the paging button itself (reserved)
                            if lights.button_has_light(button) && !settings.pad_page_hold_select {
                                let br = if current_page == 0 { Brightness::Off } else { Brightness::Normal };
                                let _ = br;
                            }
                        } else {
                            // DAW Mackie Control for Play/Rec/Stop if enabled
                            if settings.daw_mackie && matches!(button, Buttons::Play | Buttons::Rec | Buttons::Stop) {
                                let (note, ch) = match button {
                                    Buttons::Play => (94u8, 0u8),
                                    Buttons::Stop => (93u8, 0u8),
                                    Buttons::Rec => (95u8, 0u8),
                                    _ => (0,0),
                                };
                                if status {
                                    send_midi(port, ch.into(), MidiMessage::NoteOn { key: note.into(), vel: 127.into() });
                                    println!("Mackie {:?} -> Note {} ch {}", button, note, ch);
                                } else {
                                    send_midi(port, ch.into(), MidiMessage::NoteOff { key: note.into(), vel: 0.into() });
                                }
                                if lights.button_has_light(button) {
                                    let expected = if status { Brightness::Bright } else { Brightness::Dim };
                                    if lights.get_button(button) != expected {
                                        lights.set_button(button, expected);
                                        changed_lights = true;
                                    }
                                }
                            } else {
                                // Normal button MIDI - keep lit dimly when not pressed
                                handle_button_midi(port, settings, &norm_map, button, status);
                                if lights.button_has_light(button) {
                                    let expected = if status { Brightness::Normal } else { Brightness::Dim };
                                    if lights.get_button(button) != expected {
                                        lights.set_button(button, expected);
                                        changed_lights = true;
                                    }
                                }
                            }
                        }
                    } else if status {
                        // still held, optional debug
                    }
                }
            }
            let encoder_val = buf[7];
            if encoder_val != 0 && should_handle_encoder_rotation(encoder_val) {
                println!("Encoder: {}", encoder_val as i8);
                handle_encoder(port, settings, encoder_val);
                if lights.button_has_light(Buttons::EncoderPress) {
                    lights.set_button(Buttons::EncoderPress, Brightness::Bright);
                    changed_lights = true;
                }
            }
            let slider_val = buf[10];
            let slider_touched = slider_val != 0;
            if slider_touched {
                println!("Slider: {} mode {:?}", slider_val, strip_mode);
                let cnt = (slider_val as i32 - 1 + 5) * 25 / 200 - 1;
                last_slider_cnt = cnt;
                for i in 0..25 {
                    let b = match cnt - i {
                        0 => Brightness::Normal,
                        1..=25 => Brightness::Dim,
                        _ => Brightness::Off,
                    };
                    lights.set_slider(i as usize, b);
                }
                changed_lights = true;
                match strip_mode {
                    StripMode::PitchBend => handle_slider(port, settings, slider_val, true),
                    StripMode::ModWheel => handle_slider(port, settings, slider_val, false),
                    StripMode::Free => {
                        // Free assignable CC - distinct from ModWheel CC1, use CC16 on midi_channel for DAW learn
                        let ch = settings.effective_channel(settings.slider.channel);
                        let scaled = ((slider_val as u16 * 127) / 200).min(127) as u8;
                        let cc: u8 = 16; // free CC, not 1 (ModWheel), assignable
                        println!("Slider Free -> CC {} ch {} val {} raw {}", cc, ch, scaled, slider_val);
                        send_midi(port, ch, MidiMessage::Controller { controller: cc.into(), value: scaled.into() });
                    }
                }
                prev_slider_touched = true;
            } else if prev_slider_touched {
                if arp_enabled {
                    // keep arp rate LEDs dim when released, don't center pitchbend
                    for i in 0..25 { lights.set_slider(i, Brightness::Dim); }
                    changed_lights = true;
                } else if strip_mode == StripMode::PitchBend {
                    // PitchBend spring: reset to center 8192 on finger lift
                    handle_slider(port, settings, 0, true);
                    for i in 0..25 { lights.set_slider(i, Brightness::Dim); }
                    changed_lights = true;
                } else {
                    // ModWheel and Perform free: hold last value (manual reset) - keep strip LEDs at last position
                    if last_slider_cnt >= 0 {
                        for i in 0..25 {
                            let b = match last_slider_cnt - i {
                                0 => Brightness::Normal,
                                1..=25 => Brightness::Dim,
                                _ => Brightness::Off,
                            };
                            lights.set_slider(i as usize, b);
                        }
                    } else {
                        for i in 0..25 { lights.set_slider(i, Brightness::Dim); }
                    }
                    changed_lights = true;
                }
                prev_slider_touched = false;
            }
        } else if buf[0] == 0x02 {
            // pad mode
            for i in (1..buf.len()).step_by(3) {
                let idx = buf[i];
                let evt_raw = buf[i + 1] & 0xf0;
                let val = ((buf[i + 1] as u16 & 0x0f) << 8) + buf[i + 2] as u16;
                if i > 1 && idx == 0 && evt_raw == 0 && val == 0 {
                    break;
                }
                let pad_evt: PadEventType = match num::FromPrimitive::from_u8(evt_raw) {
                    Some(v) => v,
                    None => continue,
                };
                println!("Pad {}: {:?} @ {} (page {})", idx, pad_evt, val, current_page);

                // Hold-select: Group + pad chooses page (1-16)
                if pad_page_holding && settings.pad_page_hold_select && total_pages > 1 {
                    match pad_evt {
                        PadEventType::NoteOn | PadEventType::PressOn => {
                            let page_idx = idx as usize;
                            if page_idx < total_pages && page_idx < 16 {
                                if page_idx != current_page {
                                    current_page = page_idx;
                                    page_changed = true;
                                    page_selected_via_pad = true;
                                    println!("Pad page selected via pad {} -> {}/{}", idx, current_page + 1, total_pages);
                                } else {
                                    page_selected_via_pad = true;
                                }
                            }
                            continue;
                        }
                        _ => continue,
                    }
                }
                // Auto+Pad for pages 17-32 (index 16..31)
                if auto_page_holding && settings.pad_page_hold_select && total_pages > 16 {
                    match pad_evt {
                        PadEventType::NoteOn | PadEventType::PressOn => {
                            let page_idx = 16 + idx as usize;
                            if page_idx < total_pages {
                                if page_idx != current_page {
                                    current_page = page_idx;
                                    page_changed = true;
                                    auto_page_selected_via_pad = true;
                                    println!("Pad page selected via Auto+pad {} -> {}/{}", idx, current_page + 1, total_pages);
                                } else {
                                    auto_page_selected_via_pad = true;
                                }
                            }
                            continue;
                        }
                        _ => continue,
                    }
                }
                // Lock+Pad for pages 33-48 (index 32..47)
                if lock_page_holding && settings.pad_page_hold_select && total_pages > 32 {
                    match pad_evt {
                        PadEventType::NoteOn | PadEventType::PressOn => {
                            let page_idx = 32 + idx as usize;
                            if page_idx < total_pages {
                                if page_idx != current_page {
                                    current_page = page_idx;
                                    page_changed = true;
                                    lock_page_selected_via_pad = true;
                                    println!("Pad page selected via Lock+pad {} -> {}/{}", idx, current_page + 1, total_pages);
                                } else {
                                    lock_page_selected_via_pad = true;
                                }
                            }
                            continue;
                        }
                        _ => continue,
                    }
                }

                if arp_enabled {
                    // Arp arpeggiates held notes; if Chords, use triad/power notes
                    let base_notes: Vec<u8> = if chords_active {
                        let scale_name = settings.scale_names.get(current_page).map(|s| s.as_str());
                        triad_for_pad(&pad_pages[current_page], idx as usize, transpose_offset, scale_name, &settings.chord_types)
                    } else {
                        let base = pad_pages[current_page][idx as usize] as i32;
                        vec![((base + transpose_offset).clamp(0,127)) as u8]
                    };
                    // For arp, expand each base note across octaves but keep held_arp_notes as single notes per pad
                    // The actual octave expansion is done in the arp tick to be cyclic (C1 G1 C2 G2) not grouped (C1 C2 G1 G2)
                    match pad_evt {
                        PadEventType::NoteOn | PadEventType::PressOn => {
                            for n in &base_notes {
                                if !held_arp_notes.iter().any(|v| v[0] == *n) {
                                    held_arp_notes.push(vec![*n]);
                                }
                            }
                            println!("Arp held add {:?} (chords {}) -> held {}", base_notes, chords_active, held_arp_notes.len());
                            lights.set_pad(idx as usize, parse_pad_color(&settings.pad_page_colors.as_ref().and_then(|c| c.get(current_page)).unwrap_or(&"Blue".to_string())).unwrap_or(PadColors::Blue), Brightness::Normal);
                            changed_lights = true;
                        }
                        PadEventType::NoteOff | PadEventType::PressOff => {
                            for n in &base_notes {
                                held_arp_notes.retain(|v| v[0] != *n);
                            }
                            println!("Arp held remove {:?} -> held {}", base_notes, held_arp_notes.len());
                            if held_arp_notes.is_empty() {
                                if let Some(cur) = arp_current_notes.take() {
                                    for n in cur { send_midi(port, settings.pad_midi_channel(), MidiMessage::NoteOff { key: n.into(), vel: 0.into() }); }
                                }
                                arp_pos = 0;
                                arp_dir = 1;
                            }
                            lights.set_pad(idx as usize, parse_pad_color(&settings.pad_page_colors.as_ref().and_then(|c| c.get(current_page)).unwrap_or(&"Blue".to_string())).unwrap_or(PadColors::Blue), Brightness::Dim);
                            changed_lights = true;
                        }
                        _ => {}
                    }
                    continue;
                }

                // Gate: dim by default, normal when pressed, dim when released
                let (_, prev_b) = lights.get_pad(idx as usize);
                let b = match pad_evt {
                    PadEventType::NoteOn | PadEventType::PressOn => Brightness::Normal,
                    PadEventType::NoteOff | PadEventType::PressOff => Brightness::Dim,
                    PadEventType::Aftertouch => {
                        if val > 0 {
                            Brightness::Normal
                        } else {
                            Brightness::Dim
                        }
                    }
                    #[allow(unreachable_patterns)]
                    _ => prev_b,
                };
                // Determine color for this pad
                let pad_color = if let Some(colors) = &settings.pad_colors {
                    if (idx as usize) < colors.len() {
                        parse_pad_color(&colors[idx as usize]).unwrap_or(default_pad_color)
                    } else {
                        default_pad_color
                    }
                } else if let Some(page_colors) = &settings.pad_page_colors {
                    if current_page < page_colors.len() {
                        parse_pad_color(&page_colors[current_page]).unwrap_or(default_pad_color)
                    } else {
                        default_pad_color
                    }
                } else {
                    default_pad_color
                };

                if prev_b != b {
                    lights.set_pad(idx as usize, pad_color, b);
                    changed_lights = true;
                }

                // Resolve note for current page with transpose
                let notes = &pad_pages[current_page];
                if (idx as usize) >= notes.len() {
                    continue;
                }
                let mut velocity = if fixed_vel_active { 127 } else { let mut v = (val >> 5) as u8; if val > 0 && v == 0 { v = 1; } v.min(127) };
                let scaled_vel = velocity;
                let channel = settings.pad_midi_channel();

                // Step: tetrad 1-3-5-7 for 7-tone, power+octave (root+fifth+octave) for non-7
                if step_tetrad_active {
                    let scale_name = settings.scale_names.get(current_page).map(|s| s.as_str());
                    let phys = {
                        let to_phys = |driver: &[u8]| -> Vec<u8> {
                            if driver.len() < 16 { return driver.to_vec(); }
                            vec![driver[12], driver[13], driver[14], driver[15], driver[8], driver[9], driver[10], driver[11], driver[4], driver[5], driver[6], driver[7], driver[0], driver[1], driver[2], driver[3]]
                        };
                        let p = to_phys(notes);
                        let base = p[0] as i32;
                        let mut set = std::collections::HashSet::new();
                        for &n in &p { set.insert((n as i32 - base).rem_euclid(12)); }
                        let mut iv: Vec<i32> = set.into_iter().collect();
                        iv.sort_unstable();
                        iv.len()
                    };
                    let out_notes: Vec<u8> = if phys == 7 {
                        // 7-tone: tetrad 1-3-5-7
                        let mut tmp = settings.chord_types.clone();
                        if let Some(name) = scale_name { tmp.insert(name.to_string(), "tetrad".to_string()); }
                        triad_for_pad(notes, idx as usize, transpose_offset, scale_name, &tmp)
                    } else {
                        // non-7 (chromatic 12, pentatonic 5, blues 6 etc.): power+octave 1-5-8
                        let base_n = notes[idx as usize] as i32;
                        let root = ((base_n + transpose_offset).clamp(0,127)) as u8;
                        let fifth = ((root as i32 + 7).clamp(0,127)) as u8;
                        let oct = ((root as i32 + 12).clamp(0,127)) as u8;
                        vec![root, fifth, oct]
                    };
                    let tetrad = out_notes;
                    match pad_evt {
                        PadEventType::NoteOn | PadEventType::PressOn => {
                            for n in &tetrad { send_midi(port, channel, MidiMessage::NoteOn { key: (*n).into(), vel: scaled_vel.into() }); }
                        }
                        PadEventType::NoteOff | PadEventType::PressOff => {
                            for n in &tetrad { send_midi(port, channel, MidiMessage::NoteOff { key: (*n).into(), vel: scaled_vel.into() }); }
                        }
                        PadEventType::Aftertouch => {
                            match settings.pad_aftertouch.as_str() {
                                "poly" => { for n in &tetrad { send_midi(port, channel, MidiMessage::Aftertouch { key: (*n).into(), vel: scaled_vel.into() }); } },
                                "channel" => send_midi(port, channel, MidiMessage::ChannelAftertouch { vel: scaled_vel.into() }),
                                "cc" => send_midi(port, channel, MidiMessage::Controller { controller: 74u8.into(), value: scaled_vel.into() }),
                                _ => {}
                            }
                        }
                        #[allow(unreachable_patterns)]
                        _ => {}
                    }
                    continue;
                }
                // Chords mode: configurable via [chord_types] (power/triad/tetrad), default triad for 7, power for others
                if chords_active {
                    let scale_name = settings.scale_names.get(current_page).map(|s| s.as_str());
                    let triad = triad_for_pad(notes, idx as usize, transpose_offset, scale_name, &settings.chord_types);
                    match pad_evt {
                        PadEventType::NoteOn | PadEventType::PressOn => {
                            for n in triad {
                                send_midi(port, channel, MidiMessage::NoteOn { key: n.into(), vel: scaled_vel.into() });
                            }
                        }
                        PadEventType::NoteOff | PadEventType::PressOff => {
                            for n in triad {
                                send_midi(port, channel, MidiMessage::NoteOff { key: n.into(), vel: scaled_vel.into() });
                            }
                        }
                        PadEventType::Aftertouch => {
                            match settings.pad_aftertouch.as_str() {
                                "poly" => {
                                    let triad = triad_for_pad(notes, idx as usize, transpose_offset, scale_name, &settings.chord_types);
                                    for n in triad {
                                        send_midi(port, channel, MidiMessage::Aftertouch { key: n.into(), vel: scaled_vel.into() });
                                    }
                                }
                                "channel" => send_midi(port, channel, MidiMessage::ChannelAftertouch { vel: scaled_vel.into() }),
                                "cc" => send_midi(port, channel, MidiMessage::Controller { controller: 74u8.into(), value: scaled_vel.into() }),
                                _ => {}
                            }
                        }
                        #[allow(unreachable_patterns)]
                        _ => {}
                    }
                    continue;
                }

                let base = notes[idx as usize] as i32;
                let note = ((base + transpose_offset).clamp(0, 127)) as u8;

                let event_opt: Option<MidiMessage> = match pad_evt {
                    PadEventType::NoteOn | PadEventType::PressOn => Some(MidiMessage::NoteOn {
                        key: note.into(),
                        vel: scaled_vel.into(),
                    }),
                    PadEventType::NoteOff | PadEventType::PressOff => Some(MidiMessage::NoteOff {
                        key: note.into(),
                        vel: scaled_vel.into(),
                    }),
                    PadEventType::Aftertouch => match settings.pad_aftertouch.as_str() {
                        "poly" => Some(MidiMessage::Aftertouch {
                            key: note.into(),
                            vel: scaled_vel.into(),
                        }),
                        "channel" => Some(MidiMessage::ChannelAftertouch {
                            vel: scaled_vel.into(),
                        }),
                        "cc" => {
                            Some(MidiMessage::Controller {
                                controller: 74u8.into(),
                                value: scaled_vel.into(),
                            })
                        }
                        _ => None,
                    },
                    #[allow(unreachable_patterns)]
                    _ => None,
                };

                if let Some(evt) = event_opt {
                    send_midi(port, channel, evt);
                }
            }
        }

        if page_changed {
            for p in 0..16 {
                let br = if p < total_pages {
                    if p == current_page {
                        Brightness::Bright
                    } else {
                        Brightness::Dim
                    }
                } else {
                    Brightness::Off
                };
                let col = if let Some(page_colors) = &settings.pad_page_colors {
                    if p < page_colors.len() {
                        parse_pad_color(&page_colors[p]).unwrap_or(PadColors::Blue)
                    } else {
                        PadColors::Blue
                    }
                } else {
                    PadColors::Blue
                };
                lights.set_pad(p, col, br);
            }
            changed_lights = true;
            selector_active = true;
            selector_since = Some(Instant::now());
            if arp_enabled {
                let _ = update_screen_arp(screen, device, current_page, total_pages, transpose_offset, true, &arp_rate_name, arp_octaves);
            } else if let Err(e) = update_screen_with_transpose(screen, device, current_page, total_pages, transpose_offset) {
                eprintln!("screen update failed: {:?}", e);
            }
        }

        if changed_lights {
            lights.write(device)?;
        }
    }
}
