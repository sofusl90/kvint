//! In-game pause overlay (Esc or the pause button), plus the How to play and Settings
//! overlays it opens. They sit over the game without changing state.

use bevy::prelude::*;

use super::app::{AppState, GameOverState};
use super::audio::{ui_text, TitleFont, UiFont};
use super::board::{BoardEntity, DisconnectPending};
use super::menu::{spawn_settings_toggles, MenuAction, MenuButton};
use super::theme::{ButtonIntent, ButtonState, Theme};

/// Input handlers must ignore input while this is set.
#[derive(Resource, Default, Debug)]
pub struct Paused(pub bool);

#[derive(Component)]
pub struct PauseOverlay;
#[derive(Component)]
pub struct ResumeButton;
#[derive(Component)]
pub struct MainMenuButton;
/// The on-screen pause button, the mobile version of Esc.
#[derive(Component)]
pub struct PauseToggleButton;

#[derive(Component)]
pub struct OpenHelpOverlayButton;
#[derive(Component)]
pub struct OpenSettingsOverlayButton;
#[derive(Component)]
pub struct HelpOverlay;
#[derive(Component)]
pub struct SettingsOverlay;
#[derive(Component)]
pub struct CloseHelpButton;
#[derive(Component)]
pub struct CloseSettingsButton;

/// Esc or the pause button. Ignored on the game-over screen and the disconnect toast.
pub fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    btns: Query<&Interaction, (Changed<Interaction>, With<PauseToggleButton>)>,
    mut paused: ResMut<Paused>,
    game_over: Res<GameOverState>,
    disconnect: Option<Res<DisconnectPending>>,
) {
    if game_over.is_over {
        // Game-over overlay already owns the screen; force-clear pause.
        paused.0 = false;
        return;
    }
    if disconnect.is_some() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        paused.0 = !paused.0;
    }
    for i in btns.iter() {
        if *i == Interaction::Pressed {
            paused.0 = true;
        }
    }
}

/// The on-screen pause button. Cleaned up with the board.
pub fn spawn_pause_toggle_button(
    commands: &mut Commands,
    font: &Handle<Font>,
    safe_area: super::app::SafeAreaInsets,
) {
    let theme = Theme::default();
    let top = safe_area.top_pill_offset();
    commands
        .spawn((
            BoardEntity,
            PauseToggleButton,
            Button,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(top),
                right: Val::Px(theme.space.md),
                width: Val::Px(64.0),
                height: Val::Px(theme.size.button_h_compact),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: theme.radius.md(),
                ..default()
            },
            BackgroundColor(theme.button_bg(ButtonIntent::Secondary, ButtonState::Normal)),
            theme.shadow.elev_2(),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new("Menu"),
                ui_text(font, theme.font_size.small),
                TextColor(theme.color.text_primary),
            ));
        });
}

/// Spawn the overlay when paused turns on; despawn when it turns off.
pub fn update_pause_overlay(
    mut commands: Commands,
    paused: Res<Paused>,
    overlays: Query<Entity, With<PauseOverlay>>,
    font: Res<UiFont>,
    title_font: Res<TitleFont>,
    theme: Res<Theme>,
) {
    let want_overlay = paused.0;
    let has_overlay = !overlays.is_empty();

    if want_overlay && !has_overlay {
        spawn(&mut commands, &font.0, &title_font.0, &theme);
    } else if !want_overlay && has_overlay {
        for e in overlays.iter() {
            commands.entity(e).despawn();
        }
    }
}

