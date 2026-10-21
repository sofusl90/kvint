//! Saves `GameSettings` as a small `key=value` file:
//!
//!   iOS:     <Documents>/settings.txt
//!   macOS:   ~/Library/Application Support/Kvint/settings.txt
//!   Linux:   $XDG_CONFIG_HOME/kvint/settings.txt   (or ~/.config/kvint/...)
//!   Windows: %APPDATA%\Kvint\settings.txt
//!
//! Any read or write error falls back to the defaults.

use bevy::prelude::*;
use std::path::PathBuf;

use super::app::{Difficulty, FirstMove, GameSettings, Mode};
use crate::game::connect_five::Connect5Player;

#[cfg(target_os = "ios")]
extern "C" {
    fn ios_documents_path(buf: *mut u8, buf_len: i32) -> i32;
}

// The `return`s are needed when several cfg branches are compiled.
#[allow(clippy::needless_return)]
fn settings_dir() -> Option<PathBuf> {
    #[cfg(target_os = "ios")]
    {
        let mut buf = [0u8; 1024];
        let n = unsafe { ios_documents_path(buf.as_mut_ptr(), buf.len() as i32) };
        if n <= 0 {
            return None;
        }
        let s = std::str::from_utf8(&buf[..n as usize]).ok()?;
        return Some(PathBuf::from(s));
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/Application Support/Kvint"))
    }
    #[cfg(target_os = "linux")]
    {
        let dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(dir.join("kvint"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA")?;
        Some(PathBuf::from(appdata).join("Kvint"))
    }
    #[cfg(not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "linux",
        target_os = "windows"
    )))]
    {
        None
    }
}

fn settings_path() -> Option<PathBuf> {
    let dir = settings_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("settings.txt"))
}

fn serialize(s: &GameSettings) -> String {
    let mode = match s.mode {
        Mode::SinglePlayer => "single",
        Mode::Local => "local",
        Mode::Online => "online",
    };
    let difficulty = match s.difficulty {
        Difficulty::Easy => "easy".to_string(),
        Difficulty::Medium => "medium".to_string(),
        Difficulty::Hard => "hard".to_string(),
        Difficulty::Extreme => "extreme".to_string(),
        Difficulty::Custom(n) => format!("custom:{n}"),
    };
    let first_move = match s.first_move {
        FirstMove::Player1 => "p1",
        FirstMove::Player2 => "p2",
        FirstMove::Random => "random",
    };
    format!(
        "mode={mode}\ndifficulty={difficulty}\nfirst_move={first_move}\nfriendly_opening={fo}\ndisplay_advanced={da}\nmute_audio={mute}\nname={name}\n",
        fo = s.friendly_opening,
        da = s.display_advanced,
        mute = s.mute_audio,
        name = s.display_name,
    )
}

fn parse(text: &str) -> GameSettings {
    let mut out = GameSettings::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        match (k.trim(), v.trim()) {
            ("mode", "single") => out.mode = Mode::SinglePlayer,
            ("mode", "local") => out.mode = Mode::Local,
            ("mode", "online") => out.mode = Mode::Online,
            ("difficulty", "easy") => out.difficulty = Difficulty::Easy,
            ("difficulty", "medium") => out.difficulty = Difficulty::Medium,
            ("difficulty", "hard") => out.difficulty = Difficulty::Hard,
            ("difficulty", "extreme") => out.difficulty = Difficulty::Extreme,
            ("difficulty", v) => {
                if let Some(n) = v.strip_prefix("custom:").and_then(|n| n.parse().ok()) {
                    out.difficulty = Difficulty::Custom(n);
                }
            }
            ("first_move", "p1") => out.first_move = FirstMove::Player1,
            ("first_move", "p2") => out.first_move = FirstMove::Player2,
            ("first_move", "random") => out.first_move = FirstMove::Random,
            ("friendly_opening", v) => out.friendly_opening = v == "true",
            ("display_advanced", v) => out.display_advanced = v == "true",
            ("mute_audio", v) => out.mute_audio = v == "true",
            ("name", v) if !v.is_empty() => out.display_name = v.to_string(),
            _ => {}
        }
    }
    let _ = Connect5Player::Player1; // re-export check; field used elsewhere
    out
}

/// Loads saved settings, if there are any.
pub fn load_settings(mut settings: ResMut<GameSettings>) {
    let Some(path) = settings_path() else { return };
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    *settings = parse(&text);
    bevy::log::info!("loaded settings from {}", path.display());
}

/// Saves the settings whenever they change.
pub fn save_settings_on_change(settings: Res<GameSettings>, mut last_written: Local<String>) {
    if !settings.is_changed() {
        return;
    }
    let serialized = serialize(&settings);
    if *last_written == serialized {
        return; // initial change-flag without an actual content diff
    }
    let Some(path) = settings_path() else { return };
    if let Err(e) = std::fs::write(&path, &serialized) {
        bevy::log::warn!("failed to save settings to {}: {}", path.display(), e);
        return;
    }
    *last_written = serialized;
}
