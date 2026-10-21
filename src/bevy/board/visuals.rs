use bevy::prelude::*;
use bevy::window::WindowResized;

use crate::game::connect_five::{Connect5Player, ConnectFive, Move, Piece};
use crate::game::game_traits::Game;
use crate::BOARD_SIZE;

use super::components::{
    BoardEntity, BoardPosition, HoverPieceMarker, LegalDotMarker, PieceMarker, StoneAnim, TurnText,
};
use super::ui::{turn_label, ui_text_primary};

use crate::bevy::app::{
    FirstMove, GameBoard, GameOverState, GameSettings, Mode, NetMatch, PendingMove, SafeAreaInsets,
};
use crate::bevy::audio::{ui_text, UiFont};
use crate::bevy::computer::ComputerState;
use crate::bevy::net::{NetCommand, NetStatus, NetworkState};
use crate::bevy::theme::Theme;

// ---------------------------------------------------------------------------
// Board sizing. Colours live in `theme.rs`.
// ---------------------------------------------------------------------------

const PIECE_RADIUS_FRAC: f32 = 0.40;
const SHADOW_RADIUS_FRAC: f32 = 0.43;
const SHADOW_OFFSET_FRAC: f32 = 0.06;

/// Rule dots as a fraction of `cell_size`; smaller than a stone, so stones cover them.
const MARK_DOT_FRAC: f32 = 0.136;

const STONE_ANIM_SECS: f32 = 0.20;

// ---------------------------------------------------------------------------
// Shared assets — created once, reused across resize rebuilds
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct BoardAssets {
    pub unit_circle: Handle<Mesh>,
    pub legal_dot_mat: Handle<ColorMaterial>,
    pub hover_p1_mat: Handle<ColorMaterial>,
    pub hover_p2_mat: Handle<ColorMaterial>,
    pub piece_p1_mat: Handle<ColorMaterial>,
    pub piece_p2_mat: Handle<ColorMaterial>,
    /// Cool blue ring around the opponent's last move; it stands out on the warm board.
    pub highlight_ring_mat: Handle<ColorMaterial>,
    pub win_highlight_mat: Handle<ColorMaterial>,
    pub shadow_mat: Handle<ColorMaterial>,
    pub mark_ink_mat: Handle<ColorMaterial>,
    pub mark_pair_a_mat: Handle<ColorMaterial>,
    pub mark_pair_b_mat: Handle<ColorMaterial>,
}

pub fn setup_board_assets(
    mut commands: Commands,
    theme: Res<Theme>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let c = &theme.color;
    let unit_circle = meshes.add(Circle::new(1.0));
    let legal_dot_mat = materials.add(ColorMaterial::from_color(c.board_legal_dot));
    let mark_ink_mat = materials.add(ColorMaterial::from_color(c.special_mark_ink));

    // Warm up the Mesh2d pipelines during loading. iOS Metal compiles them on first use,
    // so marks and stones would otherwise pop in late. Opaque and translucent materials
    // are separate pipelines, so warm up both. The entities sit hidden behind the UI and board.
    commands.spawn((
        Mesh2d(unit_circle.clone()),
        MeshMaterial2d(legal_dot_mat.clone()),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));
    commands.spawn((
        Mesh2d(unit_circle.clone()),
        MeshMaterial2d(mark_ink_mat.clone()),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));

    commands.insert_resource(BoardAssets {
        unit_circle,
        legal_dot_mat,
        hover_p1_mat: materials.add(ColorMaterial::from_color(c.stone_p1_hover)),
        hover_p2_mat: materials.add(ColorMaterial::from_color(c.stone_p2_hover)),
        piece_p1_mat: materials.add(ColorMaterial::from_color(c.stone_p1)),
        piece_p2_mat: materials.add(ColorMaterial::from_color(c.stone_p2)),
        highlight_ring_mat: materials.add(ColorMaterial::from_color(c.accent)),
        win_highlight_mat: materials.add(ColorMaterial::from_color(c.board_win_highlight)),
        shadow_mat: materials.add(ColorMaterial::from_color(c.stone_shadow)),
        mark_ink_mat,
        mark_pair_a_mat: materials.add(ColorMaterial::from_color(c.mark_pair_a)),
        mark_pair_b_mat: materials.add(ColorMaterial::from_color(c.mark_pair_b)),
    });
}

