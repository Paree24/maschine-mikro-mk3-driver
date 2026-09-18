use std::collections::HashMap;

use serde::Deserialize;

fn default_midi_channel() -> u8 {
    0
}
fn default_pad_channel() -> u8 {
    9 // channel 10 (0-indexed 9) is standard for drums
}
fn default_client_name() -> String {
    "Maschine Mikro MK3".to_string()
}
fn default_port_name() -> String {
    "Maschine Mikro MK3 MIDI Out".to_string()
}
fn default_pad_page_button() -> String {
    "Group".to_string()
}
fn default_auto_page_button() -> String {
    "Auto".to_string()
}
fn default_encoder_cc() -> u8 {
    14
}
fn default_slider_cc() -> u8 {
    1
}
fn default_button_type() -> String {
    "cc".to_string()
}
fn default_encoder_mode() -> String {
    "relative".to_string()
}
fn default_slider_mode() -> String {
    "absolute".to_string()
}
fn default_aftertouch() -> String {
    "off".to_string()
}
fn default_transpose_up() -> String {
    "Right".to_string()
}
fn default_transpose_down() -> String {
    "Left".to_string()
}
fn default_octave_up() -> String {
    "".to_string()
}
fn default_octave_down() -> String {
    "".to_string()
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ButtonConfig {
    #[serde(rename = "type", default = "default_button_type")]
    pub type_: String,
    #[serde(default)]
    pub cc: Option<u8>,
    #[serde(default)]
    pub note: Option<u8>,
    #[serde(default)]
    pub channel: Option<u8>,
    #[serde(default)]
    pub value_press: Option<u8>,
    #[serde(default)]
    pub value_release: Option<u8>,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            type_: default_button_type(),
            cc: None,
            note: None,
            channel: None,
            value_press: None,
            value_release: None,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct EncoderConfig {
    #[serde(default = "default_encoder_cc")]
    pub cc: u8,
    #[serde(default)]
    pub channel: Option<u8>,
    #[serde(rename = "type", default = "default_button_type")]
    pub type_: String,
    #[serde(default = "default_encoder_mode")]
    pub mode: String,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            cc: default_encoder_cc(),
            channel: None,
            type_: default_button_type(),
            mode: default_encoder_mode(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct SliderConfig {
    #[serde(default = "default_slider_cc")]
    pub cc: u8,
    #[serde(default)]
    pub channel: Option<u8>,
    #[serde(rename = "type", default = "default_button_type")]
    pub type_: String,
    #[serde(default = "default_slider_mode")]
    pub mode: String,
}

impl Default for SliderConfig {
    fn default() -> Self {
        Self {
            cc: default_slider_cc(),
            channel: None,
            type_: default_button_type(),
            mode: default_slider_mode(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct TransposeConfig {
    #[serde(default = "default_transpose_up")]
    pub semitone_up: String,
    #[serde(default = "default_transpose_down")]
    pub semitone_down: String,
    #[serde(default = "default_octave_up")]
    pub octave_up: String,
    #[serde(default = "default_octave_down")]
    pub octave_down: String,
    #[serde(default)]
    pub reset: String,
}

impl Default for TransposeConfig {
    fn default() -> Self {
        Self {
            semitone_up: default_transpose_up(),
            semitone_down: default_transpose_down(),
            octave_up: default_octave_up(),
            octave_down: default_octave_down(),
            reset: String::new(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct Settings {
    // legacy: single page notemap (kept for backward compat)
    #[serde(default)]
    pub notemaps: Vec<u8>,

    // multi-page pad mappings. Each inner Vec must be 16 notes (0-127)
    // If set, it takes precedence over `notemaps`. If not set and `notemaps`
    // is present, a single page is created from `notemaps`.
    #[serde(default)]
    pub pad_pages: Option<Vec<Vec<u8>>>,

    #[serde(default = "default_midi_channel")]
    pub midi_channel: u8,

    #[serde(default = "default_pad_channel")]
    pub pad_channel: u8,

    // Button name that controls pad pages (e.g. "Group"). Empty string disables.
    #[serde(default = "default_pad_page_button")]
    pub pad_page_button: String,

    // Second page button for pages 17-32 (Auto+Pad). Empty to disable.
    #[serde(default = "default_auto_page_button")]
    pub auto_page_button: String,

    // When true, holding pad_page_button + tapping a pad (0-7) directly selects page
    // like NI Controller Editor (Group + Pad). When false, pressing pad_page_button cycles.
    #[serde(default = "default_hold_select")]
    pub pad_page_hold_select: bool,

    #[serde(default = "default_aftertouch")]
    pub pad_aftertouch: String,

    // Optional per-pad LED colors (16 entries, values like "Blue", "Red", etc.)
    // If set, overrides default Blue.
    #[serde(default)]
    pub pad_colors: Option<Vec<String>>,

    // Per-page colors (one color per page, e.g. ["Blue","Green","Yellow",...])
    #[serde(default)]
    pub pad_page_colors: Option<Vec<String>>,

    #[serde(default = "default_client_name")]
    pub client_name: String,
    #[serde(default = "default_port_name")]
    pub port_name: String,

    #[serde(default = "default_buttons")]
    pub buttons: HashMap<String, ButtonConfig>,

    #[serde(default)]
    pub encoder: EncoderConfig,

    #[serde(default)]
    pub slider: SliderConfig,

    #[serde(default)]
    pub transpose: TransposeConfig,
}

fn default_hold_select() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            notemaps: vec![
                49, 27, 31, 57, 48, 47, 43, 59, 36, 38, 46, 51, 36, 38, 42, 44,
            ],
            pad_pages: None,
            midi_channel: default_midi_channel(),
            pad_channel: default_pad_channel(),
            pad_page_button: default_pad_page_button(),
            auto_page_button: default_auto_page_button(),
            pad_page_hold_select: true,
            pad_aftertouch: default_aftertouch(),
            pad_colors: None,
            pad_page_colors: None,
            client_name: default_client_name(),
            port_name: default_port_name(),
            buttons: default_buttons(),
            encoder: EncoderConfig::default(),
            slider: SliderConfig::default(),
            transpose: TransposeConfig::default(),
        }
    }
}

fn default_buttons() -> HashMap<String, ButtonConfig> {
    let mut m = HashMap::new();
    // Populate a sensible default transport + top-row mapping.
    // User can override/extend via example_config.toml
    let defaults: Vec<(&str, u8)> = vec![
        ("Play", 85),
        ("Rec", 86),
        ("Stop", 87),
        ("Restart", 89),
        ("Erase", 90),
        ("Tap", 91),
        ("Follow", 92),
        ("Maschine", 20),
        ("Star", 21),
        ("Browse", 22),
        ("Volume", 23),
        ("Swing", 24),
        ("Tempo", 25),
        ("Plugin", 26),
        ("Sampling", 27),
        ("Left", 28),
        ("Right", 29),
        ("Pitch", 30),
        ("Mod", 31),
        ("Perform", 32),
        ("Notes", 33),
        ("Auto", 34),
        ("Lock", 35),
        ("NoteRepeat", 36),
        ("FixedVol", 37),
        ("PadMode", 38),
        ("Keyboard", 39),
        ("Chords", 40),
        ("Step", 41),
        ("Scene", 42),
        ("Pattern", 43),
        ("Events", 44),
        ("Variation", 45),
        ("Duplicate", 46),
        ("Select", 47),
        ("Solo", 48),
        ("Mute", 49),
        ("EncoderPress", 50),
    ];
    for (name, cc) in defaults {
        m.insert(
            name.to_string(),
            ButtonConfig {
                type_: "cc".to_string(),
                cc: Some(cc),
                note: None,
                channel: None,
                value_press: Some(127),
                value_release: Some(0),
            },
        );
    }
    m
}

impl Settings {
    pub(crate) fn effective_pad_pages(&self) -> Vec<Vec<u8>> {
        if let Some(pages) = &self.pad_pages {
            if !pages.is_empty() {
                return pages.clone();
            }
        }
        if !self.notemaps.is_empty() {
            return vec![self.notemaps.clone()];
        }
        vec![Settings::default().notemaps]
    }

    pub(crate) fn effective_channel(&self, override_ch: Option<u8>) -> u8 {
        override_ch.unwrap_or(self.midi_channel) & 0x0F
    }

    pub(crate) fn pad_midi_channel(&self) -> u8 {
        self.pad_channel & 0x0F
    }

    pub(crate) fn pad_page_button_parsed(&self) -> Option<String> {
        let s = self.pad_page_button.trim();
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    }

    pub(crate) fn auto_page_button_parsed(&self) -> Option<String> {
        let s = self.auto_page_button.trim();
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        // Check legacy notemaps if present and no pages
        if self.pad_pages.is_none() {
            if !self.notemaps.is_empty() {
                let padcnt = self.notemaps.len();
                if padcnt != 16 {
                    return Err(format!("notemaps should be 16 pads exactly (found {padcnt})"));
                }
                if self.notemaps.iter().any(|x| *x >= 128) {
                    return Err("MIDI notes in notemaps should be 0 to 127".to_string());
                }
            }
        }

        if let Some(pages) = &self.pad_pages {
            if pages.is_empty() {
                return Err("pad_pages must contain at least one page".to_string());
            }
            if pages.len() > 32 {
                return Err("pad_pages supports at most 32 pages".to_string());
            }
            for (idx, page) in pages.iter().enumerate() {
                if page.len() != 16 {
                    return Err(format!("pad_pages[{}] should be 16 pads exactly (found {})", idx, page.len()));
                }
                if page.iter().any(|x| *x >= 128) {
                    return Err(format!("MIDI notes in pad_pages[{}] should be 0 to 127", idx));
                }
            }
        }

        if self.midi_channel >= 16 {
            return Err("midi_channel should be 0 to 15".to_string());
        }
        if self.pad_channel >= 16 {
            return Err("pad_channel should be 0 to 15".to_string());
        }

        if self.client_name.is_empty() {
            return Err("Client name must not be empty".to_string());
        }
        if self.port_name.is_empty() {
            return Err("Port name must not be empty".to_string());
        }

        if let Some(cc) = self.encoder.cc.checked_sub(0) {
            if cc >= 128 {
                return Err("encoder cc should be 0 to 127".to_string());
            }
        }
        if self.encoder.cc >= 128 {
            return Err("encoder cc should be 0 to 127".to_string());
        }
        if self.slider.cc >= 128 {
            return Err("slider cc should be 0 to 127".to_string());
        }
        if let Some(ch) = self.encoder.channel {
            if ch >= 16 {
                return Err("encoder channel should be 0 to 15".to_string());
            }
        }
        if let Some(ch) = self.slider.channel {
            if ch >= 16 {
                return Err("slider channel should be 0 to 15".to_string());
            }
        }

        let valid_modes_enc = ["relative", "absolute"];
        if !valid_modes_enc.contains(&self.encoder.mode.as_str()) {
            return Err(format!("encoder mode must be one of {:?}", valid_modes_enc));
        }
        let valid_modes_slider = ["absolute", "relative", "pitchbend"];
        if !valid_modes_slider.contains(&self.slider.mode.as_str()) {
            return Err(format!("slider mode must be one of {:?}", valid_modes_slider));
        }

        let valid_aftertouch = ["off", "poly", "channel", "cc"];
        if !valid_aftertouch.contains(&self.pad_aftertouch.as_str()) {
            return Err(format!("pad_aftertouch must be one of {:?}", valid_aftertouch));
        }

        for (k, v) in &self.buttons {
            if v.type_ != "cc" && v.type_ != "note" && v.type_ != "pc" && v.type_ != "off" {
                return Err(format!("buttons.{k} type must be cc, note, pc or off (found {})", v.type_));
            }
            if let Some(cc) = v.cc {
                if cc >= 128 {
                    return Err(format!("buttons.{k} cc should be 0 to 127"));
                }
            }
            if let Some(note) = v.note {
                if note >= 128 {
                    return Err(format!("buttons.{k} note should be 0 to 127"));
                }
            }
            if let Some(ch) = v.channel {
                if ch >= 16 {
                    return Err(format!("buttons.{k} channel should be 0 to 15"));
                }
            }
            if v.type_ == "cc" && v.cc.is_none() {
                return Err(format!("buttons.{k} type=cc requires cc value"));
            }
            if v.type_ == "note" && v.note.is_none() {
                return Err(format!("buttons.{k} type=note requires note value"));
            }
        }

        if let Some(colors) = &self.pad_colors {
            if colors.len() != 16 {
                return Err(format!("pad_colors should be 16 entries (found {})", colors.len()));
            }
        }

        for (field, val) in [
            ("transpose.semitone_up", &self.transpose.semitone_up),
            ("transpose.semitone_down", &self.transpose.semitone_down),
            ("transpose.octave_up", &self.transpose.octave_up),
            ("transpose.octave_down", &self.transpose.octave_down),
            ("transpose.reset", &self.transpose.reset),
        ] {
            let s = val.trim();
            if !s.is_empty() && Self::parse_button_name(s).is_none() {
                return Err(format!("{field} = \"{s}\" is not a valid button name"));
            }
        }

        if !self.auto_page_button.trim().is_empty() && Self::parse_button_name(self.auto_page_button.trim()).is_none() {
            return Err(format!("auto_page_button = \"{}\" is not a valid button name", self.auto_page_button));
        }

        Ok(())
    }

    fn parse_button_name(s: &str) -> Option<()> {
        let valid = [
            "Maschine","Star","Browse","Volume","Swing","Tempo","Plugin","Sampling",
            "Left","Right","Pitch","Mod","Perform","Notes","Group","Auto","Lock","NoteRepeat",
            "Restart","Erase","Tap","Follow","Play","Rec","Stop","Shift","FixedVol","PadMode",
            "Keyboard","Chords","Step","Scene","Pattern","Events","Variation","Duplicate",
            "Select","Solo","Mute","EncoderPress","EncoderTouch",
        ];
        if valid.iter().any(|v| v.eq_ignore_ascii_case(s)) {
            Some(())
        } else {
            None
        }
    }
}
