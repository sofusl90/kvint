use bevy::prelude::*;

use super::app::{AppState, GameOverState, NetMatch, RematchInProgress, RematchState, SafeAreaInsets};
use super::audio::{ui_text, UiFont};
use super::board::{win_reveal_done, BoardEntity};
use super::net::{NetCommand, NetworkState, RemoteRematchMessage};
use super::pause::PauseToggleButton;
use super::theme::{ButtonIntent, ButtonState, Theme};

#[derive(Component)]
pub struct GameOverOverlay;

#[derive(Component)]
pub struct BackToMenuButton;

#[derive(Component)]
pub struct RematchButton;

#[derive(Component)]
pub struct RematchStatusText;

/// Hides the game-over modal so the board can be inspected.
#[derive(Component)]
pub struct InspectButton;

/// Invisible full-screen tap target while inspecting; a tap brings the modal back.
#[derive(Component)]
pub struct ReturnTapArea;

/// "Tap to return" hint, since the gesture is otherwise invisible.
#[derive(Component)]
pub struct ReturnHint;

pub fn game_over_system(
    mut commands: Commands,
    mut game_over: ResMut<GameOverState>,
    time: Res<Time>,
    font: Res<UiFont>,
    theme: Res<Theme>,
    settings: Res<super::app::GameSettings>,
    net_match: Option<Res<NetMatch>>,
    safe_area: Res<SafeAreaInsets>,
    back_btn: Query<&Interaction, (Changed<Interaction>, With<BackToMenuButton>)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Handle "Back to Menu" click
    for interaction in back_btn.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(AppState::Menu);
            return;
        }
    }

    if !game_over.is_over || game_over.overlay_shown {
        return;
    }
    // Let the winning line light up before showing the overlay.
    if !win_reveal_done(&game_over, &time) {
        return;
    }
    game_over.overlay_shown = true;

    let in_online_match = net_match.is_some();
    if in_online_match {
        commands.insert_resource(RematchState::default());
    }

    let winner_text =
        super::board::winner_label(game_over.winner, &settings, net_match.as_deref());

    commands
        .spawn((
            BoardEntity,
            GameOverOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(theme.color.surface_overlay),
        ))
        .with_children(|root| {
            // Modal sheet
            root.spawn((
                Node { min_width: Val::Px(280.0), ..theme.modal_node() },
                BackgroundColor(theme.color.surface),
                theme.shadow.elev_3(),
            ))
            .with_children(|sheet| {
                sheet.spawn((
                    Text::new(winner_text),
                    ui_text(&font.0, theme.font_size.h1),
                    TextColor(theme.color.text_primary),
                ));

                if in_online_match {
                    // Rematch button — primary CTA
                    let mut node = theme.button_node(theme.size.button_h_tall, Val::Px(240.0), 0.0);
                    node.border = UiRect::all(Val::Px(theme.size.border_thin));
                    sheet
                        .spawn((
                            RematchButton,
                            Button,
                            node,
                            BackgroundColor(theme.button_bg(ButtonIntent::Primary, ButtonState::Normal)),
                            BorderColor::all(theme.color.border),
                            theme.shadow.elev_2(),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("Rematch"),
                                ui_text(&font.0, theme.font_size.subtitle),
                                TextColor(theme.color.text_on_primary),
                            ));
                        });

                    // Status text — communicates "you said yes, waiting for them"
                    sheet.spawn((
                        RematchStatusText,
                        Text::new(""),
                        ui_text(&font.0, theme.font_size.small),
                        TextColor(theme.color.text_muted),
                    ));

                    spawn_inspect_button(sheet, &font.0, &theme);

                    // Back to menu — demoted to secondary
                    let mut node = theme.button_node(theme.size.button_h, Val::Px(240.0), 0.0);
                    node.border = UiRect::all(Val::Px(theme.size.border_thin));
                    sheet
                        .spawn((
                            BackToMenuButton,
                            Button,
                            node,
                            BackgroundColor(theme.button_bg(ButtonIntent::Secondary, ButtonState::Normal)),
                            BorderColor::all(theme.color.border),
                            theme.shadow.elev_2(),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("Back to menu"),
                                ui_text(&font.0, theme.font_size.body),
                                TextColor(theme.color.text_primary),
                            ));
                        });
                } else {
                    spawn_inspect_button(sheet, &font.0, &theme);

                    // Single-player / local match — back to menu (primary)
                    let mut node = theme.button_node(theme.size.button_h_tall, Val::Px(240.0), 0.0);
                    node.border = UiRect::all(Val::Px(theme.size.border_thin));
                    sheet
                        .spawn((
                            BackToMenuButton,
                            Button,
                            node,
                            BackgroundColor(theme.button_bg(ButtonIntent::Primary, ButtonState::Normal)),
                            BorderColor::all(theme.color.border),
                            theme.shadow.elev_2(),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("Back to Menu"),
                                ui_text(&font.0, theme.font_size.subtitle),
                                TextColor(theme.color.text_on_primary),
                            ));
                        });
                }
            });
        });

    // Inspect mode: a full-screen tap target and a hint, both hidden by default.
    let top = safe_area.top_pill_offset();
    commands.spawn((
        BoardEntity,
        ReturnTapArea,
        Button,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::NONE),
    ));
    commands.spawn((
        BoardEntity,
        ReturnHint,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(top),
            right: Val::Px(theme.space.md),
            padding: UiRect::axes(Val::Px(theme.space.md), Val::Px(theme.space.xs + 2.0)),
            border_radius: theme.radius.pill(),
            display: Display::None,
            ..default()
        },
        BackgroundColor(theme.color.surface),
        theme.shadow.elev_2(),
        children![(
            Text::new("Tap to return"),
            ui_text(&font.0, theme.font_size.caption),
            TextColor(theme.color.text_muted),
        )],
    ));
}

