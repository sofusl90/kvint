use bevy::prelude::*;

use crate::game::game_traits::Game;
use crate::BOARD_SIZE;

use super::components::{BoardEntity, DisconnectToast, StoneAnim, WinRing};
use super::visuals::{
    is_my_piece, spawn_highlight_ring, spawn_stone, BoardAssets, BoardLayout, HighlightRing,
    LatestMove,
};

use crate::bevy::app::{AppState, GameBoard, GameOverState, NetMatch, PendingMove};
use crate::bevy::audio::{play_disconnect, play_place_sound, play_win_sound, ui_text, GameSounds, UiFont};
use crate::bevy::computer::ComputerState;
use crate::bevy::net::{NetCommand, NetStatus, NetworkState, RemoteMoveMessage};

// ---------------------------------------------------------------------------
// Move processing
// ---------------------------------------------------------------------------

pub fn process_move(
    mut commands: Commands,
    mut game_board: ResMut<GameBoard>,
    mut pending: ResMut<PendingMove>,
    mut computer: ResMut<ComputerState>,
    mut latest_move: ResMut<LatestMove>,
    settings: Res<crate::bevy::app::GameSettings>,
    net_match: Option<Res<NetMatch>>,
    rings: Query<Entity, With<HighlightRing>>,
    layout: Res<BoardLayout>,
    sounds: Res<GameSounds>,
    assets: Res<BoardAssets>,
    time: Res<Time>,
) {
    let Some(mv) = pending.0.take() else { return };

    let current = game_board.game.current_player;

    // Only one move is highlighted at a time.
    for e in rings.iter() {
        commands.entity(e).despawn();
    }

    game_board.game.apply_action(&mv);
    computer.update_tree(current.opponent(), mv);

    spawn_stone(
        &mut commands,
        &layout,
        &assets,
        mv.x as usize,
        mv.y as usize,
        current,
        Some(time.elapsed_secs()),
    );

    // Ring the opponent's moves. In Local 2P, every move is the opponent's to someone.
    if !is_my_piece(current, settings.mode, net_match.as_deref()) {
        spawn_highlight_ring(
            &mut commands,
            &layout,
            &assets,
            mv.x as usize,
            mv.y as usize,
            Some(time.elapsed_secs()),
        );
    }

    latest_move.0 = Some((mv.x as usize, mv.y as usize));
    play_place_sound(&mut commands, &sounds, current);
}

// ---------------------------------------------------------------------------
// Remote move handling
// ---------------------------------------------------------------------------

pub fn apply_remote_moves(
    mut messages: MessageReader<RemoteMoveMessage>,
    game_board: Res<GameBoard>,
    game_over: Res<GameOverState>,
    mut pending: ResMut<PendingMove>,
    net_match: Option<Res<NetMatch>>,
    net: Res<NetworkState>,
) {
    let Some(net_match) = net_match else {
        // Drain leftover messages so they don't fire on the next match
        for _ in messages.read() {}
        return;
    };
    if game_over.is_over || pending.0.is_some() {
        return;
    }
    if let Some(msg) = messages.read().next() {
        let mv = msg.0;
        // Bounds-check first: the coordinates come off the wire as u8, and `is_legal` would panic.
        if mv.x as usize >= BOARD_SIZE || mv.y as usize >= BOARD_SIZE {
            net.send(NetCommand::Disconnect);
            return;
        }
        // Reject moves on our turn, or a peer could place stones for us.
        if game_board.game.current_player != net_match.local_player.opponent() {
            net.send(NetCommand::Disconnect);
            return;
        }
        if game_board.game.is_legal(&mv) {
            pending.0 = Some(mv);
        } else {
            // An illegal move means the games are out of sync, so disconnect.
            net.send(NetCommand::Disconnect);
        }
    }
}

// ---------------------------------------------------------------------------
// Game-over check + win-line reveal
// ---------------------------------------------------------------------------

