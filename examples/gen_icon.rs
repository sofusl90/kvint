//! Renders icon variants to `target/icon-variants/`:
//!     cargo run --example gen_icon --release

use image::{ImageBuffer, Rgba};

const SIZE: u32 = 1024;

// Palette
const FRAME: [u8; 4] = [97, 74, 50, 255];
const FRAME_INNER: [u8; 4] = [70, 53, 35, 255];
const TILE: [u8; 4] = [226, 216, 195, 255];
const TILE_EDGE: [u8; 4] = [205, 194, 173, 255];
const STONE_RED: [u8; 4] = [126, 42, 38, 255];
const STONE_RED_DARK: [u8; 4] = [82, 24, 22, 255];
const STONE_BLACK: [u8; 4] = [27, 26, 24, 255];
const STONE_BLACK_DARK: [u8; 4] = [10, 10, 9, 255];
const STONE_BLUE: [u8; 4] = [44, 78, 110, 255]; // matte ink-blue
const STONE_BLUE_DARK: [u8; 4] = [26, 50, 76, 255];
const INK: [u8; 4] = [42, 37, 32, 255];

#[derive(Clone, Copy)]
struct Stone {
    color: [u8; 4],
    edge: [u8; 4],
}
const RED: Stone = Stone { color: STONE_RED, edge: STONE_RED_DARK };
const BLACK: Stone = Stone { color: STONE_BLACK, edge: STONE_BLACK_DARK };
const BLUE: Stone = Stone { color: STONE_BLUE, edge: STONE_BLUE_DARK };

fn main() {
    render("target/icon-variants/icon_a_red.png", |x, y| variant_single(x, y, RED, true));
    render("target/icon-variants/icon_b_black.png", |x, y| variant_single(x, y, BLACK, true));
    render("target/icon-variants/icon_c_pair.png", variant_pair);
    render("target/icon-variants/icon_d_hoshi.png", variant_hoshi);
    render("target/icon-variants/icon_e_five.png", variant_five_in_row);
    render("target/icon-variants/icon_f_flat.png", |x, y| variant_single(x, y, RED, false));
    render("target/icon-variants/icon_g_letterk.png", variant_letter_k);
    println!("done — review variants under target/icon-variants/");
    println!("       to pick one: cp target/icon-variants/icon_<name>.png branding/icon.png");
    println!("       then run: ./scripts/gen_app_icons.sh");
}

fn render(path: &str, mut f: impl FnMut(f32, f32) -> [u8; 4]) {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(SIZE, SIZE, Rgba([0, 0, 0, 0]));
    let cx = SIZE as f32 / 2.0;
    let cy = SIZE as f32 / 2.0;
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let dx = x as f32 + 0.5 - cx;
        let dy = y as f32 + 0.5 - cy;
        *pixel = Rgba(f(dx, dy));
    }
    img.save(path).expect("save");
    println!("wrote {path}");
}

// ---------------------------------------------------------------------------
// Layout helpers — the frame + tile, shared by every variant
// ---------------------------------------------------------------------------

// The frame reaches the edges; iOS rounds the corners with its own mask.
const OUTER_HALF: f32 = SIZE as f32 / 2.0;
// A rounded frame for platforms that don't mask, like macOS.
const OUTER_CORNER: f32 = 224.0;
const FRAME_THICKNESS: f32 = 56.0;

fn inner_half() -> f32 {
    OUTER_HALF - FRAME_THICKNESS
}
fn inner_corner() -> f32 {
    (OUTER_CORNER - FRAME_THICKNESS).max(20.0)
}

/// Returns (outer_alpha, frame_color, inner_alpha, tile_base) for a pixel.
fn tile_at(dx: f32, dy: f32, vignette: bool) -> (f32, [u8; 4], f32, [u8; 4]) {
    let outer_alpha = rounded_square_alpha(dx, dy, OUTER_HALF, OUTER_CORNER);
    if outer_alpha <= 0.0 {
        return (0.0, [0; 4], 0.0, [0; 4]);
    }

    let mut frame_color = FRAME;
    let inner_alpha = rounded_square_alpha(dx, dy, inner_half(), inner_corner());
    if inner_alpha < 1.0 {
        let bevel = bevel_strength(dx, dy, inner_half(), inner_corner(), 8.0);
        frame_color = blend(frame_color, FRAME_INNER, bevel * 0.55);
    }

    let mut tile_base = TILE;
    if vignette && inner_alpha > 0.0 {
        let edge_d = edge_distance(dx, dy, inner_half(), inner_corner());
        let v = (1.0 - (edge_d / 80.0).clamp(0.0, 1.0)).powf(2.5);
        tile_base = blend(TILE, TILE_EDGE, v * 0.45);
    }

    (outer_alpha, frame_color, inner_alpha, tile_base)
}

