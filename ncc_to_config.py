#!/usr/bin/env python3
"""
Convert NI Controller Editor .ncc (XML) for Maschine Mikro MK3 to
maschine-mikro-mk3-driver TOML config.

Usage:
  python ncc_to_config.py "Mashine Mikro Config 2.ncc" -o example_config.toml
  python ncc_to_config.py "Mashine Mikro Config 2.ncc"          # stdout

Supports both single-controller .ncc and multi-controller exports.
Picks the latest Maschine Mikro MK3 controller (last occurrence).
"""

import argparse
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

# Mapping from .ncc button id -> driver Buttons enum name
BUTTON_ID_MAP = {
    "Auto": "Auto",
    "Browser": "Browse",
    "Chords": "Chords",
    "Duplicate": "Duplicate",
    "Erase": "Erase",
    "Events": "Events",
    "FixedVel": "FixedVol",
    "Follow": "Follow",
    "Group": "Group",
    "Keyboard": "Keyboard",
    "Lock": "Lock",
    "Maschine": "Maschine",
    "Mod": "Mod",
    "Mute": "Mute",
    "Navigation Push": "EncoderPress",
    "Navigation Touch": "EncoderTouch",
    "NoteRep": "NoteRepeat",
    "Notes": "Notes",
    "PadMode": "PadMode",
    "Pattern": "Pattern",
    "Perform": "Perform",
    "Pitch": "Pitch",
    "Play": "Play",
    "Plugin": "Plugin",
    "Rec": "Rec",
    "Restart": "Restart",
    "Sampling": "Sampling",
    "Scene": "Scene",
    "Select": "Select",
    "Solo": "Solo",
    "Star": "Star",
    "Step": "Step",
    "Stop": "Stop",
    "Swing": "Swing",
    "Tap": "Tap",
    "Tempo": "Tempo",
    "Variation": "Variation",
    "Volume": "Volume",
    # Touchstrip and Navigation wheel handled separately
}

# Reverse for docs
DRIVER_TO_NCC = {v: k for k, v in BUTTON_ID_MAP.items()}

def parse_ncc(path: Path):
    tree = ET.parse(path)
    root = tree.getroot()
    # Find all Maschine Mikro MK3 controllers; pick last (latest)
    ctrls = root.findall(".//controller[@type='Maschine Mikro MK3']")
    if not ctrls:
        # fallback: case-insensitive search or MikroMK3 type variations
        ctrls = [c for c in root.findall(".//controller") if "Mikro" in c.get("type", "") and "MK3" in c.get("type", "")]
    if not ctrls:
        raise SystemExit(f"No Maschine Mikro MK3 controller found in {path}")
    ctrl = ctrls[-1]  # latest
    midi_map = ctrl.find("./midi-map")
    if midi_map is None:
        raise SystemExit("No midi-map found")
    # controls
    controls = midi_map.find("controls")
    buttons = {}
    encoder_cc = None
    encoder_ch = 0
    slider_cc = None
    slider_ch = 0

    if controls is not None:
        for elem in controls:
            eid = elem.get("id")
            disabled = elem.find("disabled") is not None
            if disabled:
                continue
            ctrl_elem = elem.find("controller")
            note_elem = elem.find("note")
            ch_elem = elem.find("channel")
            ch = int(ch_elem.text) if ch_elem is not None else 0
            if elem.tag == "button":
                if eid in BUTTON_ID_MAP:
                    driver_name = BUTTON_ID_MAP[eid]
                    if ctrl_elem is not None:
                        buttons[driver_name] = {"type": "cc", "cc": int(ctrl_elem.text), "channel": ch}
                    elif note_elem is not None:
                        buttons[driver_name] = {"type": "note", "note": int(note_elem.text), "channel": ch}
                # ignore other buttons like TouchstripCap
            elif elem.tag == "wheel" and eid == "Navigation":
                if ctrl_elem is not None:
                    encoder_cc = int(ctrl_elem.text)
                    encoder_ch = ch
            elif elem.tag == "knob" and eid == "Touchstrip":
                if ctrl_elem is not None:
                    slider_cc = int(ctrl_elem.text)
                    slider_ch = ch
            # knob Touchstrip etc handled above
    # Fallback defaults if not found
    if encoder_cc is None:
        encoder_cc = 7
    if slider_cc is None:
        slider_cc = 1

    # groups (pad pages)
    # Driver idx -> Pad id mapping derived from hardware layout:
    # Hardware has Pad1 at bottom-left, Pad16 at top-right.
    # Driver HID idx 0 is top-left, idx 15 bottom-right (vertical flip).
    # So driver idx order = [Pad13,Pad14,Pad15,Pad16, Pad12,Pad11,Pad10,Pad9, Pad8,Pad7,Pad6,Pad5, Pad1,Pad2,Pad3,Pad4]
    # This makes Pad1 appear at bottom where user expects, Pad16 at top.
    DRIVER_IDX_TO_PAD = [13,14,15,16,12,11,10,9,8,7,6,5,1,2,3,4]
    groups = []
    groups_elem = midi_map.find("groups")
    if groups_elem is not None:
        for g in groups_elem.findall("group"):
            name = g.get("name") or "Unnamed"
            pads = {}
            for pad in g.findall("pad"):
                pid = pad.get("id")
                note_elem = pad.find("note")
                if note_elem is not None and pad.get("subtype") == "trigger":
                    pads[pid] = int(note_elem.text)
            if pads:
                # order as driver idx 0..15 via Pad mapping above
                ordered = []
                for pad_num in DRIVER_IDX_TO_PAD:
                    key = f"Pad{pad_num}"
                    ordered.append(pads.get(key, 0))
                groups.append((name, ordered))
            else:
                # group without pads (e.g., CC pages) – skip for pad_pages
                continue

    return {
        "buttons": buttons,
        "encoder": {"cc": encoder_cc, "channel": encoder_ch},
        "slider": {"cc": slider_cc, "channel": slider_ch},
        "groups": groups,
    }

