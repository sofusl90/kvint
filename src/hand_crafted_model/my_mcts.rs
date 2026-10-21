
use crate::game::game_traits::{Action, ActionMask, Game};
use bevy::ecs::component::Component;
use rand::rngs::SmallRng;
use rand::{rng, RngExt, SeedableRng};

/// Child indices visited in one iteration. Depth ≤ 121 and branching ~36, so `u8`s in a fixed buffer suffice.
const MAX_DEPTH: usize = 128;
struct VisitTrail {
    buf: [u8; MAX_DEPTH],
    len: usize,
}
impl VisitTrail {
    fn new() -> Self { Self { buf: [0; MAX_DEPTH], len: 0 } }
    #[inline(always)] fn clear(&mut self) { self.len = 0; }
    #[inline(always)]
    fn push(&mut self, idx: usize) {
        debug_assert!(self.len < MAX_DEPTH, "MCTS traversal exceeded MAX_DEPTH");
        debug_assert!(idx < 256, "child index exceeds u8 range");
        self.buf[self.len] = idx as u8;
        self.len += 1;
    }
    #[inline(always)] fn as_slice(&self) -> &[u8] { &self.buf[..self.len] }
}



#[derive(Debug, Clone)]
pub struct Node<G: Game> {
    children: Vec<Node<G>>,
    /// Actions not yet expanded into children.
    unexpanded: G::ActionMask,
    /// `f32` is precise enough here and cheaper for `sqrt`/`ln`.
    pub visits: f32,
    pub win_score: f32,
    action: Option<G::ActionType>,
    game: G,
}

impl<G: Game> Node<G> {
    fn new(mut game: G, action: Option<G::ActionType>) -> Self {
        let unexpanded = if !game.is_game_over() {
            game.legal_actions_mask()
        } else {
            G::ActionMask::default()
        };
        Self {
            children: Vec::new(),
            unexpanded,
            visits: 0.0,
            win_score: 0.0,
            action,
            game,
        }
    }

    /// `None` if there are no children. NaN UCT values compare as equal.
    fn best_uct(&self, c: f32) -> Option<usize> {
        let parent_ln = self.visits.ln();
        self.children
            .iter()
            .enumerate()
            .map(|(idx, child)| (idx, Self::uct_value(child, c, parent_ln)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx)
    }

    #[inline]
    fn uct_value(node: &Node<G>, c: f32, parent_ln: f32) -> f32 {
        if node.visits == 0.0 {
            f32::INFINITY
        } else {
            node.win_score / node.visits + c * (parent_ln / node.visits).sqrt()
        }
    }
}




#[derive(Component)]
pub(crate) struct Tree<G: Game> {
    root: Node<G>,
    c: f32,
}

impl<G: Game> Tree<G> {
    pub fn new(game: G, c: f32) -> Self {
        Self {
            root: Node::new(game, None),
            c,
        }
    }

    /// `None` if there is no move to pick, e.g. the game is already over.
    pub fn monte_carlo_tree_search(&mut self, num_iterations: usize) -> Option<G::ActionType> {
        let root_player: G::PlayerType = self.root.game.get_current_player();
        // MCTS doesn't need crypto-grade randomness, and `SmallRng` is cheaper.
        let mut rng = SmallRng::from_rng(&mut rng());
        let mut nodes_visited = VisitTrail::new();
        for _ in 0..num_iterations {
            nodes_visited.clear();
            let node = Self::traverse(&mut self.root, &mut nodes_visited, &mut rng, self.c);
            let simulation_result = Self::simulate(root_player, node.game, &mut rng);
            self.backpropagation(simulation_result, &nodes_visited, root_player);
        }
        let (best_move_index, _) = self.root.children
            .iter()
            .enumerate()
            .max_by(|a, b| match a.1.visits.partial_cmp(&b.1.visits) {
                Some(o) => o,
                None => {
                    bevy::log::warn!(
                        "MCTS pick: NaN visits comparison ({} vs {}); treating as Equal",
                        a.1.visits,
                        b.1.visits
                    );
                    std::cmp::Ordering::Equal
                }
            })?;
        self.root.children[best_move_index].action
    }