/// Final composite: paint `inside` color into the tile, blend with frame, AA outer.
fn finish(outer_alpha: f32, frame: [u8; 4], inner_alpha: f32, inside: [u8; 4]) -> [u8; 4] {
    if outer_alpha <= 0.0 {
        return [0, 0, 0, 0];
    }
    let mut color = blend(frame, inside, inner_alpha);
    color[3] = (color[3] as f32 * outer_alpha) as u8;
    color
}

fn draw_stone(at: (f32, f32), radius: f32, stone: Stone, dx: f32, dy: f32, base: [u8; 4]) -> [u8; 4] {
    let (sx, sy) = at;
    let ddx = dx - sx;
    let ddy = dy - sy;
    let d = (ddx * ddx + ddy * ddy).sqrt();
    let alpha = circle_alpha(d, radius);
    if alpha <= 0.0 {
        return base;
    }
    let edge_factor = ((d / radius - 0.85) / 0.15).clamp(0.0, 1.0);
    let stone_color = blend(stone.color, stone.edge, edge_factor * 0.55);
    blend(base, stone_color, alpha)
}

// ---------------------------------------------------------------------------
// Variants
// ---------------------------------------------------------------------------

fn variant_single(dx: f32, dy: f32, stone: Stone, vignette: bool) -> [u8; 4] {
    let (oa, fc, ia, tb) = tile_at(dx, dy, vignette);
    if oa <= 0.0 {
        return [0; 4];
    }
    let inside = draw_stone((0.0, 0.0), inner_half() * 0.46, stone, dx, dy, tb);
    finish(oa, fc, ia, inside)
}

fn variant_pair(dx: f32, dy: f32) -> [u8; 4] {
    let (oa, fc, ia, tb) = tile_at(dx, dy, true);
    if oa <= 0.0 {
        return [0; 4];
    }
    // Two stones diagonally — red top-left, black bottom-right.
    let r = inner_half() * 0.32;
    let off = inner_half() * 0.30;
    let mut inside = tb;
    inside = draw_stone((-off, -off), r, RED, dx, dy, inside);
    inside = draw_stone((off, off), r, BLACK, dx, dy, inside);
    finish(oa, fc, ia, inside)
}

fn variant_hoshi(dx: f32, dy: f32) -> [u8; 4] {
    let (oa, fc, ia, tb) = tile_at(dx, dy, true);
    if oa <= 0.0 {
        return [0; 4];
    }
    // Center red stone, plus four ink "hoshi" dots toward the corners.
    let r_center = inner_half() * 0.40;
    let r_dot = inner_half() * 0.06;
    let off = inner_half() * 0.62;
    let dot = Stone { color: INK, edge: INK };
    let mut inside = draw_stone((0.0, 0.0), r_center, RED, dx, dy, tb);
    for (sx, sy) in [(-off, -off), (off, -off), (-off, off), (off, off)] {
        inside = draw_stone((sx, sy), r_dot, dot, dx, dy, inside);
    }
    finish(oa, fc, ia, inside)
}

/// "K" letterform built out of red stones laid on a small grid.
fn variant_letter_k(dx: f32, dy: f32) -> [u8; 4] {
    let (oa, fc, ia, tb) = tile_at(dx, dy, true);
    if oa <= 0.0 {
        return [0; 4];
    }
    // A K on a 5 × 3 grid:
    //   X . X
    //   X X .
    //   X . .
    //   X X .
    //   X . X
    let cells: &[(i32, i32)] = &[
        (0, 0), (1, 0), (2, 0), (3, 0), (4, 0), // spine
        (1, 1), (0, 2),                          // upper diagonal
        (3, 1), (4, 2),                          // lower diagonal
    ];

    let cols = 3.0;
    let rows = 5.0;
    // Cell pitch: fit the grid in the inner tile with comfortable margin.
    let cell = (inner_half() * 1.85 / rows).min(inner_half() * 1.85 / cols);
    let r = cell * 0.42;

    // Center the grid about the tile center.
    let origin_x = -((cols - 1.0) / 2.0) * cell;
    let origin_y = -((rows - 1.0) / 2.0) * cell;

    let mut inside = tb;
    for &(row, col) in cells {
        let x = origin_x + col as f32 * cell;
        let y = origin_y + row as f32 * cell;
        inside = draw_stone((x, y), r, RED, dx, dy, inside);
    }
    finish(oa, fc, ia, inside)
}

