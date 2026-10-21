//! Scales the UI uniformly from its 440 × 956 design size (iPhone Pro Max) to fit the window.
//! The board isn't affected; it lays itself out on resize.
//! Polled every frame, since the iOS size override may not send `WindowResized`.

use bevy::prelude::*;
use bevy::ui::UiScale;

const REF_W: f32 = 440.0;
const REF_H: f32 = 956.0;

pub struct UiScalePlugin;

impl Plugin for UiScalePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_ui_scale);
    }
}

fn apply_ui_scale(windows: Query<&Window>, mut ui_scale: ResMut<UiScale>) {
    let Ok(window) = windows.single() else { return };
    let s = (window.width() / REF_W).min(window.height() / REF_H);
    // Only write on change, so UI layout doesn't rerun every frame.
    if (ui_scale.0 - s).abs() > 0.001 {
        ui_scale.0 = s;
    }
}
