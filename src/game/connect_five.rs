use bevy::ecs::component::Component;

use crate::BOARD_SIZE;

use super::game_traits::{Action, ActionMask, Game, Player};

type Board = [[Piece; BOARD_SIZE]; BOARD_SIZE];
type Bitboard = u128;

// 121 cells fit in one u128. Bit index = x * BOARD_SIZE + y.
const TOTAL_BITS: u32 = (BOARD_SIZE * BOARD_SIZE) as u32;
const _: () = assert!(TOTAL_BITS <= 128, "BOARD_SIZE too large for u128 bitboard");

const BOARD_MASK: Bitboard = if TOTAL_BITS == 128 {
    Bitboard::MAX
} else {
    (1u128 << TOTAL_BITS) - 1
};

const fn col_mask(col: usize) -> Bitboard {
    let mut m: Bitboard = 0;
    let mut x = 0;
    while x < BOARD_SIZE {
        m |= 1u128 << (x * BOARD_SIZE + col);
        x += 1;
    }
    m
}

// Masks that stop shifts from wrapping across rows.
const LAST_COL: Bitboard = col_mask(BOARD_SIZE - 1);
const FIRST_COL: Bitboard = col_mask(0);
const NOT_LAST_COL: Bitboard = BOARD_MASK & !LAST_COL;
const NOT_FIRST_COL: Bitboard = BOARD_MASK & !FIRST_COL;

#[inline(always)]
const fn bit(x: usize, y: usize) -> Bitboard {
    1u128 << (x * BOARD_SIZE + y)
}

// Special always-legal cells: midpoints of each edge.
const SPECIAL_CELLS: Bitboard = bit(BOARD_SIZE / 2, 0)
    | bit(BOARD_SIZE / 2, BOARD_SIZE - 1)
    | bit(0, BOARD_SIZE / 2)
    | bit(BOARD_SIZE - 1, BOARD_SIZE / 2);

// Shifts mask out cells that would wrap across a row or fall off the board.
#[inline(always)] fn shift_e (b: Bitboard) -> Bitboard { ((b & NOT_LAST_COL ) << 1)                 & BOARD_MASK }
#[inline(always)] fn shift_w (b: Bitboard) -> Bitboard {  (b & NOT_FIRST_COL) >> 1 }
#[inline(always)] fn shift_s (b: Bitboard) -> Bitboard {  (b << BOARD_SIZE)                          & BOARD_MASK }
#[inline(always)] fn shift_n (b: Bitboard) -> Bitboard {   b >> BOARD_SIZE }
#[inline(always)] fn shift_se(b: Bitboard) -> Bitboard { ((b & NOT_LAST_COL ) << (BOARD_SIZE + 1))  & BOARD_MASK }
#[inline(always)] fn shift_sw(b: Bitboard) -> Bitboard { ((b & NOT_FIRST_COL) << (BOARD_SIZE - 1))  & BOARD_MASK }

/// Returns the far end of a five in a row in the shift direction, if any.
#[inline(always)]
fn five_in_dir(b: Bitboard, shift: fn(Bitboard) -> Bitboard) -> Option<u32> {
    let s1 = shift(b);
    let s2 = shift(s1);
    let s3 = shift(s2);
    let s4 = shift(s3);
    let hit = b & s1 & s2 & s3 & s4;
    if hit == 0 { None } else { Some(hit.trailing_zeros()) }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Connect5Player {
    Player1,
    Player2,
}

impl Connect5Player {
    pub fn opponent(&self) -> Self {
        match self {
            Self::Player1 => Self::Player2,
            Self::Player2 => Self::Player1,
        }
    }
}

impl Player for Connect5Player {
    fn to_string(&self) -> String {
        match self {
            Self::Player1 => "O".to_string(),
            Self::Player2 => "X".to_string(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Component)]
pub enum Piece {
    Occupied(Connect5Player),
    Empty,
}

impl std::fmt::Display for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Occupied(player) => f.write_str(&player.to_string()),
            Self::Empty => f.write_str(" "),
        }
    }
}

/// `u8` coordinates keep MCTS nodes small.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Move {
    pub x: u8,
    pub y: u8,
}

