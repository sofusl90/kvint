//! Loading screen, shown until the fonts are loaded (3 s timeout).

use bevy::prelude::*;

use super::app::AppState;
use super::audio::{ui_text, TitleFont, UiFont};
use super::theme::Theme;

#[derive(Component)]
pub struct LoadingRoot;

pub fn setup_loading(
    mut commands: Commands,
    title_font: Res<TitleFont>,
    font: Res<UiFont>,
    theme: Res<Theme>,
) {
    commands
        .spawn((
            LoadingRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(theme.space.lg),
                ..default()
            },
            BackgroundColor(theme.color.bg_root),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("KVINT"),
                ui_text(&title_font.0, theme.font_size.display),
                TextColor(theme.color.headline),
            ));
            root.spawn((
                Text::new("Loading…"),
                ui_text(&font.0, theme.font_size.body),
                TextColor(theme.color.text_muted),
            ));
        });
}

pub fn cleanup_loading(mut commands: Commands, roots: Query<Entity, With<LoadingRoot>>) {
    for e in roots.iter() {
        commands.entity(e).despawn();
    }
}

/// Goes to the menu once both fonts are loaded, or after 3 s so a stall can't trap the user.
pub fn check_loading_done(
    asset_server: Res<AssetServer>,
    title_font: Res<TitleFont>,
    font: Res<UiFont>,
    mut elapsed: Local<f32>,
    time: Res<Time>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    *elapsed += time.delta_secs();
    let title_loaded = asset_server.load_state(title_font.0.id()).is_loaded();
    let body_loaded = asset_server.load_state(font.0.id()).is_loaded();
    if (title_loaded && body_loaded) || *elapsed > 3.0 {
        next_state.set(AppState::Menu);
    }
}