pub(super) fn piece_material(
    assets: &BoardAssets,
    player: Connect5Player,
) -> Handle<ColorMaterial> {
    match player {
        Connect5Player::Player1 => assets.piece_p1_mat.clone(),
        Connect5Player::Player2 => assets.piece_p2_mat.clone(),
    }
}

/// The last stone placed, so the highlight ring survives a resize.
#[derive(Resource, Default)]
pub struct LatestMove(pub Option<(usize, usize)>);

/// The ring around the opponent's last move.
#[derive(Component)]
pub struct HighlightRing;

/// Whether `player` is us. In Local 2P every move counts as the opponent's,
/// so both players see the ring on the other's move.
pub(super) fn is_my_piece(
    player: Connect5Player,
    mode: Mode,
    net_match: Option<&NetMatch>,
) -> bool {
    match mode {
        Mode::SinglePlayer => matches!(player, Connect5Player::Player1),
        Mode::Local => false,
        Mode::Online => net_match.is_some_and(|nm| nm.local_player == player),
    }
}

// ---------------------------------------------------------------------------
// Board layout
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct BoardLayout {
    pub cell_size: f32,
    pub board_half: f32,
    /// Moves the board down to centre it below the top UI band.
    pub y_offset: f32,
}

impl BoardLayout {
    fn new(window_width: f32, window_height: f32, safe_area: SafeAreaInsets) -> Self {
        // ~3% margin at the sides. Only the top is reserved, for the turn text and pause button.
        let top_reserved = safe_area.top.max(20.0) + 56.0;
        let usable_h = (window_height - top_reserved).max(0.0);
        let min_dim = window_width.min(usable_h);
        let cell_size = min_dim * 0.97 / BOARD_SIZE as f32;
        let board_half = BOARD_SIZE as f32 * cell_size / 2.0;
        // Negative y moves the board down on screen.
        let y_offset = -top_reserved / 2.0;
        Self { cell_size, board_half, y_offset }
    }

    pub fn cell_center(&self, x: usize, y: usize) -> Vec2 {
        Vec2::new(
            -self.board_half + (x as f32 + 0.5) * self.cell_size,
            -self.board_half + (y as f32 + 0.5) * self.cell_size + self.y_offset,
        )
    }

    pub(super) fn world_to_cell(&self, world: Vec2) -> Option<(usize, usize)> {
        let fx = (world.x + self.board_half) / self.cell_size;
        let fy = (world.y - self.y_offset + self.board_half) / self.cell_size;
        if fx >= 0.0 && fy >= 0.0 {
            let cx = fx as usize;
            let cy = fy as usize;
            if cx < BOARD_SIZE && cy < BOARD_SIZE {
                return Some((cx, cy));
            }
        }
        None
    }
}

pub(super) fn screen_to_world(window: &Window, screen: Vec2) -> Vec2 {
    Vec2::new(
        screen.x - window.width() / 2.0,
        window.height() / 2.0 - screen.y,
    )
}

// ---------------------------------------------------------------------------
// Setup / cleanup / resize
// ---------------------------------------------------------------------------

