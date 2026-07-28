# TikClock

[![Build](https://github.com/GDWhisper/opendeck-tikclock/actions/workflows/build.yml/badge.svg)](https://github.com/GDWhisper/opendeck-tikclock/actions/workflows/build.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

English | [简体中文](README.zh-CN.md)

An [OpenAction](https://openaction.amankhanna.me/) plugin for [OpenDeck](https://github.com/nekename/OpenDeck) that displays a digital clock across your deck keys — **one digit per key**.

Split the current time (HH:MM:SS) into individual digits and lay them out however you like: a full 8-key clock, a compact 3-key layout using two-digits-per-key mode, or anything in between.

![TikClock on a deck — 20:41 with seconds in pair mode](assets/preview.png)

## Features

- **One digit per key** — assign any position of HH:MM:SS to any key
- **Two digits per key** — compact mode showing full hours/minutes/seconds in a single key
- **Blinking colon** — optional, synced to seconds
- **12 / 24 hour clock** — 12h mode drops the leading zero naturally
- **Custom colors** — text and background color per key
- **Run command on press** — every digit doubles as a launcher (`cmd /C` on Windows, `sh -c` on macOS/Linux); leave empty for no action
- **Localized settings UI** — English and 简体中文, following the host language
- **Lightweight & defensive** — single ~1.4 MB native binary; differential rendering sends only changed frames (~3 messages/s for a full 8-key clock), with staggered forced refresh, invalidation debounce and a per-tick send cap so it can never flood the host or device

## Example layouts

| Layout | Keys | Positions |
|---|---|---|
| Full clock | 8 | `H` `H` `:` `M` `M` `:` `S` `S` |
| Compact | 3 | `HH` `MM` `SS` (two digits per key) |
| Minimal | 2 | `HH` `MM` |

## Installation

### From the OpenAction Marketplace

Search for **TikClock** in the [OpenAction Marketplace](https://marketplace.rivul.us/).

### Manual

1. Download `com.gdwhisper.tikclock.zip` from the [latest release](https://github.com/GDWhisper/opendeck-tikclock/releases/latest)
2. Extract, then copy the `com.gdwhisper.tikclock.sdPlugin` folder into your OpenDeck plugins directory:

| Platform | Path |
|---|---|
| Windows | `%appdata%\opendeck\plugins\` |
| macOS | `~/Library/Application Support/opendeck/plugins/` |
| Linux | `~/.config/opendeck/plugins/` |
| Flatpak | `~/.var/app/me.amankhanna.opendeck/config/opendeck/plugins/` |

3. Restart OpenDeck

## Configuration

Add the **Clock Digit** action to a key, then configure it in the property inspector:

| Setting | Description |
|---|---|
| Position | Which part of HH:MM:SS this key shows (single digit, colon, or two-digit pair) |
| 24-hour clock | Toggle between 24h and 12h display |
| Blinking colon | Colon keys blink once per second |
| Text / Background color | Per-key colors |
| On-press command | Shell command executed when the key is pressed (optional) |

## Building from source

```sh
cargo build --release
```

The binary goes into `com.gdwhisper.tikclock.sdPlugin/bin/` (on Windows, `./build.ps1` does both steps). Symlink the `.sdPlugin` folder into your plugins directory for development.

Supported targets: `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`.

## License

[MIT](LICENSE)