/// The board behind the stones: checker tiles, grid lines, corner rings and a gold centre.
fn paint_board_features(_base: [u8; 4], dx: f32, dy: f32, spacing: f32) -> [u8; 4] {
    // Checker tiles, centred on (0, 0) like the stones.
    let col = (dx / spacing).round() as i32;
    let row = (dy / spacing).round() as i32;
    let mut color = if (col + row).rem_euclid(2) == 0 { TILE } else { TILE_EDGE };

    // Faint grid lines between the tiles.
    let half = spacing * 0.5;
    let near_h = ((-3..=3).map(|i| (dy - (i as f32 * spacing + half)).abs())
        .fold(f32::INFINITY, f32::min)) <= 0.7;
    let near_v = ((-3..=3).map(|i| (dx - (i as f32 * spacing + half)).abs())
        .fold(f32::INFINITY, f32::min)) <= 0.7;
    if near_h || near_v {
        color = blend(color, INK, 0.20);
    }

    // Hollow corner rings at ±2 cells.
    let ring_outer = spacing * 0.36;
    let ring_inner = spacing * 0.22;
    for &(sx, sy) in &[(-2.0, -2.0), (2.0, -2.0), (-2.0, 2.0), (2.0, 2.0)] {
        let cx = sx * spacing;
        let cy = sy * spacing;
        let d = ((dx - cx).powi(2) + (dy - cy).powi(2)).sqrt();
        if d <= ring_outer && d >= ring_inner {
            let a_outer = (0.5 - (d - ring_outer)).clamp(0.0, 1.0);
            let a_inner = (0.5 - (ring_inner - d)).clamp(0.0, 1.0);
            color = blend(color, INK, a_outer.min(a_inner) * 0.6);
        }
    }

    // Gold center disc.
    let center_r = spacing * 0.20;
    let d_center = (dx * dx + dy * dy).sqrt();
    if d_center <= center_r + 0.5 {
        let a = (0.5 - (d_center - center_r)).clamp(0.0, 1.0);
        let gold = [201, 162, 74, 255];
        color = blend(color, gold, a * 0.9);
    }

    color
}

fn variant_five_in_row(dx: f32, dy: f32) -> [u8; 4] {
    let (oa, fc, ia, tb) = tile_at(dx, dy, true);
    if oa <= 0.0 {
        return [0; 4];
    }
    // Five red stones in a row, cut by a blue diagonal: five in a row against a block.
    let r = inner_half() * 0.13;
    let spacing = inner_half() * 0.32;

    // Board features behind the pieces (grid, corner rings, gold center).
    let mut inside = paint_board_features(tb, dx, dy, spacing);

    // Reds first (line of 5)
    for i in 0..5 {
        let x = (i as f32 - 2.0) * spacing;
        inside = draw_stone((x, 0.0), r, RED, dx, dy, inside);
    }
    // Each blue shares its x with a red, on the same grid.
    for &(mx, my) in &[(-2.0, -2.0), (-1.0, -1.0), (1.0, 1.0), (2.0, 2.0)] {
        let x = mx * spacing;
        let y = my * spacing;
        inside = draw_stone((x, y), r, BLUE, dx, dy, inside);
    }
    finish(oa, fc, ia, inside)
}

// ---------------------------------------------------------------------------
// AA primitives
// ---------------------------------------------------------------------------

fn rounded_square_alpha(dx: f32, dy: f32, half: f32, r: f32) -> f32 {
    let ax = dx.abs();
    let ay = dy.abs();
    let signed = if ax <= half - r || ay <= half - r {
        -((half - ax).min(half - ay))
    } else {
        let cx = ax - (half - r);
        let cy = ay - (half - r);
        (cx * cx + cy * cy).sqrt() - r
    };
    (0.5 - signed).clamp(0.0, 1.0)
}

fn circle_alpha(d: f32, r: f32) -> f32 {
    (0.5 - (d - r)).clamp(0.0, 1.0)
}

fn bevel_strength(dx: f32, dy: f32, half: f32, r: f32, width: f32) -> f32 {
    let ax = dx.abs();
    let ay = dy.abs();
    let d_inside = if ax <= half - r || ay <= half - r {
        ((half - ax).min(half - ay)).max(0.0)
    } else {
        let cx = ax - (half - r);
        let cy = ay - (half - r);
        (r - (cx * cx + cy * cy).sqrt()).max(0.0)
    };
    if d_inside <= 0.0 {
        return 0.0;
    }
    1.0 - (d_inside / width).clamp(0.0, 1.0)
}

fn edge_distance(dx: f32, dy: f32, half: f32, r: f32) -> f32 {
    let ax = dx.abs();
    let ay = dy.abs();
    if ax <= half - r && ay <= half - r {
        (half - ax).min(half - ay)
    } else {
        let cx = ax - (half - r);
        let cy = ay - (half - r);
        (r - (cx * cx + cy * cy).sqrt()).max(0.0)
    }
}

fn blend(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    let one = 1.0 - t;
    [
        (a[0] as f32 * one + b[0] as f32 * t) as u8,
        (a[1] as f32 * one + b[1] as f32 * t) as u8,
        (a[2] as f32 * one + b[2] as f32 * t) as u8,
        (a[3] as f32 * one + b[3] as f32 * t).clamp(0.0, 255.0) as u8,
    ]
}