pub fn setup_board(
    mut commands: Commands,
    windows: Query<&Window>,
    settings: Res<GameSettings>,
    assets: Res<BoardAssets>,
    net: Res<NetworkState>,
    font: Res<UiFont>,
    safe_area: Res<SafeAreaInsets>,
) {
    let Ok(window) = windows.single() else { return };
    let layout = BoardLayout::new(window.width(), window.height(), *safe_area);

    // Online, the host already decided who starts, so ignore `first_move`.
    let mut net_match: Option<NetMatch> = None;
    let first_player = if let NetStatus::Connected { is_host, first_player_is_host, .. } = &net.status {
        let local_player = if (*is_host) == (*first_player_is_host) {
            Connect5Player::Player1
        } else {
            Connect5Player::Player2
        };
        net_match = Some(NetMatch { local_player });
        Connect5Player::Player1
    } else {
        match settings.first_move {
            FirstMove::Player1 => Connect5Player::Player1,
            FirstMove::Player2 => Connect5Player::Player2,
            FirstMove::Random => {
                if rand::random::<bool>() {
                    Connect5Player::Player1
                } else {
                    Connect5Player::Player2
                }
            }
        }
    };
    let game = ConnectFive::new(first_player);

    spawn_board_visuals(
        &mut commands,
        &layout,
        &game,
        &settings,
        net_match.as_ref(),
        *safe_area,
        &assets,
        &font.0,
        None, // fresh game — no latest move yet
    );

    if let Some(nm) = net_match {
        commands.insert_resource(nm);
    }

    commands.insert_resource(GameBoard { game });
    commands.insert_resource(layout);
    commands.insert_resource(PendingMove::default());
    commands.insert_resource(GameOverState::default());
    commands.insert_resource(LatestMove::default());
    commands.insert_resource(ComputerState::new(game, &settings));
}

/// Spawns every board entity for the current game state.
fn spawn_board_visuals(
    commands: &mut Commands,
    layout: &BoardLayout,
    game: &ConnectFive,
    settings: &GameSettings,
    net_match: Option<&NetMatch>,
    safe_area: SafeAreaInsets,
    assets: &BoardAssets,
    font: &Handle<Font>,
    latest: Option<(usize, usize)>,
) {
    spawn_tiles(commands, layout);
    spawn_special_marks(commands, layout, assets);
    spawn_legal_dots(commands, layout, game, assets);
    spawn_hover_pieces(commands, layout, assets);
    spawn_existing_pieces(commands, layout, game, assets, latest, settings.mode, net_match);
    spawn_ui_overlays(commands, game.current_player, settings, net_match, safe_area, font);
}

fn spawn_tiles(commands: &mut Commands, layout: &BoardLayout) {
    let theme = Theme::default();
    let tile_size = Vec2::splat(layout.cell_size);
    for x in 0..BOARD_SIZE {
        for y in 0..BOARD_SIZE {
            let p = layout.cell_center(x, y);
            let color = if (x + y) % 2 == 0 { theme.color.board_light } else { theme.color.board_dark };
            commands.spawn((
                BoardEntity,
                Sprite { color, custom_size: Some(tile_size), ..default() },
                Transform::from_translation(p.extend(0.0)),
            ));
        }
    }
}

/// Rule dots: the centre, and the corners as two colour-matched diagonal pairs.
fn spawn_special_marks(commands: &mut Commands, layout: &BoardLayout, assets: &BoardAssets) {
    let mid = BOARD_SIZE / 2;
    let last = BOARD_SIZE - 1;
    let dot = layout.cell_size * MARK_DOT_FRAC;

    let mut spawn = |cx: usize, cy: usize, mat: &Handle<ColorMaterial>| {
        let p = layout.cell_center(cx, cy);
        commands.spawn((
            BoardEntity,
            Mesh2d(assets.unit_circle.clone()),
            MeshMaterial2d(mat.clone()),
            Transform::from_translation(p.extend(0.2)).with_scale(Vec3::splat(dot)),
        ));
    };

    // Centre.
    spawn(mid, mid, &assets.mark_ink_mat);

    // Diagonal pair A (NW + SE) plum.
    spawn(0, 0, &assets.mark_pair_a_mat);
    spawn(last, last, &assets.mark_pair_a_mat);

    // Diagonal pair B (SW + NE) sage.
    spawn(last, 0, &assets.mark_pair_b_mat);
    spawn(0, last, &assets.mark_pair_b_mat);
}

fn spawn_legal_dots(
    commands: &mut Commands,
    layout: &BoardLayout,
    game: &ConnectFive,
    assets: &BoardAssets,
) {
    // A bit smaller than the rule dots. Visibility is set at spawn, since a resize
    // right after entering can rebuild these before `update_legal_dots` runs.
    let dot_scale = layout.cell_size * 0.10;
    for x in 0..BOARD_SIZE {
        for y in 0..BOARD_SIZE {
            let p = layout.cell_center(x, y);
            let visibility = if game.is_legal(&Move::new(x, y)) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            commands.spawn((
                BoardEntity,
                LegalDotMarker,
                BoardPosition { x, y },
                Mesh2d(assets.unit_circle.clone()),
                MeshMaterial2d(assets.legal_dot_mat.clone()),
                Transform::from_translation(p.extend(0.3))
                    .with_scale(Vec3::splat(dot_scale)),
                visibility,
            ));
        }
    }
}

