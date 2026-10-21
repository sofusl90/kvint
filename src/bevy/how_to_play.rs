//! How to Play — static rules panel.

use bevy::prelude::*;

use super::app::{AppState, SafeAreaInsets};
use super::audio::{ui_text, TitleFont, UiFont};
use super::lobby::ScrollableScreen;
use super::theme::{ButtonIntent, ButtonState, Theme};

#[derive(Component)]
pub struct HowToPlayRoot;

#[derive(Component)]
pub struct HowToPlayBack;

pub fn setup(
    mut commands: Commands,
    font: Res<UiFont>,
    title_font: Res<TitleFont>,
    theme: Res<Theme>,
    safe_area: Res<SafeAreaInsets>,
) {
    let f = font.0.clone();
    let tf = title_font.0.clone();
    let top_pad = safe_area.top.max(20.0) + theme.space.md;
    let bottom_pad = safe_area.bottom + theme.space.md;
    // Content starts at the top and scrolls (`ScrollableScreen`) when it's too tall.
    commands
        .spawn((
            HowToPlayRoot,
            ScrollableScreen,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme.color.bg_root),
        ))
        .with_children(|root| {
            root.spawn(Node {
                width: Val::Percent(100.0),
                max_width: Val::Px(theme.size.column_max_wide),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(theme.space.lg),
                padding: UiRect {
                    left: Val::Px(theme.space.xl),
                    right: Val::Px(theme.space.xl),
                    top: Val::Px(top_pad),
                    bottom: Val::Px(bottom_pad),
                },
                ..default()
            })
            .with_children(|col| {
                spawn_rules_content(col, &f, &tf, &theme);
                col.spawn(Node { height: Val::Px(theme.space.sm), ..default() });
                col.spawn((
                    HowToPlayBack,
                    Button,
                    theme.button_node(theme.size.button_h, Val::Percent(100.0), 0.0),
                    BackgroundColor(theme.button_bg(ButtonIntent::Secondary, ButtonState::Normal)),
                    theme.shadow.elev_2(),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Back to menu"),
                        ui_text(&f, theme.font_size.body),
                        TextColor(theme.color.text_primary),
                    ));
                });
            });
        });
}

/// The rules, shared by the How to play screen and the pause overlay.
pub fn spawn_rules_content(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    title_font: &Handle<Font>,
    theme: &Theme,
) {
    parent.spawn((
        Text::new("HOW TO PLAY"),
        ui_text(title_font, theme.font_size.h2),
        TextColor(theme.color.headline),
        Node { align_self: AlignSelf::Center, ..default() },
    ));

    section(parent, "Legal moves", &[
        "Place your pawn on any cell marked with a gray dot.",
    ], font, theme);

    section(parent, "How to win", &[
        "Win by getting five in a row horizontally, vertically, or diagonally.",
        "Win by getting a pawn in the black square in the center.",
        "Win by getting two pawns in the corners with the same color.",
    ], font, theme);
}

fn section(
    parent: &mut ChildSpawnerCommands,
    heading: &str,
    lines: &[&str],
    font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme.space.lg)),
                row_gap: Val::Px(theme.space.xs + 2.0),
                border_radius: theme.radius.lg(),
                ..default()
            },
            BackgroundColor(theme.color.surface),
            theme.shadow.elev_1(),
        ))
        .with_children(|sec| {
            sec.spawn((
                Text::new(heading),
                ui_text(font, theme.font_size.heading),
                TextColor(theme.color.text_primary),
            ));
            for line in lines {
                sec.spawn((
                    Text::new(*line),
                    ui_text(font, theme.font_size.small),
                    TextColor(theme.color.text_muted),
                ));
            }
        });
}

pub fn cleanup(mut commands: Commands, roots: Query<Entity, With<HowToPlayRoot>>) {
    for e in roots.iter() {
        commands.entity(e).despawn();
    }
}

pub fn interaction(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (With<HowToPlayBack>, Changed<Interaction>),
    >,
    theme: Res<Theme>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, mut bg) in buttons.iter_mut() {
        let state = match interaction {
            Interaction::Hovered | Interaction::Pressed => ButtonState::Hovered,
            _ => ButtonState::Normal,
        };
        *bg = BackgroundColor(theme.button_bg(ButtonIntent::Secondary, state));
        if *interaction == Interaction::Pressed {
            next_state.set(AppState::Menu);
        }
    }
}

pub fn escape_back(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Menu);
    }
}
