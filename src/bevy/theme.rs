//! Design tokens for every screen: colours, radii, spacing, shadows and sizes.
//! Warm dark surfaces like lichess, green for primary actions, rounded corners.

use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Theme>();
    }
}

// ---------------------------------------------------------------------------
// The Theme resource
// ---------------------------------------------------------------------------

#[derive(Resource, Clone, Debug, Default)]
pub struct Theme {
    pub color: ColorTokens,
    pub radius: RadiusTokens,
    pub space: SpacingTokens,
    pub size: SizeTokens,
    pub shadow: ShadowTokens,
    pub font_size: FontSizeTokens,
}

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------
// Hex values are sRGB.

#[derive(Clone, Copy, Debug)]
pub struct ColorTokens {
    // -- Surfaces (warm dark, lichess-style) --
    pub bg_root: Color,         // page background
    pub surface: Color,         // cards, panels, secondary buttons
    pub surface_raised: Color,  // hover / elevated state
    pub surface_overlay: Color, // modal scrim (alpha-blended)
    pub input_bg: Color,        // text fields

    // -- Text --
    pub text_primary: Color,    // body text
    pub text_muted: Color,      // labels, captions
    pub text_on_primary: Color, // text over green button
    pub text_on_inverse: Color, // dark text on parchment-fill buttons
    pub text_warning: Color,    // amber warnings
    pub text_error: Color,      // soft red errors

    // -- Intent (action coloring, lichess palette) --
    pub primary: Color,           // Start / Confirm / Host / primary CTA
    pub primary_hover: Color,
    pub primary_disabled: Color,
    pub destructive: Color,       // Quit / Leave
    pub destructive_hover: Color,
    pub accent: Color,            // links, info, "Join" labels

    // -- Selected segment, filled so it stands out.
    pub seg_active: Color,
    pub seg_active_text: Color,

    // -- Borders --
    pub border: Color,
    pub border_subtle: Color,

    // -- Brand accent --
    pub title_gold: Color,  // status emphasis (lobby "Connecting…") + the gold rule under the title
    /// Screen titles. Muted, so the Start button and corner colours stand out.
    pub headline: Color,

    // -- Board (parchment, kept close to the existing values) --
    pub board_light: Color,
    pub board_dark: Color,
    pub board_grid: Color,
    pub board_legal_dot: Color,
    pub board_win_highlight: Color,
    pub special_mark_ink: Color,
    pub special_mark_gold: Color,
    /// Corner pair colours. Both ends of a diagonal match, which shows the rule without reading it.
    pub mark_pair_a: Color,
    pub mark_pair_b: Color,

    // -- Stones --
    pub stone_p1: Color,
    pub stone_p2: Color,
    pub stone_p1_hover: Color,
    pub stone_p2_hover: Color,
    pub stone_shadow: Color,

    // -- Toast / fatal error --
    pub toast_bg: Color,
    pub toast_border: Color,
    pub toast_text: Color,
}