impl Move {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x: x as u8, y: y as u8 }
    }
}

impl Action for Move {
    fn to_string(&self) -> String {
        format!("{} {}", self.x, self.y)
    }

    fn to_index(&self) -> usize {
        self.x as usize * BOARD_SIZE + self.y as usize
    }

    fn from_index(idx: usize) -> Self {
        Self { x: (idx / BOARD_SIZE) as u8, y: (idx % BOARD_SIZE) as u8 }
    }

    fn default() -> Self {
        Self { x: 0, y: 0 }
    }
}

/// One bit per cell, same layout as `Move::to_index()`.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Connect5ActionMask(pub Bitboard);

impl ActionMask for Connect5ActionMask {
    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    fn count(&self) -> usize {
        self.0.count_ones() as usize
    }

    /// A linear bit walk. A popcount bisect was tried and made MCTS ~50% slower.
    #[inline]
    fn pop_nth(&mut self, n: usize) -> usize {
        let mut bits = self.0;
        for _ in 0..n {
            bits &= bits - 1;
        }
        let idx = bits.trailing_zeros() as usize;
        self.0 &= !(1u128 << idx);
        idx
    }
}

#[derive(Debug, Copy, Clone, Component)]
pub struct ConnectFive {
    p1: Bitboard,
    p2: Bitboard,
    pub current_player: Connect5Player,
    pub is_game_over: bool,
}

impl ConnectFive {
    /// True iff no piece has been placed yet.
    #[inline]
    pub fn is_empty_board(&self) -> bool {
        (self.p1 | self.p2) == 0
    }
}

impl Game for ConnectFive {
    type PlayerType = Connect5Player;
    type ActionType = Move;
    type ActionMask = Connect5ActionMask;
    type BoardType = Board;

    fn new(current_player: Connect5Player) -> Self {
        Self {
            p1: 0,
            p2: 0,
            current_player,
            is_game_over: false,
        }
    }

    fn new_random_start_player() -> Self {
        let current_player = if rand::random::<bool>() {
            Connect5Player::Player1
        } else {
            Connect5Player::Player2
        };
        Self::new(current_player)
    }

    /// precondition: is_legal(x, y) must be true
    fn apply_action(&mut self, _move: &Self::ActionType) {
        // Hot path: callers bounds-check the move (network moves in `apply_remote_moves`).
        let b = bit(_move.x as usize, _move.y as usize);
        let player_board = match self.current_player {
            Connect5Player::Player1 => {
                self.p1 |= b;
                self.p1
            }
            Connect5Player::Player2 => {
                self.p2 |= b;
                self.p2
            }
        };

        // Only the stone just placed can win, so check it here and cache the result.
        if !self.is_game_over {
            let centre = bit(BOARD_SIZE / 2, BOARD_SIZE / 2);
            if b == centre {
                self.is_game_over = true;
            } else {
                let nw = bit(0, 0);
                let ne = bit(0, BOARD_SIZE - 1);
                let sw = bit(BOARD_SIZE - 1, 0);
                let se = bit(BOARD_SIZE - 1, BOARD_SIZE - 1);
                let corner_capture = (player_board & nw != 0 && player_board & se != 0)
                    || (player_board & sw != 0 && player_board & ne != 0);
                if corner_capture
                    || five_in_dir(player_board, shift_e).is_some()
                    || five_in_dir(player_board, shift_s).is_some()
                    || five_in_dir(player_board, shift_se).is_some()
                    || five_in_dir(player_board, shift_sw).is_some()
                {
                    self.is_game_over = true;
                }
            }
        }

        self.current_player = self.current_player.opponent();
    }

    fn is_legal(&self, _move: &Move) -> bool {
        // Hot path — bounds already checked by caller.
        let b = bit(_move.x as usize, _move.y as usize);
        let occupied = self.p1 | self.p2;
        if occupied & b != 0 {
            return false;
        }
        let neighbours = shift_n(occupied) | shift_s(occupied) | shift_e(occupied) | shift_w(occupied);
        (neighbours | SPECIAL_CELLS) & b != 0
    }