pub fn check_game_over(
    mut commands: Commands,
    mut game_board: ResMut<GameBoard>,
    mut game_over: ResMut<GameOverState>,
    sounds: Res<GameSounds>,
    time: Res<Time>,
) {
    if game_over.is_over {
        return;
    }
    if game_board.game.is_game_over() {
        game_over.is_over = true;
        game_over.winning_positions = game_board.game.get_winning_positions();
        game_over.winner = game_board.game.get_winner();
        game_over.revealed_at = Some(time.elapsed_secs());
        game_over.revealed_count = 0;
        play_win_sound(&mut commands, &sounds);
    }
}

/// Time between lighting up each cell of the winning line.
const WIN_REVEAL_INTERVAL: f32 = 0.12;
/// Pause after the last position before the overlay drops in.
const WIN_REVEAL_TAIL: f32 = 0.35;

pub fn reveal_win_line(
    mut commands: Commands,
    mut game_over: ResMut<GameOverState>,
    layout: Res<BoardLayout>,
    assets: Res<BoardAssets>,
    time: Res<Time>,
) {
    if !game_over.is_over {
        return;
    }
    let Some(start) = game_over.revealed_at else { return };
    let elapsed = time.elapsed_secs() - start;
    let total = game_over.winning_positions.len();
    let target = ((elapsed / WIN_REVEAL_INTERVAL).floor() as usize).min(total);

    while game_over.revealed_count < target {
        let i = game_over.revealed_count;
        let (x, y) = game_over.winning_positions[i];
        let pos = layout.cell_center(x, y);
        let radius = layout.cell_size * 0.48;
        let now = time.elapsed_secs();
        commands.spawn((
            BoardEntity,
            WinRing,
            Mesh2d(assets.unit_circle.clone()),
            MeshMaterial2d(assets.win_highlight_mat.clone()),
            Transform::from_translation(pos.extend(2.0))
                .with_scale(Vec3::splat(radius * 0.7)),
            StoneAnim {
                started: now,
                final_scale: radius,
            },
        ));
        game_over.revealed_count += 1;
    }
}

/// Returns true once the staggered reveal + tail pause has elapsed.
pub fn win_reveal_done(game_over: &GameOverState, time: &Time) -> bool {
    let Some(start) = game_over.revealed_at else { return false };
    let total_secs = game_over.winning_positions.len() as f32 * WIN_REVEAL_INTERVAL
        + WIN_REVEAL_TAIL;
    time.elapsed_secs() - start >= total_secs
}

// ---------------------------------------------------------------------------
// Disconnect handling
// ---------------------------------------------------------------------------

/// Set when the connection drops mid-game; the toast shows this long.
#[derive(Resource)]
pub struct DisconnectPending {
    pub until: f32,
}

const DISCONNECT_TOAST_SECS: f32 = 2.0;

/// Online: show a toast when the connection drops, then go back to the menu.
pub fn detect_disconnect(
    mut commands: Commands,
    net: Res<NetworkState>,
    net_match: Option<Res<NetMatch>>,
    pending: Option<Res<DisconnectPending>>,
    font: Res<UiFont>,
    sounds: Res<GameSounds>,
    time: Res<Time>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let connection_lost = net_match.is_some()
        && !matches!(net.status, NetStatus::Connected { .. });

    // Already in disconnect mode — wait out the toast and transition.
    if let Some(p) = pending {
        if time.elapsed_secs() >= p.until {
            commands.remove_resource::<DisconnectPending>();
            next_state.set(AppState::Menu);
        }
        return;
    }

    if !connection_lost {
        return;
    }

    commands.insert_resource(DisconnectPending {
        until: time.elapsed_secs() + DISCONNECT_TOAST_SECS,
    });
    play_disconnect(&mut commands, &sounds);

    // Spawn the toast — same parchment-on-ink style as the game-over overlay.
    commands
        .spawn((
            BoardEntity,
            DisconnectToast,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.165, 0.145, 0.125, 0.78)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Opponent left"),
                ui_text(&font.0, 38.0),
                TextColor(Color::srgb(0.910, 0.878, 0.808)),
            ));
        });
}
