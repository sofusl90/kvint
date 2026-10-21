use bevy::prelude::*;

use crate::game::connect_five::{Connect5Player, Move};
use crate::game::game_traits::Game;

use super::components::{BoardPosition, HoverPieceMarker, HoveredCell, LegalDotMarker};
use super::visuals::{screen_to_world, BoardLayout};

use crate::bevy::app::{GameBoard, GameOverState, NetMatch, PendingMove};
use crate::bevy::computer::ComputerState;
use crate::bevy::net::{NetCommand, NetworkState};
use crate::bevy::pause::Paused;

pub fn human_input(
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    game_board: Res<GameBoard>,
    layout: Res<BoardLayout>,
    computer: Res<ComputerState>,
    game_over: Res<GameOverState>,
    mut pending: ResMut<PendingMove>,
    net: Res<NetworkState>,
    net_match: Option<Res<NetMatch>>,
    paused: Res<Paused>,
) {
    if game_over.is_over || computer.thinking || pending.0.is_some() || paused.0 {
        return;
    }
    let current = game_board.game.current_player;
    if computer.is_computer_turn(current) {
        return;
    }
    // Online, ignore taps on the opponent's turn.
    if let Some(net_match) = &net_match {
        if current != net_match.local_player {
            return;
        }
    }

    // Tap position: prefer a fresh touch, otherwise a fresh mouse click
    let Ok(window) = windows.single() else { return };
    let tap_screen = if let Some(touch) = touches.iter_just_pressed().next() {
        Some(touch.position())
    } else if mouse.just_pressed(MouseButton::Left) {
        window.cursor_position()
    } else {
        None
    };
    let Some(screen) = tap_screen else { return };
    let world = screen_to_world(window, screen);

    if let Some((cx, cy)) = layout.world_to_cell(world) {
        let mv = Move::new(cx, cy);
        if game_board.game.is_legal(&mv) {
            // Send it to the peer first for the lowest latency.
            if net_match.is_some() {
                net.send(NetCommand::SendMove(mv));
            }
            pending.0 = Some(mv);
        }
    }
}

pub fn hover_piece(
    game_board: Res<GameBoard>,
    layout: Res<BoardLayout>,
    computer: Res<ComputerState>,
    game_over: Res<GameOverState>,
    windows: Query<&Window>,
    mut hovers: Query<(&HoverPieceMarker, &mut Transform, &mut Visibility)>,
    mut hovered_cell: ResMut<HoveredCell>,
) {
    // Only write Visibility and Transform on change, to avoid triggering change detection.
    let target: Option<(usize, usize, Connect5Player)> = (|| {
        if game_over.is_over || computer.thinking {
            return None;
        }
        let current = game_board.game.current_player;
        if computer.is_computer_turn(current) {
            return None;
        }
        let window = windows.single().ok()?;
        let cursor = window.cursor_position()?;
        let (cx, cy) = layout.world_to_cell(screen_to_world(window, cursor))?;
        if !game_board.game.is_legal(&Move::new(cx, cy)) {
            return None;
        }
        Some((cx, cy, current))
    })();

    for (marker, mut transform, mut vis) in hovers.iter_mut() {
        let (want_vis, want_pos) = match target {
            Some((cx, cy, current)) if marker.0 == current => {
                (Visibility::Inherited, Some(layout.cell_center(cx, cy)))
            }
            _ => (Visibility::Hidden, None),
        };
        if *vis != want_vis {
            *vis = want_vis;
        }
        if let Some(p) = want_pos {
            let new_t = p.extend(0.5);
            if transform.translation != new_t {
                transform.translation = new_t;
            }
        }
    }

    // So `update_legal_dots` skips the dot under the preview piece.
    let new_cell = target.map(|(cx, cy, _)| (cx, cy));
    if hovered_cell.0 != new_cell {
        hovered_cell.0 = new_cell;
    }
}

pub fn update_legal_dots(
    game_board: Res<GameBoard>,
    game_over: Res<GameOverState>,
    hovered: Res<HoveredCell>,
    mut dots: Query<(&BoardPosition, &mut Visibility), With<LegalDotMarker>>,
) {
    let game_over_now = game_over.is_over;
    for (pos, mut vis) in dots.iter_mut() {
        let is_hovered = hovered.0 == Some((pos.x, pos.y));
        let want = if !game_over_now
            && !is_hovered
            && game_board.game.is_legal(&Move::new(pos.x, pos.y))
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        // Only write on change: 121 dots would otherwise trigger change detection every frame.
        if *vis != want {
            *vis = want;
        }
    }
}