    fn legal_actions(&self) -> Vec<Move> {
        let mut bits = self.legal_actions_mask().0;
        let mut moves = Vec::with_capacity(bits.count_ones() as usize);
        while bits != 0 {
            let i = bits.trailing_zeros() as usize;
            moves.push(Move { x: (i / BOARD_SIZE) as u8, y: (i % BOARD_SIZE) as u8 });
            bits &= bits - 1;
        }
        moves
    }

    #[inline]
    fn legal_actions_mask(&self) -> Connect5ActionMask {
        let occupied = self.p1 | self.p2;
        let empty = !occupied & BOARD_MASK;
        let neighbours = shift_n(occupied) | shift_s(occupied) | shift_e(occupied) | shift_w(occupied);
        Connect5ActionMask(empty & (neighbours | SPECIAL_CELLS))
    }

    fn is_game_over(&mut self) -> bool {
        // Cached by `apply_action`.
        self.is_game_over
    }

    fn get_winner(&self) -> Option<Connect5Player> {
        if !self.is_game_over {
            None
        } else {
            Some(self.current_player.opponent())
        }
    }

    fn to_string(&self) -> String {
        let mut s = String::new();
        let border = "-".repeat(BOARD_SIZE * 2 + 1);
        s.push_str(&border);
        s.push('\n');

        let mid = BOARD_SIZE / 2;

        for j in (0..BOARD_SIZE).rev() {
            s.push('|');
            for i in 0..BOARD_SIZE {
                let cell = self.piece_at(i, j);
                if i == mid && j == mid && cell == Piece::Empty {
                    s.push('█');
                } else {
                    s.push_str(&cell.to_string());
                }
                s.push('|');
            }
            s.push('\n');
            s.push_str(&border);
            s.push('\n');
        }

        s
    }

    fn get_current_player(&self) -> Connect5Player {
        self.current_player
    }

    fn get_board(&self) -> Board {
        let mut board = [[Piece::Empty; BOARD_SIZE]; BOARD_SIZE];
        for (x, row) in board.iter_mut().enumerate() {
            for (y, cell) in row.iter_mut().enumerate() {
                *cell = self.piece_at(x, y);
            }
        }
        board
    }

    fn is_space_empty(&self, _move: &Move) -> bool {
        let b = bit(_move.x as usize, _move.y as usize);
        (self.p1 | self.p2) & b == 0
    }
}

impl Default for ConnectFive {
    fn default() -> Self {
        ConnectFive::new(Connect5Player::Player1)
    }
}

impl ConnectFive {
    pub fn piece_at(&self, x: usize, y: usize) -> Piece {
        let b = bit(x, y);
        if self.p1 & b != 0 {
            Piece::Occupied(Connect5Player::Player1)
        } else if self.p2 & b != 0 {
            Piece::Occupied(Connect5Player::Player2)
        } else {
            Piece::Empty
        }
    }

