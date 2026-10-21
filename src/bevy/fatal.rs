//! Recovery from errors we can't continue from, like the AI worker dying mid-game.
//! [`FatalError::set`] logs the message; [`enforce_menu`] then goes to the menu,
//! and [`drive_toast`] shows it there for a few seconds.

use bevy::prelude::*;

use super::app::AppState;
use super::audio::{ui_text, UiFont};
use super::theme::Theme;

/// How long the error toast stays on-screen, in seconds.
const TOAST_SECS: f32 = 4.5;

/// Set on an unrecoverable error; the systems below take it from there.
#[derive(Resource, Default)]
pub struct FatalError {
    pub message: Option<String>,
    /// Set when the toast is on-screen so we don't spawn a new one each frame.
    toast_shown: bool,
    /// Wall-clock time at which the current toast should be torn down.
    expires_at: f32,
}

impl FatalError {
    /// For code that can't take `Commands`, like worker threads.
    pub fn set(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        bevy::log::error!("Fatal: {}", msg);
        self.message = Some(msg);
        self.toast_shown = false;
    }
}

#[derive(Component)]
pub struct ErrorToast;

/// Goes to the menu while a fatal error is set. Runs in every state.
pub fn enforce_menu(
    err: Res<FatalError>,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if err.message.is_some() && *state.get() != AppState::Menu {
        next_state.set(AppState::Menu);
    }
}

/// Shows the toast and counts it down. Does nothing without a message.
pub fn drive_toast(
    mut commands: Commands,
    mut err: ResMut<FatalError>,
    time: Res<Time>,
    font: Res<UiFont>,
    theme: Res<Theme>,
    state: Res<State<AppState>>,
    toasts: Query<Entity, With<ErrorToast>>,
) {
    if err.message.is_none() {
        return;
    }
    if *state.get() != AppState::Menu {
        // Wait until we're back in Menu before showing anything.
        return;
    }

    if !err.toast_shown {
        let now = time.elapsed_secs();
        spawn_toast(
            &mut commands,
            err.message.as_deref().unwrap_or(""),
            &font.0,
            &theme,
        );
        err.toast_shown = true;
        err.expires_at = now + TOAST_SECS;
        return;
    }

    if time.elapsed_secs() >= err.expires_at {
        for e in toasts.iter() {
            commands.entity(e).despawn();
        }
        err.message = None;
        err.toast_shown = false;
    }
}

fn spawn_toast(commands: &mut Commands, message: &str, font: &Handle<Font>, theme: &Theme) {
    commands
        .spawn((
            ErrorToast,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(60.0),
                left: Val::Percent(8.0),
                right: Val::Percent(8.0),
                padding: UiRect::all(Val::Px(theme.space.lg)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(theme.space.xs),
                border: UiRect::all(Val::Px(theme.size.border_thin)),
                border_radius: theme.radius.lg(),
                ..default()
            },
            BackgroundColor(theme.color.toast_bg),
            BorderColor::all(theme.color.toast_border),
            theme.shadow.elev_3(),
            // High z so it floats over the menu.
            ZIndex(100),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Something went wrong"),
                ui_text(font, theme.font_size.heading),
                TextColor(theme.color.toast_text),
            ));
            p.spawn((
                Text::new(message.to_string()),
                ui_text(font, theme.font_size.small),
                TextColor(theme.color.toast_text),
            ));
        });
}