fn spawn(commands: &mut Commands, font: &Handle<Font>, title_font: &Handle<Font>, theme: &Theme) {
    commands
        .spawn((
            BoardEntity, // so cleanup_board / handle_resize can sweep it
            PauseOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(theme.color.surface_overlay),
        ))
        .with_children(|root| {
            // Modal sheet
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(theme.space.xl)),
                    row_gap: Val::Px(theme.space.lg),
                    border_radius: theme.radius.xl(),
                    min_width: Val::Px(280.0),
                    ..default()
                },
                BackgroundColor(theme.color.surface),
                theme.shadow.elev_3(),
            ))
            .with_children(|sheet| {
                sheet.spawn((
                    Text::new("Paused"),
                    ui_text(title_font, theme.font_size.h1),
                    TextColor(theme.color.headline),
                ));

                sheet
                    .spawn((
                        ResumeButton,
                        Button,
                        theme.button_node(theme.size.button_h_tall, Val::Px(240.0), 0.0),
                        BackgroundColor(theme.button_bg(ButtonIntent::Primary, ButtonState::Normal)),
                        theme.shadow.elev_2(),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Resume"),
                            ui_text(font, theme.font_size.subtitle),
                            TextColor(theme.color.text_on_primary),
                        ));
                    });

                // Help + full-settings overlays (open over the paused game).
                spawn_pause_secondary_button::<OpenHelpOverlayButton>(
                    sheet,
                    "How to play",
                    OpenHelpOverlayButton,
                    font,
                    theme,
                );
                spawn_pause_secondary_button::<OpenSettingsOverlayButton>(
                    sheet,
                    "Settings",
                    OpenSettingsOverlayButton,
                    font,
                    theme,
                );

                sheet
                    .spawn((
                        MainMenuButton,
                        Button,
                        theme.button_node(theme.size.button_h, Val::Px(240.0), 0.0),
                        BackgroundColor(theme.button_bg(ButtonIntent::Destructive, ButtonState::Normal)),
                        theme.shadow.elev_2(),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Main menu"),
                            ui_text(font, theme.font_size.body),
                            TextColor(theme.color.text_on_primary),
                        ));
                    });
            });
        });
}

fn spawn_pause_secondary_button<T: Component>(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    marker: T,
    font: &Handle<Font>,
    theme: &Theme,
) {
    // `surface_raised`, since `surface` would blend into the sheet.
    parent
        .spawn((
            marker,
            Button,
            theme.button_node(theme.size.button_h, Val::Px(240.0), 0.0),
            BackgroundColor(theme.color.surface_raised),
            theme.shadow.elev_2(),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                ui_text(font, theme.font_size.body),
                TextColor(theme.color.text_primary),
            ));
        });
}

/// Hover feedback for the pause sheet's secondary buttons.
pub fn handle_pause_secondary_hover(
    theme: Res<Theme>,
    mut help: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<OpenHelpOverlayButton>, Without<OpenSettingsOverlayButton>),
    >,
    mut settings: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<OpenSettingsOverlayButton>, Without<OpenHelpOverlayButton>),
    >,
) {
    for (i, mut bg) in help.iter_mut().chain(settings.iter_mut()) {
        *bg = BackgroundColor(match i {
            Interaction::Hovered | Interaction::Pressed => theme.color.border,
            _ => theme.color.surface_raised,
        });
    }
}

pub fn handle_pause_buttons(
    mut resume: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<ResumeButton>, Without<MainMenuButton>),
    >,
    mut main_menu: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MainMenuButton>, Without<ResumeButton>),
    >,
    theme: Res<Theme>,
    mut paused: ResMut<Paused>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (i, mut bg) in resume.iter_mut() {
        let state = match i {
            Interaction::Hovered | Interaction::Pressed => ButtonState::Hovered,
            _ => ButtonState::Normal,
        };
        *bg = BackgroundColor(theme.button_bg(ButtonIntent::Primary, state));
        if *i == Interaction::Pressed {
            paused.0 = false;
        }
    }
    for (i, mut bg) in main_menu.iter_mut() {
        let state = match i {
            Interaction::Hovered | Interaction::Pressed => ButtonState::Hovered,
            _ => ButtonState::Normal,
        };
        *bg = BackgroundColor(theme.button_bg(ButtonIntent::Destructive, state));
        if *i == Interaction::Pressed {
            paused.0 = false;
            next_state.set(AppState::Menu);
        }
    }
}

// ---------------------------------------------------------------------------
// Help and Settings overlays, opened from the pause menu without leaving InGame.
// ---------------------------------------------------------------------------

pub fn handle_open_overlay_buttons(
    mut commands: Commands,
    help_btns: Query<&Interaction, (Changed<Interaction>, With<OpenHelpOverlayButton>)>,
    settings_btns: Query<&Interaction, (Changed<Interaction>, With<OpenSettingsOverlayButton>)>,
    existing_help: Query<Entity, With<HelpOverlay>>,
    existing_settings: Query<Entity, With<SettingsOverlay>>,
    font: Res<UiFont>,
    title_font: Res<TitleFont>,
    theme: Res<Theme>,
) {
    for i in help_btns.iter() {
        if *i == Interaction::Pressed && existing_help.is_empty() {
            spawn_help_overlay(&mut commands, &font.0, &title_font.0, &theme);
        }
    }
    for i in settings_btns.iter() {
        if *i == Interaction::Pressed && existing_settings.is_empty() {
            spawn_settings_overlay(&mut commands, &font.0, &title_font.0, &theme);
        }
    }
}