fn spawn_hover_pieces(commands: &mut Commands, layout: &BoardLayout, assets: &BoardAssets) {
    let radius = layout.cell_size * PIECE_RADIUS_FRAC;
    for (player, mat) in [
        (Connect5Player::Player1, &assets.hover_p1_mat),
        (Connect5Player::Player2, &assets.hover_p2_mat),
    ] {
        commands.spawn((
            BoardEntity,
            HoverPieceMarker(player),
            Mesh2d(assets.unit_circle.clone()),
            MeshMaterial2d(mat.clone()),
            Transform::from_translation(Vec3::new(0.0, 0.0, 0.5))
                .with_scale(Vec3::splat(radius)),
            Visibility::Hidden,
        ));
    }
}

fn spawn_existing_pieces(
    commands: &mut Commands,
    layout: &BoardLayout,
    game: &ConnectFive,
    assets: &BoardAssets,
    latest: Option<(usize, usize)>,
    mode: Mode,
    net_match: Option<&NetMatch>,
) {
    for x in 0..BOARD_SIZE {
        for y in 0..BOARD_SIZE {
            if let Piece::Occupied(player) = game.piece_at(x, y) {
                spawn_stone(commands, layout, assets, x, y, player, None);
                if latest == Some((x, y)) && !is_my_piece(player, mode, net_match) {
                    spawn_highlight_ring(commands, layout, assets, x, y, None);
                }
            }
        }
    }
}

/// A stone with a drop shadow. With `animate_at`, it scales in from 0.7× over ~120 ms.
pub(super) fn spawn_stone(
    commands: &mut Commands,
    layout: &BoardLayout,
    assets: &BoardAssets,
    cx: usize,
    cy: usize,
    player: Connect5Player,
    animate_at: Option<f32>,
) {
    let center = layout.cell_center(cx, cy);
    let body_r = layout.cell_size * PIECE_RADIUS_FRAC;
    let shadow_r = layout.cell_size * SHADOW_RADIUS_FRAC;
    let shadow_off = layout.cell_size * SHADOW_OFFSET_FRAC;

    let initial_factor = if animate_at.is_some() { 0.7 } else { 1.0 };

    // Shadow — slightly larger, offset down-right
    let shadow_entity = commands
        .spawn((
            BoardEntity,
            BoardPosition { x: cx, y: cy },
            Mesh2d(assets.unit_circle.clone()),
            MeshMaterial2d(assets.shadow_mat.clone()),
            Transform::from_translation(
                (center + Vec2::new(shadow_off, -shadow_off)).extend(0.95),
            )
            .with_scale(Vec3::splat(shadow_r * initial_factor)),
        ))
        .id();

    // Body
    let body_entity = commands
        .spawn((
            BoardEntity,
            PieceMarker,
            BoardPosition { x: cx, y: cy },
            Mesh2d(assets.unit_circle.clone()),
            MeshMaterial2d(piece_material(assets, player)),
            Transform::from_translation(center.extend(1.0))
                .with_scale(Vec3::splat(body_r * initial_factor)),
        ))
        .id();

    if let Some(start) = animate_at {
        commands.entity(shadow_entity).insert(StoneAnim {
            started: start,
            final_scale: shadow_r,
        });
        commands.entity(body_entity).insert(StoneAnim {
            started: start,
            final_scale: body_r,
        });
    }
}