    fn backpropagation(
        &mut self,
        simulation_result: f32,
        nodes_visited: &VisitTrail,
        root_player: G::PlayerType,
    ) {
        let mut node = &mut self.root;
        node.visits += 1.0;
        for &node_idx in nodes_visited.as_slice() {
            node = &mut node.children[node_idx as usize];
            node.visits += 1.0;
            if node.game.get_current_player() == root_player {
                // if it was not the root that took the previous turn
                node.win_score += 1.0 - simulation_result;
            } else {
                node.win_score += simulation_result;
            }
        }
    }

    pub fn update_root(&mut self, moves: Vec<G::ActionType>) {
        for _move in moves {
            if let Some(child_index) = self
                .root
                .children
                .iter()
                .position(|node| node.action == Some(_move))
            {
                self.root = std::mem::replace(
                    &mut self.root.children[child_index],
                    Node::new(G::default(), None),
                );
            } else {
                let mut new_game = self.root.game;
                new_game.apply_action(&_move);
                self.root = Node::new(new_game, Some(_move));
            }
        }
    }

    fn traverse<'b>(
        mut node: &'b mut Node<G>,
        nodes_visited: &mut VisitTrail,
        rng: &mut SmallRng,
        c: f32,
    ) -> &'b mut Node<G> {
        while !node.children.is_empty() && node.unexpanded.is_empty() {
            let Some(idx) = node.best_uct(c) else { break };
            nodes_visited.push(idx);
            node = &mut node.children[idx];
        }
        if !node.unexpanded.is_empty() {
            let n = rng.random_range(0..node.unexpanded.count());
            let bit_idx = node.unexpanded.pop_nth(n);
            let random_move = G::ActionType::from_index(bit_idx);
            let mut new_game = node.game;
            new_game.apply_action(&random_move);
            node.children.push(Node::new(new_game, Some(random_move)));
            let child_index = node.children.len() - 1;
            nodes_visited.push(child_index);
            node = &mut node.children[child_index];
        }
        node
    }

    fn simulate(root_player: G::PlayerType, game: G, rng: &mut SmallRng) -> f32 {
        let mut game = game;
        while !game.is_game_over() {
            let mut mask = game.legal_actions_mask();
            // Not over but no legal moves: treat it as a draw.
            let count = mask.count();
            if count == 0 { break; }
            let n = rng.random_range(0..count);
            let action = G::ActionType::from_index(mask.pop_nth(n));
            game.apply_action(&action);
        }
        match game.get_winner() {
            Some(winner) if winner == root_player => 1.0,
            Some(_) => 0.0,
            // Draw: full board with no 5-in-a-row. Score it neutral (0.5).
            None => 0.5,
        }
    }

}

#[cfg(test)]
mod bench {
    use super::*;
    use crate::game::connect_five::{Connect5Player, ConnectFive, Move};
    use crate::game::game_traits::Game;
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    const C: f32 = 0.81;

    /// Reports order statistics; the median is the signal.
    fn report(label: &str, mut samples: Vec<Duration>) {
        samples.sort();
        let n = samples.len();
        let min = samples[0];
        let p50 = samples[n / 2];
        let p90 = samples[(n * 9) / 10];
        let max = samples[n - 1];
        let sum: Duration = samples.iter().sum();
        let mean = sum / n as u32;
        println!(
            "{label:<32}  reps={n:>3}  min={min:>10.2?}  p50={p50:>10.2?}  mean={mean:>10.2?}  p90={p90:>10.2?}  max={max:>10.2?}"
        );
    }

    /// A position that isn't over and has 30+ legal moves.
    fn mid_game_state() -> ConnectFive {
        let mut g = ConnectFive::new(Connect5Player::Player1);
        let seq: &[(usize, usize)] = &[
            (5, 5), (4, 5), (5, 4), (6, 5), (4, 6), (6, 4),
            (3, 5), (5, 6), (4, 4), (7, 5), (5, 3), (3, 6),
            (6, 6), (4, 7), (3, 4), (7, 4), (6, 3), (2, 5),
        ];
        for &(x, y) in seq {
            let mv = Move::new(x, y);
            if !g.is_game_over() && g.is_legal(&mv) {
                g.apply_action(&mv);
            }
        }
        assert!(!g.is_game_over(), "bench fixture should not be terminal");
        g
    }

