mod components;
mod flow;
mod interaction;
mod ui;
mod visuals;

// Used by `bevy::app`, `pause` and `game_over`.

pub use components::{BoardEntity, HoveredCell, StoneAnim};
pub use flow::{
    apply_remote_moves, check_game_over, detect_disconnect, process_move, reveal_win_line,
    win_reveal_done, DisconnectPending,
};
pub use interaction::{hover_piece, human_input, update_legal_dots};
pub use ui::{update_turn_indicator, winner_label};
pub use visuals::{animate_stones, cleanup_board, handle_resize, setup_board, setup_board_assets};