    pub fn get_winning_positions(&self) -> Vec<(usize, usize)> {
        let center = bit(BOARD_SIZE / 2, BOARD_SIZE / 2);
        if (self.p1 | self.p2) & center != 0 {
            return vec![(BOARD_SIZE / 2, BOARD_SIZE / 2)];
        }
        let nw = bit(0, 0);
        let ne = bit(0, BOARD_SIZE - 1);
        let sw = bit(BOARD_SIZE - 1, 0);
        let se = bit(BOARD_SIZE - 1, BOARD_SIZE - 1);
        for player in [self.p1, self.p2] {
            if player & nw != 0 && player & se != 0 {
                return vec![(0, 0), (BOARD_SIZE - 1, BOARD_SIZE - 1)];
            }
            if player & sw != 0 && player & ne != 0 {
                return vec![(BOARD_SIZE - 1, 0), (0, BOARD_SIZE - 1)];
            }
        }

        // `five_in_dir` returns the far end of the line; walk back from it by `step`.
        let dirs: [(fn(Bitboard) -> Bitboard, usize); 4] = [
            (shift_e,  1),
            (shift_s,  BOARD_SIZE),
            (shift_se, BOARD_SIZE + 1),
            (shift_sw, BOARD_SIZE - 1),
        ];
        for &player in &[self.p1, self.p2] {
            for &(shift, step) in &dirs {
                if let Some(tail) = five_in_dir(player, shift) {
                    let tail = tail as usize;
                    return (0..5)
                        .map(|i| {
                            let p = tail - i * step;
                            (p / BOARD_SIZE, p % BOARD_SIZE)
                        })
                        .collect();
                }
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand_crafted_model::my_mcts::Tree;
    use Connect5Player::{Player1 as P1, Player2 as P2};

    /// Places a piece, ignoring legality and turn order.
    fn place(g: &mut ConnectFive, who: Connect5Player, x: usize, y: usize) {
        g.current_player = who;
        g.apply_action(&Move::new(x, y));
    }

    #[test]
    fn five_in_a_row_east() {
        let mut g = ConnectFive::new(P1);
        for y in 0..5 {
            place(&mut g, P1, 3, y);
        }
        assert!(g.is_game_over());
        let pos = g.get_winning_positions();
        assert_eq!(pos.len(), 5);
        for y in 0..5 {
            assert!(pos.contains(&(3, y)), "missing (3, {y})");
        }
    }

    #[test]
    fn five_in_a_row_se_diagonal() {
        // shift_se direction: +x, +y. Path avoids the centre (5,5).
        let mut g = ConnectFive::new(P1);
        for i in 0..5 {
            place(&mut g, P1, i, i + 1);
        }
        assert!(g.is_game_over());
        let pos = g.get_winning_positions();
        assert_eq!(pos.len(), 5);
        for i in 0..5 {
            assert!(pos.contains(&(i, i + 1)), "missing ({i}, {})", i + 1);
        }
    }

    #[test]
    fn five_in_a_row_sw_diagonal() {
        // shift_sw direction: +x, -y. Path avoids the centre.
        let mut g = ConnectFive::new(P1);
        for i in 0..5 {
            place(&mut g, P1, i, 9 - i);
        }
        assert!(g.is_game_over());
        let pos = g.get_winning_positions();
        assert_eq!(pos.len(), 5);
        for i in 0..5 {
            assert!(pos.contains(&(i, 9 - i)), "missing ({i}, {})", 9 - i);
        }
    }

    #[test]
    fn nw_se_corner_capture() {
        let last = BOARD_SIZE - 1;
        let mut g = ConnectFive::new(P1);
        place(&mut g, P1, 0, 0);
        assert!(!g.is_game_over(), "single corner shouldn't win");
        place(&mut g, P1, last, last);
        assert!(g.is_game_over(), "NW + SE should win");
        let pos = g.get_winning_positions();
        assert!(pos.contains(&(0, 0)));
        assert!(pos.contains(&(last, last)));
    }

    #[test]
    fn sw_ne_corner_capture() {
        let last = BOARD_SIZE - 1;
        let mut g = ConnectFive::new(P1);
        place(&mut g, P1, last, 0);
        assert!(!g.is_game_over(), "single corner shouldn't win");
        place(&mut g, P1, 0, last);
        assert!(g.is_game_over(), "SW + NE should win");
        let pos = g.get_winning_positions();
        assert!(pos.contains(&(last, 0)));
        assert!(pos.contains(&(0, last)));
    }

    #[test]
    fn corner_capture_requires_diagonal_pair() {
        let last = BOARD_SIZE - 1;

        // Same-edge pairs must NOT trigger corner capture.
        for &(a, b) in &[
            ((0, 0), (last, 0)),       // west edge: NW + SW
            ((0, 0), (0, last)),       // north edge: NW + NE
            ((0, last), (last, last)), // east edge: NE + SE
            ((last, 0), (last, last)), // south edge: SW + SE
        ] {
            let mut g = ConnectFive::new(P1);
            place(&mut g, P1, a.0, a.1);
            place(&mut g, P1, b.0, b.1);
            assert!(
                !g.is_game_over(),
                "corners {a:?} + {b:?} share an edge; should not win"
            );
        }

        // Mixed ownership of a diagonal pair must NOT win either.
        let mut g = ConnectFive::new(P1);
        place(&mut g, P1, 0, 0);
        place(&mut g, P2, last, last);
        assert!(
            !g.is_game_over(),
            "NW (P1) + SE (P2) is not a single-player capture"
        );
    }

    #[test]
    fn legal_actions_matches_mask() {
        // Build a non-trivial mid-game position; assert Vec and mask agree.
        let mut g = ConnectFive::new(P1);
        place(&mut g, P1, 5, 0);
        place(&mut g, P2, 0, 5);
        place(&mut g, P1, 5, 1);
        place(&mut g, P2, 0, 6);
        place(&mut g, P1, 4, 1);
        assert!(!g.is_game_over());

        let actions = g.legal_actions();
        let mask = g.legal_actions_mask();
        assert_eq!(actions.len(), mask.count(), "Vec and mask differ in count");

        let mut expected_mask = 0u128;
        for m in &actions {
            let bit_idx = m.x as usize * BOARD_SIZE + m.y as usize;
            expected_mask |= 1u128 << bit_idx;
        }
        assert_eq!(expected_mask, mask.0, "Vec and mask differ in membership");

        // Sanity: occupied cells aren't legal.
        let occupied = [(5, 0), (0, 5), (5, 1), (0, 6), (4, 1)];
        for (x, y) in occupied {
            assert!(
                !actions.contains(&Move::new(x, y)),
                "({x},{y}) is occupied but listed as legal"
            );
        }
    }

    #[test]
    fn pop_nth_iterates_set_bits_in_order() {
        // pop_nth is on the MCTS hot path; this protects its tricky bit walk.
        let bits = [3u128, 7, 41, 80, 100];
        let mut raw = 0u128;
        for &b in &bits {
            raw |= 1u128 << b;
        }
        let mut mask = Connect5ActionMask(raw);
        assert_eq!(mask.count(), bits.len());

        // Pop the 3rd set bit (41), then the 2nd (7).
        let popped = mask.pop_nth(2);
        assert_eq!(popped, 41);
        assert_eq!(mask.count(), 4);
        assert_eq!(mask.0 & (1u128 << 41), 0, "bit 41 should be cleared");

        let popped = mask.pop_nth(1);
        assert_eq!(popped, 7);
        assert_eq!(mask.count(), 3);

        // Pop remaining in order to confirm full iteration.
        let mut remaining: Vec<usize> = Vec::new();
        while !mask.is_empty() {
            remaining.push(mask.pop_nth(0));
        }
        assert_eq!(remaining, vec![3, 80, 100]);
    }

    #[test]
    fn center_capture_ends_game() {
        let mut game = ConnectFive::new(Connect5Player::Player1);
        let mid = BOARD_SIZE / 2;
        game.apply_action(&Move::new(mid, 0));
        game.apply_action(&Move::new(mid, BOARD_SIZE - 1));
        for y in 1..mid {
            game.apply_action(&Move::new(mid, y));
            if !game.is_game_over() {
                game.apply_action(&Move::new(mid, BOARD_SIZE - 1 - y));
            }
        }
        if !game.is_game_over() {
            game.apply_action(&Move::new(mid, mid));
            assert!(game.is_game_over());
            assert_eq!(game.get_winning_positions(), vec![(mid, mid)]);
        }
    }

    #[test]
    fn five_in_a_row_horizontal() {
        let mut game = ConnectFive::new(Connect5Player::Player1);
        let mid = BOARD_SIZE / 2;
        game.apply_action(&Move::new(0, mid));
        game.apply_action(&Move::new(mid, 0));
        game.apply_action(&Move::new(1, mid));
        game.apply_action(&Move::new(mid, 1));
        game.apply_action(&Move::new(2, mid));
        game.apply_action(&Move::new(mid, 2));
        game.apply_action(&Move::new(3, mid));
        game.apply_action(&Move::new(mid, 3));
        game.apply_action(&Move::new(4, mid));

        assert!(game.is_game_over());
        let positions = game.get_winning_positions();
        assert_eq!(positions.len(), 5);
        for i in 0..5 {
            assert!(positions.contains(&(i, mid)));
        }
    }

    #[test]
    fn diagonal_corner_capture() {
        let mut game = ConnectFive::new(Connect5Player::Player1);
        let last = BOARD_SIZE - 1;
        let mid = BOARD_SIZE / 2;
        game.apply_action(&Move::new(mid, 0));
        game.apply_action(&Move::new(0, mid));
        game.apply_action(&Move::new(mid - 1, 0));
        game.apply_action(&Move::new(0, mid - 1));
        game.apply_action(&Move::new(mid - 2, 0));
        game.apply_action(&Move::new(0, mid - 2));
        game.apply_action(&Move::new(mid - 3, 0));
        game.apply_action(&Move::new(0, mid - 3));
        game.apply_action(&Move::new(mid - 4, 0));
        if game.is_game_over() {
            return;
        }
        game.apply_action(&Move::new(0, mid - 4));
        game.apply_action(&Move::new(0, 0));
        if game.is_game_over() {
            return;
        }
        game.apply_action(&Move::new(1, 0));
        game.apply_action(&Move::new(mid, BOARD_SIZE - 1));
        game.apply_action(&Move::new(2, 0));
        for x in (mid + 1)..last {
            game.apply_action(&Move::new(x, BOARD_SIZE - 1));
            if game.is_game_over() { return; }
            game.apply_action(&Move::new(x - mid + 3, 0));
            if game.is_game_over() { return; }
        }
        game.apply_action(&Move::new(last, BOARD_SIZE - 1));
        if game.is_game_over() {
            let positions = game.get_winning_positions();
            assert!(positions.contains(&(0, 0)) || positions.contains(&(last, last))
                || positions.len() == 5);
        }
    }

    #[test]
    fn legal_actions_nonempty_at_start() {
        let game = ConnectFive::new(Connect5Player::Player1);
        let actions = game.legal_actions();
        assert!(!actions.is_empty());
        let mid = BOARD_SIZE / 2;
        let expected = vec![
            Move::new(0, mid),
            Move::new(mid, 0),
            Move::new(mid, BOARD_SIZE - 1),
            Move::new(BOARD_SIZE - 1, mid),
        ];
        for m in &expected {
            assert!(actions.contains(m), "Expected {:?} to be legal", m);
        }
    }

    #[test]
    fn mcts_finds_immediate_win_when_available() {
        // P1 has four in a row at (5, 0..3), so (5, 4) wins at once.
        let mut g = ConnectFive::new(P1);
        for y in 0..4 {
            place(&mut g, P1, 5, y);
        }
        assert!(!g.is_game_over(), "fixture should not yet be terminal");
        g.current_player = P1;

        let mut tree = Tree::new(g, 0.81);
        let mv = tree
            .monte_carlo_tree_search(1_000)
            .expect("MCTS returned None on a winnable state");

        let mut after = g;
        after.apply_action(&mv);
        assert!(
            after.is_game_over(),
            "MCTS picked {mv:?}, which doesn't end the game; an immediate \
             win at (5, 4) was available"
        );
        assert_eq!(after.get_winner(), Some(P1));
    }

    #[test]
    fn mcts_full_game_terminates() {
        let mut game = ConnectFive::new(Connect5Player::Player1);
        let c = 0.81;
        let mut trees = [Tree::new(game, c), Tree::new(game, c)];
        let mut current = 0usize;
        let mut moves = 0;

        while !game.is_game_over() {
            // The game isn't over, so the tree must have a move.
            let mv = trees[current]
                .monte_carlo_tree_search(500)
                .expect("MCTS returned None on a non-terminal state");
            trees[0].update_root(vec![mv]);
            trees[1].update_root(vec![mv]);
            game.apply_action(&mv);
            current = 1 - current;
            moves += 1;
            assert!(moves < 200, "Game did not terminate within 200 moves");
        }

        assert!(game.is_game_over());
        // Draws are possible but rare; the point is that the game ends cleanly.
        if game.get_winner().is_some() {
            let positions = game.get_winning_positions();
            assert!(!positions.is_empty(), "Winning positions should be non-empty");
        }
    }
}