/// Drawn behind the stone and ~15% larger, so only a thin rim shows.
pub(super) fn spawn_highlight_ring(
    commands: &mut Commands,
    layout: &BoardLayout,
    assets: &BoardAssets,
    cx: usize,
    cy: usize,
    animate_at: Option<f32>,
) {
    let center = layout.cell_center(cx, cy);
    let body_r = layout.cell_size * PIECE_RADIUS_FRAC;
    let ring_r = body_r * 1.10;
    let initial_factor = if animate_at.is_some() { 0.7 } else { 1.0 };

    let entity = commands
        .spawn((
            BoardEntity,
            BoardPosition { x: cx, y: cy },
            HighlightRing,
            Mesh2d(assets.unit_circle.clone()),
            MeshMaterial2d(assets.highlight_ring_mat.clone()),
            Transform::from_translation(center.extend(0.97))
                .with_scale(Vec3::splat(ring_r * initial_factor)),
        ))
        .id();

    if let Some(start) = animate_at {
        commands.entity(entity).insert(StoneAnim {
            started: start,
            final_scale: ring_r,
        });
    }
}

/// The place-in animation. Removes `StoneAnim` when done.
pub fn animate_stones(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &StoneAnim, &mut Transform)>,
) {
    let now = time.elapsed_secs();
    for (entity, anim, mut transform) in q.iter_mut() {
        let t = ((now - anim.started) / STONE_ANIM_SECS).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t) * (1.0 - t); // ease-out quadratic
        let factor = 0.7 + 0.3 * eased;
        transform.scale = Vec3::splat(anim.final_scale * factor);
        if t >= 1.0 {
            commands.entity(entity).remove::<StoneAnim>();
        }
    }
}

fn spawn_ui_overlays(
    commands: &mut Commands,
    current: Connect5Player,
    settings: &GameSettings,
    net_match: Option<&NetMatch>,
    safe_area: SafeAreaInsets,
    font: &Handle<Font>,
) {
    // Safe area plus a little room, with a minimum before iOS reports it.
    let top = safe_area.top.max(20.0) + 6.0;

    commands.spawn((
        BoardEntity,
        TurnText,
        Text::new(turn_label(current, settings, net_match)),
        ui_text(font, 20.0),
        TextColor(ui_text_primary()),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(top),
            left: Val::Px(16.0),
            ..default()
        },
    ));

    crate::bevy::pause::spawn_pause_toggle_button(commands, font, safe_area);
}

pub fn cleanup_board(
    mut commands: Commands,
    entities: Query<Entity, With<BoardEntity>>,
    net: Res<NetworkState>,
    rematching: Option<Res<crate::bevy::app::RematchInProgress>>,
) {
    for e in entities.iter() {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<GameBoard>();
    commands.remove_resource::<BoardLayout>();
    commands.remove_resource::<PendingMove>();
    commands.remove_resource::<GameOverState>();
    commands.remove_resource::<ComputerState>();
    commands.remove_resource::<LatestMove>();
    commands.remove_resource::<super::flow::DisconnectPending>();

    if rematching.is_some() {
        // Rematch: keep the connection so the board can be rebuilt on the same session.
        return;
    }
    commands.remove_resource::<NetMatch>();
    if matches!(net.status, NetStatus::Connected { .. }) {
        net.send(NetCommand::Disconnect);
    }
}

pub fn handle_resize(
    mut commands: Commands,
    mut resize_events: MessageReader<WindowResized>,
    entities: Query<Entity, With<BoardEntity>>,
    game_board: Res<GameBoard>,
    settings: Res<GameSettings>,
    net_match: Option<Res<NetMatch>>,
    safe_area: Res<SafeAreaInsets>,
    assets: Res<BoardAssets>,
    font: Res<UiFont>,
    latest_move: Res<LatestMove>,
    mut game_over: ResMut<GameOverState>,
    time: Res<Time>,
) {
    let Some(resized) = resize_events.read().last() else { return };

    for e in entities.iter() {
        commands.entity(e).despawn();
    }

    let layout = BoardLayout::new(resized.width, resized.height, *safe_area);
    spawn_board_visuals(
        &mut commands,
        &layout,
        &game_board.game,
        &settings,
        net_match.as_deref(),
        *safe_area,
        &assets,
        &font.0,
        latest_move.0,
    );
    commands.insert_resource(layout);

    // The resize removed the win line, so reveal it again.
    if game_over.is_over {
        game_over.overlay_shown = false;
        game_over.revealed_count = 0;
        game_over.revealed_at = Some(time.elapsed_secs());
    }
}