/// The three settings toggles inside the in-game overlay.
/// `menu_interaction` can't be registered twice, so this handles just these.
pub fn handle_overlay_settings_toggles(
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    mut settings: ResMut<super::app::GameSettings>,
) {
    for (interaction, btn) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn.0 {
            MenuAction::ToggleMuteAudio => settings.mute_audio = !settings.mute_audio,
            MenuAction::ToggleFriendlyOpening => {
                settings.friendly_opening = !settings.friendly_opening
            }
            MenuAction::ToggleDisplayAdvanced => {
                settings.display_advanced = !settings.display_advanced
            }
            _ => {}
        }
    }
}

/// Closes the overlay; the pause sheet stays underneath.
pub fn handle_close_overlay_buttons(
    mut commands: Commands,
    help_btns: Query<&Interaction, (Changed<Interaction>, With<CloseHelpButton>)>,
    settings_btns: Query<&Interaction, (Changed<Interaction>, With<CloseSettingsButton>)>,
    help_overlays: Query<Entity, With<HelpOverlay>>,
    settings_overlays: Query<Entity, With<SettingsOverlay>>,
) {
    if help_btns.iter().any(|i| *i == Interaction::Pressed) {
        for e in help_overlays.iter() {
            commands.entity(e).despawn();
        }
    }
    if settings_btns.iter().any(|i| *i == Interaction::Pressed) {
        for e in settings_overlays.iter() {
            commands.entity(e).despawn();
        }
    }
}

fn spawn_help_overlay(
    commands: &mut Commands,
    font: &Handle<Font>,
    title_font: &Handle<Font>,
    theme: &Theme,
) {
    commands
        .spawn((
            BoardEntity, // swept by cleanup_board / handle_resize
            HelpOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.color.surface_overlay),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    max_width: Val::Px(theme.size.column_max_wide),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(theme.space.lg),
                    padding: UiRect::all(Val::Px(theme.space.xl)),
                    ..default()
                },
                BackgroundColor(theme.color.surface),
                theme.shadow.elev_3(),
            ))
            .with_children(|sheet| {
                super::how_to_play::spawn_rules_content(sheet, font, title_font, theme);
                sheet.spawn(Node { height: Val::Px(theme.space.sm), ..default() });
                sheet
                    .spawn((
                        CloseHelpButton,
                        Button,
                        theme.button_node(theme.size.button_h, Val::Percent(100.0), 0.0),
                        BackgroundColor(theme.button_bg(
                            ButtonIntent::Secondary,
                            ButtonState::Normal,
                        )),
                        theme.shadow.elev_2(),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Back"),
                            ui_text(font, theme.font_size.body),
                            TextColor(theme.color.text_primary),
                        ));
                    });
            });
        });
}

fn spawn_settings_overlay(
    commands: &mut Commands,
    font: &Handle<Font>,
    title_font: &Handle<Font>,
    theme: &Theme,
) {
    commands
        .spawn((
            BoardEntity,
            SettingsOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.color.surface_overlay),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    max_width: Val::Px(theme.size.column_max),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(theme.space.md),
                    padding: UiRect::all(Val::Px(theme.space.xl)),
                    ..default()
                },
                BackgroundColor(theme.color.surface),
                theme.shadow.elev_3(),
            ))
            .with_children(|sheet| {
                sheet.spawn((
                    Text::new("Settings"),
                    ui_text(title_font, theme.font_size.h1),
                    TextColor(theme.color.headline),
                    Node { align_self: AlignSelf::Center, ..default() },
                ));
                spawn_settings_toggles(sheet, font, theme);
                sheet.spawn(Node { height: Val::Px(theme.space.sm), ..default() });
                sheet
                    .spawn((
                        CloseSettingsButton,
                        Button,
                        theme.button_node(theme.size.button_h, Val::Percent(100.0), 0.0),
                        BackgroundColor(theme.button_bg(
                            ButtonIntent::Secondary,
                            ButtonState::Normal,
                        )),
                        theme.shadow.elev_2(),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Back"),
                            ui_text(font, theme.font_size.body),
                            TextColor(theme.color.text_primary),
                        ));
                    });
            });
        });
}