fn spawn_inspect_button(
    sheet: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    theme: &Theme,
) {
    let mut node = theme.button_node(theme.size.button_h, Val::Px(240.0), 0.0);
    node.border = UiRect::all(Val::Px(theme.size.border_thin));
    sheet
        .spawn((
            InspectButton,
            Button,
            node,
            BackgroundColor(theme.button_bg(ButtonIntent::Secondary, ButtonState::Normal)),
            BorderColor::all(theme.color.border),
            theme.shadow.elev_2(),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new("Inspect"),
                ui_text(font, theme.font_size.body),
                TextColor(theme.color.text_primary),
            ));
        });
}

/// Toggles `GameOverState::inspecting` from the modal button or the tap target.
pub fn handle_inspect_toggle(
    mut game_over: ResMut<GameOverState>,
    inspect_btns: Query<&Interaction, (Changed<Interaction>, With<InspectButton>)>,
    return_taps: Query<&Interaction, (Changed<Interaction>, With<ReturnTapArea>)>,
) {
    for i in inspect_btns.iter() {
        if *i == Interaction::Pressed {
            game_over.inspecting = true;
        }
    }
    for i in return_taps.iter() {
        if *i == Interaction::Pressed {
            game_over.inspecting = false;
        }
    }
}

/// Shows the overlay unless inspecting, the inspect controls only while inspecting,
/// and hides the pause button once the game is over.
pub fn update_inspect_visibility(
    game_over: Res<GameOverState>,
    mut overlay_q: Query<
        &mut Node,
        (
            With<GameOverOverlay>,
            Without<ReturnTapArea>,
            Without<ReturnHint>,
            Without<PauseToggleButton>,
        ),
    >,
    mut inspect_q: Query<
        &mut Node,
        (
            Or<(With<ReturnTapArea>, With<ReturnHint>)>,
            Without<GameOverOverlay>,
            Without<PauseToggleButton>,
        ),
    >,
    mut pause_q: Query<
        &mut Node,
        (
            With<PauseToggleButton>,
            Without<GameOverOverlay>,
            Without<ReturnTapArea>,
            Without<ReturnHint>,
        ),
    >,
) {
    let inspecting = game_over.is_over && game_over.inspecting;
    let overlay_target = if inspecting { Display::None } else { Display::Flex };
    let inspect_target = if inspecting { Display::Flex } else { Display::None };
    let pause_target = if game_over.is_over { Display::None } else { Display::Flex };

    set_display(overlay_q.iter_mut(), overlay_target);
    set_display(inspect_q.iter_mut(), inspect_target);
    set_display(pause_q.iter_mut(), pause_target);
}

/// Writes `display` only on change, to avoid triggering change detection.
fn set_display<'a, I>(nodes: I, target: Display)
where
    I: IntoIterator<Item = bevy::ecs::change_detection::Mut<'a, Node>>,
{
    for mut n in nodes {
        if n.display != target {
            n.display = target;
        }
    }
}

/// Local "Rematch" button click — forward our consent to the peer.
pub fn handle_rematch_input(
    buttons: Query<&Interaction, (Changed<Interaction>, With<RematchButton>)>,
    rematch: Option<ResMut<RematchState>>,
    net: Res<NetworkState>,
) {
    let Some(mut rematch) = rematch else { return };
    for interaction in buttons.iter() {
        if *interaction == Interaction::Pressed && !rematch.local {
            rematch.local = true;
            net.send(NetCommand::SendRematch);
        }
    }
}

/// Drain RemoteRematchMessage onto the rematch state.
pub fn apply_remote_rematch(
    mut messages: MessageReader<RemoteRematchMessage>,
    rematch: Option<ResMut<RematchState>>,
) {
    if messages.is_empty() {
        return;
    }
    let Some(mut rematch) = rematch else {
        for _ in messages.read() {}
        return;
    };
    for _ in messages.read() {
        rematch.remote = true;
    }
}

/// Once both players agree, bounce through Rematching for a fresh board.
pub fn start_rematch_when_both_consent(
    mut commands: Commands,
    rematch: Option<Res<RematchState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut status_q: Query<&mut Text, With<RematchStatusText>>,
) {
    let Some(rematch) = rematch else { return };

    let label = match (rematch.local, rematch.remote) {
        (true, false) => "Waiting for opponent…",
        (false, true) => "Opponent wants a rematch.",
        _ => "",
    };
    for mut t in status_q.iter_mut() {
        *t = Text::new(label);
    }

    if rematch.local && rematch.remote {
        // Set before the state change, so `cleanup_board` keeps the connection.
        commands.insert_resource(RematchInProgress);
        next_state.set(AppState::Rematching);
    }
}

/// Back to InGame. The connection survives because of `RematchInProgress`.
pub fn start_rematch_bounce(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    commands.remove_resource::<RematchInProgress>();
    commands.remove_resource::<RematchState>();
    next_state.set(AppState::InGame);
}
