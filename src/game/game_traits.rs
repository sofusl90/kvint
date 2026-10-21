use std::fmt::Debug;

pub trait Player: Debug + Copy + Clone + PartialEq + Send + Sync {
    fn to_string(&self) -> String;
}

pub trait Action: Debug + Copy + Clone + PartialEq + Send + Sync {
    fn default() -> Self;
    fn to_string(&self) -> String;
    fn to_index(&self) -> usize;
    fn from_index(idx: usize) -> Self;
}

/// Set of actions keyed by `Action::to_index()`, so MCTS can avoid allocating.
pub trait ActionMask: Debug + Copy + Clone + Default + Send + Sync {
    fn is_empty(&self) -> bool;
    fn count(&self) -> usize;
    /// Removes and returns the index of the `n`-th set bit. Requires `n < self.count()`.
    fn pop_nth(&mut self, n: usize) -> usize;
}

pub trait Game: Debug + Copy + Clone + Default + Send + Sync {
    type PlayerType: Player;
    type ActionType: Action;
    type ActionMask: ActionMask;
    type BoardType;

    fn new(start_player: Self::PlayerType) -> Self;
    fn new_random_start_player() -> Self;
    fn apply_action(&mut self, action: &Self::ActionType);
    fn is_legal(&self, action: &Self::ActionType) -> bool;
    fn legal_actions(&self) -> Vec<Self::ActionType>;
    /// `legal_actions` as a bitmask, for hot paths.
    fn legal_actions_mask(&self) -> Self::ActionMask;
    fn is_game_over(&mut self) -> bool;
    fn get_winner(&self) -> Option<Self::PlayerType>;
    fn to_string(&self) -> String;
    fn get_current_player(&self) -> Self::PlayerType;
    fn get_board(&self) -> Self::BoardType;
    fn is_space_empty(&self, action: &Self::ActionType) -> bool;
}