impl Default for ColorTokens {
    fn default() -> Self {
        // Channel values are 0..255 — easier to read against hex pickers.
        fn rgb(r: u8, g: u8, b: u8) -> Color { Color::srgb_u8(r, g, b) }
        fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color { Color::srgba_u8(r, g, b, a) }

        Self {
            // Surfaces
            bg_root: rgb(0x16, 0x15, 0x12),
            surface: rgb(0x26, 0x24, 0x21),
            surface_raised: rgb(0x31, 0x2e, 0x2b),
            surface_overlay: rgba(0x16, 0x15, 0x12, 0xd9),
            input_bg: rgb(0x1f, 0x1d, 0x1a),

            // Text
            text_primary: rgb(0xe5, 0xe1, 0xd6),
            text_muted: rgb(0x9a, 0x94, 0x8a),
            text_on_primary: Color::WHITE,
            text_on_inverse: rgb(0x16, 0x15, 0x12),
            text_warning: rgb(0xd4, 0xa0, 0x17),
            text_error: rgb(0xd4, 0x57, 0x50),

            // Intent
            primary: rgb(0x62, 0x99, 0x24),
            primary_hover: rgb(0x7a, 0xba, 0x2c),
            primary_disabled: rgb(0x3c, 0x5a, 0x1c),
            destructive: rgb(0xc0, 0x39, 0x2b),
            destructive_hover: rgb(0xd4, 0x4a, 0x3a),
            accent: rgb(0x38, 0x93, 0xe8),

            // Segmented active
            seg_active: rgb(0xe5, 0xe1, 0xd6),
            seg_active_text: rgb(0x16, 0x15, 0x12),

            // Borders
            border: rgb(0x4a, 0x44, 0x3e),
            border_subtle: rgb(0x32, 0x2e, 0x2a),

            // Brand
            title_gold: rgb(0xc9, 0xa1, 0x4a),
            headline: rgb(0xef, 0xe6, 0xd2),

            // Board: lichess's wood palette.
            board_light: rgb(0xf0, 0xd9, 0xb5),
            board_dark: rgb(0xb5, 0x88, 0x63),
            board_grid: rgb(0x2a, 0x1f, 0x14),
            board_legal_dot: rgba(0x2a, 0x1f, 0x14, 0x38),
            board_win_highlight: rgba(0xff, 0xc8, 0x4a, 0xb0),
            special_mark_ink: rgb(0x2a, 0x1f, 0x14),
            special_mark_gold: rgb(0x9e, 0x7c, 0x3b),
            mark_pair_a: rgb(0x6e, 0x4f, 0x7a), // dusty plum
            mark_pair_b: rgb(0x6a, 0x8a, 0x5a), // muted sage

            // Stones
            stone_p1: rgb(0x1b, 0x1a, 0x18),
            stone_p2: rgb(0x7e, 0x2a, 0x26),
            stone_p1_hover: rgba(0x1b, 0x1a, 0x18, 0x4c),
            stone_p2_hover: rgba(0x7e, 0x2a, 0x26, 0x4c),
            stone_shadow: rgba(0, 0, 0, 0x2e),

            // Toast / fatal
            toast_bg: rgba(0x61, 0x1c, 0x1c, 0xeb),
            toast_border: rgb(0x9e, 0x42, 0x3a),
            toast_text: rgb(0xf6, 0xe3, 0xdb),
        }
    }
}

// ---------------------------------------------------------------------------
// Radii
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct RadiusTokens {
    pub xs: f32,  // chips, badges
    pub sm: f32,  // small inputs
    pub md: f32,  // default buttons
    pub lg: f32,  // cards, panels
    pub xl: f32,  // modals, sheets
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self { xs: 6.0, sm: 10.0, md: 14.0, lg: 18.0, xl: 24.0 }
    }
}

impl RadiusTokens {
    pub fn xs(&self) -> BorderRadius { BorderRadius::all(Val::Px(self.xs)) }
    pub fn sm(&self) -> BorderRadius { BorderRadius::all(Val::Px(self.sm)) }
    pub fn md(&self) -> BorderRadius { BorderRadius::all(Val::Px(self.md)) }
    pub fn lg(&self) -> BorderRadius { BorderRadius::all(Val::Px(self.lg)) }
    pub fn xl(&self) -> BorderRadius { BorderRadius::all(Val::Px(self.xl)) }
    pub fn pill(&self) -> BorderRadius { BorderRadius::MAX }
}

// ---------------------------------------------------------------------------
// Spacing
// ---------------------------------------------------------------------------
//
// 4 / 8 / 12 / 16 / 24 / 32 / 48 / 64 — the only values used in the UI.

#[derive(Clone, Copy, Debug)]
pub struct SpacingTokens {
    pub xs: f32,   // 4
    pub sm: f32,   // 8
    pub md: f32,   // 12
    pub lg: f32,   // 16
    pub xl: f32,   // 24
    pub xxl: f32,  // 32
    pub xxxl: f32, // 48
    pub huge: f32, // 64
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self {
            xs: 4.0, sm: 8.0, md: 12.0, lg: 16.0,
            xl: 24.0, xxl: 32.0, xxxl: 48.0, huge: 64.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Sizes (button heights, column widths)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct SizeTokens {
    pub button_h_compact: f32, // 36 — secondary actions, list rows
    pub button_h: f32,         // 48 — default touch-target height
    pub button_h_tall: f32,    // 56 — primary CTAs (Start)
    pub column_max: f32,       // 520 — menu / lobby column max width
    pub column_max_wide: f32,  // 600 — how-to-play, longer reading content
    pub border_thin: f32,      // 1.0 — hairline border
}

impl Default for SizeTokens {
    fn default() -> Self {
        Self {
            button_h_compact: 36.0,
            button_h: 48.0,
            button_h_tall: 56.0,
            column_max: 520.0,
            column_max_wide: 600.0,
            border_thin: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Shadows
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
pub struct ShadowTokens;

impl ShadowTokens {
    /// Resting cards / panels.
    pub fn elev_1(&self) -> BoxShadow {
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.18),
            Val::Px(0.0),
            Val::Px(2.0),
            Val::Px(0.0),
            Val::Px(6.0),
        )
    }

    /// Buttons / hovered cards.
    pub fn elev_2(&self) -> BoxShadow {
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.25),
            Val::Px(0.0),
            Val::Px(4.0),
            Val::Px(0.0),
            Val::Px(12.0),
        )
    }

    /// Modals, sheets, toasts.
    pub fn elev_3(&self) -> BoxShadow {
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.35),
            Val::Px(0.0),
            Val::Px(10.0),
            Val::Px(0.0),
            Val::Px(28.0),
        )
    }
}