    /// A full search from a fresh game, as `bevy/computer.rs` runs it.
    #[test]
    #[ignore = "benchmark; run: cargo test --release -- --ignored --nocapture mcts_search_bench"]
    fn mcts_search_bench() {
        const ITERATIONS: usize = 20_000;
        const WARMUP: usize = 3;
        const REPS: usize = 30;

        for _ in 0..WARMUP {
            let game = ConnectFive::new(Connect5Player::Player1);
            let mut tree = Tree::new(game, C);
            black_box(tree.monte_carlo_tree_search(ITERATIONS));
        }

        let mut samples = Vec::with_capacity(REPS);
        for _ in 0..REPS {
            let game = ConnectFive::new(Connect5Player::Player1);
            let mut tree = Tree::new(game, C);
            let t = Instant::now();
            black_box(tree.monte_carlo_tree_search(black_box(ITERATIONS)));
            samples.push(t.elapsed());
        }
        report(&format!("mcts_search ({ITERATIONS} iters)"), samples);
    }

    /// `legal_actions()` alone, without the MCTS overhead.
    #[test]
    #[ignore = "benchmark; run: cargo test --release -- --ignored --nocapture legal_actions_bench"]
    fn legal_actions_bench() {
        const STEPS: usize = 2_000_000;
        const WARMUP: usize = 2;
        const REPS: usize = 30;

        let g = mid_game_state();

        for _ in 0..WARMUP {
            for _ in 0..STEPS {
                black_box(black_box(&g).legal_actions());
            }
        }

        let mut samples = Vec::with_capacity(REPS);
        for _ in 0..REPS {
            let t = Instant::now();
            for _ in 0..STEPS {
                black_box(black_box(&g).legal_actions());
            }
            samples.push(t.elapsed());
        }
        report(&format!("legal_actions × {STEPS}"), samples);
    }

    /// Same as `legal_actions_bench`, using the bitmask that MCTS actually calls.
    #[test]
    #[ignore = "benchmark; run: cargo test --release -- --ignored --nocapture legal_mask_bench"]
    fn legal_mask_bench() {
        const STEPS: usize = 2_000_000;
        const WARMUP: usize = 2;
        const REPS: usize = 30;

        let g = mid_game_state();

        for _ in 0..WARMUP {
            for _ in 0..STEPS {
                black_box(black_box(&g).legal_actions_mask());
            }
        }

        let mut samples = Vec::with_capacity(REPS);
        for _ in 0..REPS {
            let t = Instant::now();
            for _ in 0..STEPS {
                black_box(black_box(&g).legal_actions_mask());
            }
            samples.push(t.elapsed());
        }
        report(&format!("legal_actions_mask × {STEPS}"), samples);
    }

    /// `is_game_over` on a fresh copy of a mid-game position.
    #[test]
    #[ignore = "benchmark; run: cargo test --release -- --ignored --nocapture is_game_over_bench"]
    fn is_game_over_bench() {
        const STEPS: usize = 2_000_000;
        const WARMUP: usize = 2;
        const REPS: usize = 30;

        let template = mid_game_state();

        for _ in 0..WARMUP {
            for _ in 0..STEPS {
                // `black_box` stops LLVM from hoisting this out of the loop.
                let mut g = *black_box(&template);
                black_box(g.is_game_over());
            }
        }

        let mut samples = Vec::with_capacity(REPS);
        for _ in 0..REPS {
            let t = Instant::now();
            for _ in 0..STEPS {
                // `black_box` stops LLVM from hoisting this out of the loop.
                let mut g = *black_box(&template);
                black_box(g.is_game_over());
            }
            samples.push(t.elapsed());
        }
        report(&format!("is_game_over × {STEPS}"), samples);
    }
}
