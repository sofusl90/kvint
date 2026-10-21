use bevy::prelude::*;

use crate::game::connect_five::Connect5Player;

use super::components::TurnText;

use crate::bevy::app::{GameBoard, GameOverState, GameSettings, Mode, NetMatch};
use crate::bevy::computer::ComputerState;
use crate::bevy::theme::Theme;

// Functions rather than consts, because `Theme::default()` isn't const.
pub(super) fn ui_text_primary() -> Color {
    Theme::default().color.text_primary
}

pub fn update_turn_indicator(
    game_board: Res<GameBoard>,
    game_over: Res<GameOverState>,
    settings: Res<GameSettings>,
    theme: Res<Theme>,
    net_match: Option<Res<NetMatch>>,
    computer: Res<ComputerState>,
    time: Res<Time>,
    mut query: Query<(&mut Text, &mut TextColor), With<TurnText>>,
    mut last: Local<Option<(bool, Connect5Player, bool, usize)>>,
) {
    // "CPU is thinking" with animated dots. The text is left-aligned, so the dots don't shift it.
    let phase = ((time.elapsed_secs() * 2.5) as usize) % 4;
    let key = (
        game_over.is_over,
        game_board.game.current_player,
        computer.thinking,
        phase,
    );
    if *last == Some(key) {
        return;
    }
    *last = Some(key);

    let nm = net_match.as_deref();
    let (label, c) = if game_over.is_over {
        (
            winner_label(game_over.winner, &settings, nm).to_string(),
            theme.color.special_mark_gold,
        )
    } else if computer.thinking {
        let dots = ["", " .", " ..", " ..."][phase];
        (format!("CPU is thinking{dots}"), theme.color.text_primary)
    } else {
        (
            turn_label(game_board.game.current_player, &settings, nm),
            theme.color.text_primary,
        )
    };
    for (mut text, mut color) in query.iter_mut() {
        *text = Text::new(&label);
        *color = TextColor(c);
    }
}

// ---------------------------------------------------------------------------
// Labels, shared with the game-over overlay through `winner_label`.
// ---------------------------------------------------------------------------

pub(super) fn turn_label(
    p: Connect5Player,
    settings: &GameSettings,
    net_match: Option<&NetMatch>,
) -> String {
    if let Some(nm) = net_match {
        // Online — frame the label in terms of the local player.
        return if p == nm.local_player {
            "Your turn".to_string()
        } else {
            "Opponent's turn".to_string()
        };
    }
    if settings.is_computer(p) {
        return "CPU's turn".to_string();
    }
    if matches!(settings.mode, Mode::SinglePlayer) {
        return "Your turn".to_string();
    }
    // Local 2P keeps numbered labels so the players can tell whose turn it is.
    match p {
        Connect5Player::Player1 => "Player 1's turn".to_string(),
        Connect5Player::Player2 => "Player 2's turn".to_string(),
    }
}

pub fn winner_label(
    winner: Option<Connect5Player>,
    settings: &GameSettings,
    net_match: Option<&NetMatch>,
) -> &'static str {
    let Some(w) = winner else { return "Draw" };
    if let Some(nm) = net_match {
        return if w == nm.local_player { "You won" } else { "Opponent won" };
    }
    if settings.is_computer(w) {
        return "CPU won";
    }
    if matches!(settings.mode, Mode::SinglePlayer) {
        return "You won";
    }
    match w {
        Connect5Player::Player1 => "Player 1 wins",
        Connect5Player::Player2 => "Player 2 wins",
    }
}
