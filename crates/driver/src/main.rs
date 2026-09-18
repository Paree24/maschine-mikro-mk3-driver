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

fn update_screen_with_transpose(screen: &mut Screen, device: &HidDevice, page: usize, total: usize, transpose: i32) -> HidResult<()> {
    screen.reset();
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
    // Show transpose offset at top-right small if non-zero: T+12 etc
    if transpose != 0 {
        let sign = if transpose > 0 { 1 } else { 0 };
        // crude: draw a tiny indicator – use first row pixels to show +/- and value
        // Just set a few pixels as marker: top edge bar for transpose active
        for x in 100..126 {
            screen.set(2, x, true);
            screen.set(3, x, true);
        }
        // Show transpose value as digit(s) at top
        let abs_t = transpose.abs() as usize;
        if abs_t < 10 {
            Font::write_digit(screen, 0, 110, abs_t % 10, 1);
        } else {
            Font::write_digit(screen, 0, 100, (abs_t / 10) % 10, 1);
            Font::write_digit(screen, 0, 110, abs_t % 10, 1);
        }
        if sign == 1 {
            // plus sign
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

fn handle_slider(port: &mut MidiOutputConnection, settings: &Settings, raw: u8) {
    if raw == 0 {
        return;
    }
    let ch = settings.effective_channel(settings.slider.channel);
    let cc = settings.slider.cc;
    // raw approx 1..200 as per lights logic -> scale to 0..127
    let scaled = ((raw as u16 * 127) / 200).min(127) as u8;
    // Alternative scaling via 255 if raw larger
    let val = scaled;
    match settings.slider.mode.as_str() {
        "pitchbend" => {
            // map 0..127 to 0..16383 centered? Use simple: val scaled to 14-bit
            let bend_val = (val as u16 * 128) + 8192; // crude center
            let bend = midly::num::u14::from(bend_val.min(16383));
            println!("Slider -> PitchBend ch {} val {} raw {}", ch, bend_val, raw);
            send_midi(
                port,
                ch,
                MidiMessage::PitchBend { bend: midly::PitchBend(bend) },
            );
        }
        _ => {
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
    let mut button_prev = [false; 64];
    let norm_map = normalized_button_map(settings);
    let mut transpose_offset: i32 = 0;

    // For auto-clearing page selector LEDs
    let mut selector_active = false;
    let mut selector_since: Option<Instant> = None;

    // Prepare pad color helpers
    let default_pad_color = PadColors::Blue;

    let mut buf = [0u8; 64];
    loop {
        let size = device.read_timeout(&mut buf, 10)?;
        if size < 1 {
            // still need to handle selector timeout even without HID data
            if selector_active {
                if let Some(since) = selector_since {
                    if since.elapsed() > Duration::from_millis(700) {
                        // clear selector: all pads off
                        for p in 0..16 {
                            lights.set_pad(p, PadColors::Blue, Brightness::Off);
                        }
                        lights.write(device)?;
                        selector_active = false;
                        selector_since = None;
                    }
                }
            }
            continue;
        }

        // selector timeout check also when we have data
        if selector_active {
            if let Some(since) = selector_since {
                if since.elapsed() > Duration::from_millis(700) {
                    for p in 0..16 {
                        lights.set_pad(p, PadColors::Blue, Brightness::Off);
                    }
                    // will be written via changed_lights below
                    lights.write(device)?;
                    selector_active = false;
                    selector_since = None;
                }
            }
        }

        let mut changed_lights = false;
        let mut page_changed = false;

        if buf[0] == 0x01 {
            // button mode
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

                        // Transpose handling (reserved, no MIDI)
                        if let Some(action) = transpose_action(settings, button) {
                            if status {
                                let delta = match action {
                                    TransposeAction::SemitoneUp => 1,
                                    TransposeAction::SemitoneDown => -1,
                                    TransposeAction::OctaveUp => 12,
                                    TransposeAction::OctaveDown => -12,
                                    TransposeAction::Reset => -transpose_offset,
                                };
                                let new_off = (transpose_offset + delta).clamp(-48, 48);
                                if new_off != transpose_offset {
                                    transpose_offset = new_off;
                                    println!("Transpose {:?} -> {}", action, transpose_offset);
                                    // update screen to show transpose
                                    let _ = update_screen_with_transpose(screen, device, current_page, total_pages, transpose_offset);
                                    // brief flash of pad colors to indicate transpose? Keep selector active
                                } else if matches!(action, TransposeAction::Reset) {
                                    println!("Transpose reset");
                                    let _ = update_screen_with_transpose(screen, device, current_page, total_pages, transpose_offset);
                                }
                                if lights.button_has_light(button) {
                                    lights.set_button(button, Brightness::Bright);
                                    changed_lights = true;
                                }
                            } else {
                                if lights.button_has_light(button) {
                                    lights.set_button(button, Brightness::Off);
                                    changed_lights = true;
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
                                        lights.set_button(button, Brightness::Off);
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
                            // Normal button MIDI
                            handle_button_midi(port, settings, &norm_map, button, status);
                            if lights.button_has_light(button) {
                                let light_status = lights.get_button(button) != Brightness::Off;
                                if status != light_status {
                                    lights.set_button(
                                        button,
                                        if status {
                                            Brightness::Normal
                                        } else {
                                            Brightness::Off
                                        },
                                    );
                                    changed_lights = true;
                                }
                            }
                        }
                    } else if status {
                        // still held, optional debug
                    }
                }
            }
            let encoder_val = buf[7];
            if encoder_val != 0 {
                println!("Encoder: {}", encoder_val as i8);
                handle_encoder(port, settings, encoder_val);
            }
            let slider_val = buf[10];
            if slider_val != 0 {
                println!("Slider: {}", slider_val);
                let cnt = (slider_val as i32 - 1 + 5) * 25 / 200 - 1;
                for i in 0..25 {
                    let b = match cnt - i {
                        0 => Brightness::Normal,
                        1..=25 => Brightness::Dim,
                        _ => Brightness::Off,
                    };
                    lights.set_slider(i as usize, b);
                }
                changed_lights = true;
                handle_slider(port, settings, slider_val);
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

                // Hold-select: Group + pad chooses page
                if pad_page_holding && settings.pad_page_hold_select && total_pages > 1 {
                    // Only on NoteOn/PressOn (initial hit) to select
                    match pad_evt {
                        PadEventType::NoteOn | PadEventType::PressOn => {
                            let page_idx = idx as usize;
                            if page_idx < total_pages {
                                if page_idx != current_page {
                                    current_page = page_idx;
                                    page_changed = true;
                                    page_selected_via_pad = true;
                                    println!("Pad page selected via pad {} -> {}/{}", idx, current_page + 1, total_pages);
                                } else {
                                    page_selected_via_pad = true;
                                }
                                // Light feedback: flash selected pad
                            }
                            // Don't send MIDI for the page-select pad hit
                            continue;
                        }
                        _ => continue,
                    }
                }

                // Normal pad handling
                let (_, prev_b) = lights.get_pad(idx as usize);
                let b = match pad_evt {
                    PadEventType::NoteOn | PadEventType::PressOn => Brightness::Normal,
                    PadEventType::NoteOff | PadEventType::PressOff => Brightness::Off,
                    PadEventType::Aftertouch => {
                        if val > 0 {
                            Brightness::Normal
                        } else {
                            Brightness::Off
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
                let base = notes[idx as usize] as i32;
                let note = ((base + transpose_offset).clamp(0, 127)) as u8;
                let mut velocity = (val >> 5) as u8;
                if val > 0 && velocity == 0 {
                    velocity = 1;
                }
                let scaled_vel = velocity.min(127);

                let channel = settings.pad_midi_channel();

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
                            // Send CC 74 (or 1) with pressure - use CC 74 as generic
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
            // Update lights for paging: highlight pads that correspond to pages
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
            if let Err(e) = update_screen_with_transpose(screen, device, current_page, total_pages, transpose_offset) {
                eprintln!("screen update failed: {:?}", e);
            }
        }

        if changed_lights {
            lights.write(device)?;
        }
    }
}
