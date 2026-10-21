use bevy::prelude::*;

use crate::game::connect_five::Connect5Player;

/// Every entity of the board scene, so cleanup and resize can despawn them all.
#[derive(Component)]
pub struct BoardEntity;

#[derive(Component, Clone, Copy, PartialEq)]
pub struct BoardPosition {
    pub x: usize,
    pub y: usize,
}

#[derive(Component)]
pub struct PieceMarker;

#[derive(Component)]
pub struct HoverPieceMarker(pub Connect5Player);

#[derive(Component)]
pub struct LegalDotMarker;


/// A stone being animated in. The power system keeps redrawing while one exists.
#[derive(Component)]
pub struct StoneAnim {
    pub started: f32,
    pub final_scale: f32,
}

#[derive(Component)]
pub struct TurnText;

#[derive(Component)]
pub struct WinRing;

/// Marks the toast that says "Opponent left" before we snap back to the menu.
#[derive(Component)]
pub struct DisconnectToast;

/// The legal cell under the cursor. `update_legal_dots` skips its dot.
#[derive(Resource, Default)]
pub struct HoveredCell(pub Option<(usize, usize)>);
