use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver};

use crate::game::connect_five::{Connect5Player, ConnectFive, Move};
use crate::hand_crafted_model::my_mcts::Tree;
use super::app::{GameBoard, GameOverState, GameSettings, PendingMove};


/// Minimum think time, so fast searches don't look instant.
const MIN_THINK_SECS: f32 = 0.9;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct ComputerState {
    trees: [Option<Tree<ConnectFive>>; 2],
    iterations: [Option<usize>; 2],
    receiver: Option<Receiver<ComputerResult>>,
    pub thinking: bool,
    /// When the current search started, for the minimum think time.
    thinking_started: Option<f32>,
}

struct ComputerResult {
    player_index: usize,
    tree: Tree<ConnectFive>,
    chosen_move: Move,
}

impl ComputerState {
    pub fn new(game: ConnectFive, settings: &GameSettings) -> Self {
        let iterations = settings.computer_iterations();
        let trees = [
            iterations[0].map(|_| Tree::new(game, crate::C)),
            iterations[1].map(|_| Tree::new(game, crate::C)),
        ];
        Self {
            trees,
            iterations,
            receiver: None,
            thinking: false,
            thinking_started: None,
        }
    }

    pub fn player_index(player: Connect5Player) -> usize {
        match player {
            Connect5Player::Player1 => 0,
            Connect5Player::Player2 => 1,
        }
    }

    pub fn update_tree(&mut self, player: Connect5Player, mv: Move) {
        let idx = Self::player_index(player);
        if let Some(ref mut tree) = self.trees[idx] {
            tree.update_root(vec![mv]);
        }
    }

    pub fn is_computer_turn(&self, player: Connect5Player) -> bool {
        self.trees[Self::player_index(player)].is_some()
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

pub fn start_computer_turn(
    game_board: Res<GameBoard>,
    mut computer: ResMut<ComputerState>,
    game_over: Res<GameOverState>,
    settings: Res<GameSettings>,
    pending: Res<PendingMove>,
    time: Res<Time>,
) {
    // `game_over.is_over` can lag a frame behind, so check the board too.
    if computer.thinking || game_over.is_over || game_board.game.is_game_over {
        return;
    }
    if pending.0.is_some() {
        return;
    }

    let current = game_board.game.current_player;
    let idx = ComputerState::player_index(current);
    let Some(iterations) = computer.iterations[idx] else { return };

    let Some(tree) = computer.trees[idx].take() else { return };

    // Friendly opening: on an empty board, start at the bottom edge instead of rushing the centre.
    let preset_move = if settings.friendly_opening && game_board.game.is_empty_board() {
        Some(Move::new(crate::BOARD_SIZE / 2, 0))
    } else {
        None
    };

    let (tx, rx) = bounded(1);
    computer.receiver = Some(rx);
    computer.thinking = true;
    computer.thinking_started = Some(time.elapsed_secs());

    std::thread::spawn(move || {
        let mut tree = tree;
        let chosen = if let Some(mv) = preset_move {
            mv
        } else {
            // The game is already over: drop the tree and let `check_game_over` handle it.
            let Some(mv) = tree.monte_carlo_tree_search(iterations) else {
                return;
            };
            mv
        };
        tree.update_root(vec![chosen]);
        let _ = tx.send(ComputerResult {
            player_index: idx,
            tree,
            chosen_move: chosen,
        });
    });
}

pub fn poll_computer(
    mut computer: ResMut<ComputerState>,
    mut pending: ResMut<PendingMove>,
    game_board: Res<GameBoard>,
    mut fatal: ResMut<super::fatal::FatalError>,
    time: Res<Time>,
) {
    use crossbeam_channel::TryRecvError;

    if computer.receiver.is_none() {
        return;
    }

    // Hold the result until the minimum think time has passed.
    if let Some(started) = computer.thinking_started {
        if time.elapsed_secs() - started < MIN_THINK_SECS {
            return;
        }
    }

    let rx = computer.receiver.as_ref().expect("checked above");
    match rx.try_recv() {
        Ok(result) => {
            computer.trees[result.player_index] = Some(result.tree);
            pending.0 = Some(result.chosen_move);
            computer.thinking = false;
            computer.thinking_started = None;
            computer.receiver = None;
        }
        Err(TryRecvError::Empty) => {
            // Worker still thinking — try again next frame.
        }
        Err(TryRecvError::Disconnected) => {
            // No move was sent: fine if the game is over, otherwise the worker crashed.
            computer.thinking = false;
            computer.thinking_started = None;
            computer.receiver = None;
            if !game_board.game.is_game_over {
                fatal.set("AI worker stopped unexpectedly");
            }
        }
    }
}