// ---------------------------------------------------------------------------
// Font sizes
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct FontSizeTokens {
    pub caption: f32,  // 13
    pub small: f32,    // 14
    pub body: f32,     // 16
    pub heading: f32,  // 18
    pub subtitle: f32, // 22
    pub h2: f32,       // 32
    pub h1: f32,       // 48
    pub display: f32,  // 56 — wordmark
}

impl Default for FontSizeTokens {
    fn default() -> Self {
        Self {
            caption: 13.0,
            small: 14.0,
            body: 16.0,
            heading: 18.0,
            subtitle: 22.0,
            h2: 32.0,
            h1: 48.0,
            display: 56.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Button intent — used by the helpers below to pick colors
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonIntent {
    Primary,     // green CTA — Start, Host, Resume
    Secondary,   // dark surface — Cancel, Back, segmented inactive
    Destructive, // red — Quit, Leave
    Ghost,       // transparent — text-only / tertiary
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Hovered,
    Pressed,
    Active,   // selected (segmented control)
    Disabled,
}

impl Theme {
    /// Button background for an intent and state, so hover and press look the same everywhere.
    pub fn button_bg(&self, intent: ButtonIntent, state: ButtonState) -> Color {
        use ButtonIntent::*;
        use ButtonState::*;
        match (intent, state) {
            (_, Disabled) => match intent {
                Primary => self.color.primary_disabled,
                Destructive => self.color.surface,
                Secondary | Ghost => self.color.surface,
            },
            (Primary, Hovered | Pressed) => self.color.primary_hover,
            (Primary, _) => self.color.primary,
            (Destructive, Hovered | Pressed) => self.color.destructive_hover,
            (Destructive, _) => self.color.destructive,
            (Secondary, Active) => self.color.seg_active,
            (Secondary, Hovered | Pressed) => self.color.surface_raised,
            (Secondary, _) => self.color.surface,
            (Ghost, Hovered | Pressed) => Color::srgba(1.0, 1.0, 1.0, 0.04),
            (Ghost, _) => Color::NONE,
        }
    }

    /// Text color matching a button intent + state.
    pub fn button_text(&self, intent: ButtonIntent, state: ButtonState) -> Color {
        use ButtonIntent::*;
        use ButtonState::*;
        match (intent, state) {
            (_, Disabled) => self.color.text_muted,
            (Primary, _) => self.color.text_on_primary,
            (Destructive, _) => self.color.text_on_primary,
            (Secondary, Active) => self.color.seg_active_text,
            (Secondary | Ghost, _) => self.color.text_primary,
        }
    }

    /// Standard button node. `flex_grow` is for segmented rows.
    pub fn button_node(&self, height: f32, width: Val, flex_grow: f32) -> Node {
        Node {
            width,
            height: Val::Px(height),
            flex_grow,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: self.radius.md(),
            ..default()
        }
    }

    /// Pill-shaped button Node (toggles, "Ready" indicators).
    pub fn pill_node(&self, height: f32, width: Val, flex_grow: f32) -> Node {
        Node {
            width,
            height: Val::Px(height),
            flex_grow,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(self.space.xl)),
            border_radius: self.radius.pill(),
            ..default()
        }
    }

    /// Card / panel Node with inner padding and a rounded corner radius.
    pub fn card_node(&self) -> Node {
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(self.space.lg)),
            row_gap: Val::Px(self.space.sm),
            border_radius: self.radius.lg(),
            ..default()
        }
    }

    /// Modal / sheet Node — used by pause, game-over, fatal overlays.
    pub fn modal_node(&self) -> Node {
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(self.space.xl)),
            row_gap: Val::Px(self.space.lg),
            align_items: AlignItems::Center,
            border_radius: self.radius.xl(),
            ..default()
        }
    }
}