def generate_toml(data):
    buttons = data["buttons"]
    encoder = data["encoder"]
    slider = data["slider"]
    groups = data["groups"]

    lines = []
    lines.append("# Generated from Windows Controller Editor .ncc")
    lines.append("# Source: Mashine Mikro Config 2.ncc (latest, 16 groups)")
    lines.append("# Converted with ncc_to_config.py — edit as needed")
    lines.append("")
    lines.append('client_name = "Maschine Mikro MK3"')
    lines.append('port_name = "Maschine Mikro MK3 MIDI Out"')
    lines.append("")
    lines.append("midi_channel = 0")
    lines.append("pad_channel = 9")
    lines.append("")
    lines.append("# Pad pages from Windows groups (in file order).")
    lines.append("# Driver idx order is permuted: idx0=top-left=Pad13, idx12=bottom-left=Pad1")
    lines.append("# Mapping: driver idx 0..15 -> Pad [13,14,15,16,12,11,10,9,8,7,6,5,1,2,3,4]")
    lines.append("# This makes Pad1 appear at bottom (where hardware has 1) and Pad16 at top,")
    lines.append("# giving sequential chromatic left->right bottom->top physically.")
    lines.append("pad_pages = [")
    for name, notes in groups:
        # Use repr to keep Python list style -> TOML compatible
        lines.append(f"  # {name}")
        lines.append(f"  {notes},")
    lines.append("]")
    lines.append("")
    lines.append('pad_page_button = "Group"')
    lines.append("pad_page_hold_select = true")
    lines.append('pad_aftertouch = "poly"')
    lines.append("")
    lines.append("# Encoder (Navigation wheel) and Slider (Touchstrip) from Windows")
    lines.append("[encoder]")
    lines.append(f"cc = {encoder['cc']}")
    if encoder["channel"] != 0:
        lines.append(f"channel = {encoder['channel']}")
    lines.append('type = "cc"')
    lines.append('mode = "relative"')
    lines.append("")
    lines.append("[slider]")
    lines.append(f"cc = {slider['cc']}")
    if slider["channel"] != 0:
        lines.append(f"channel = {slider['channel']}")
    lines.append('type = "cc"')
    lines.append('mode = "absolute"')
    lines.append("")
    lines.append("[buttons]")
    # Sort buttons for stable output
    for name in sorted(buttons.keys()):
        cfg = buttons[name]
        if cfg["type"] == "cc":
            ch_part = f", channel = {cfg['channel']}" if cfg["channel"] != 0 else ""
            lines.append(f"{name} = {{ type = \"cc\", cc = {cfg['cc']}{ch_part} }}")
        elif cfg["type"] == "note":
            ch_part = f", channel = {cfg['channel']}" if cfg["channel"] != 0 else ""
            lines.append(f"{name} = {{ type = \"note\", note = {cfg['note']}{ch_part} }}")
    # Add missing buttons as commented defaults for reference
    all_driver_buttons = [
        "Maschine","Star","Browse","Volume","Swing","Tempo","Plugin","Sampling",
        "Left","Right","Pitch","Mod","Perform","Notes","Group","Auto","Lock","NoteRepeat",
        "Restart","Erase","Tap","Follow","Play","Rec","Stop","Shift","FixedVol","PadMode",
        "Keyboard","Chords","Step","Scene","Pattern","Events","Variation","Duplicate",
        "Select","Solo","Mute","EncoderPress","EncoderTouch"
    ]
    missing = [b for b in all_driver_buttons if b not in buttons]
    if missing:
        lines.append("")
        lines.append("# Not mapped in Windows config (driver will use defaults if omitted):")
        for b in missing:
            lines.append(f"# {b} = {{ type = \"cc\", cc = 0 }}")
    lines.append("")
    return "\n".join(lines)

def main():
    ap = argparse.ArgumentParser(description="Convert Maschine Mikro MK3 .ncc to driver TOML")
    ap.add_argument("ncc", help="Path to .ncc file (e.g., 'Mashine Mikro Config 2.ncc')")
    ap.add_argument("-o", "--output", help="Output TOML path (default: stdout)")
    args = ap.parse_args()
    data = parse_ncc(Path(args.ncc))
    toml = generate_toml(data)
    if args.output:
        Path(args.output).write_text(toml + "\n", encoding="utf-8")
        print(f"Wrote {args.output} with {len(data['groups'])} pad pages, {len(data['buttons'])} buttons")
    else:
        print(toml)

if __name__ == "__main__":
    main()
