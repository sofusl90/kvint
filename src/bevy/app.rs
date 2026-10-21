use std::time::Duration;

use bevy::prelude::*;
use bevy::winit::{UpdateMode, WinitSettings};

use crate::game::connect_five::{Connect5Player, Move};

// ---------------------------------------------------------------------------
// States
// ---------------------------------------------------------------------------

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Shown while assets load, then moves on to `Menu` (3 s timeout).
    #[default]
    Loading,
    Menu,
    HowToPlay,
    /// Display name and toggles, kept off the main menu.
    Settings,
    Lobby,
    InGame,
    /// Passed through for one frame so a rematch rebuilds the board but keeps the connection.
    Rematching,
}

// ---------------------------------------------------------------------------
// Settings / config
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    SinglePlayer,
    Local,
    Online,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Extreme,
    Custom(usize),
}

impl Difficulty {
    pub fn iterations(self) -> usize {
        match self {
            Self::Easy => 1_000,
            Self::Medium => 10_000,
            Self::Hard => 100_000,
            Self::Extreme => 500_000,
            Self::Custom(n) => n,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FirstMove {
    Player1,
    Player2,
    Random,
}

#[derive(Resource, Clone, Debug)]
pub struct GameSettings {
    pub mode: Mode,
    pub difficulty: Difficulty,
    pub first_move: FirstMove,
    /// The AI's first move on an empty board skips MCTS and doesn't take the centre.
    pub friendly_opening: bool,
    /// Shows power-user controls, like the MCTS iterations input.
    pub display_advanced: bool,
    /// Silences new sounds; sounds already playing finish.
    pub mute_audio: bool,
    /// The name other players see, e.g. `brave-otter`.
    pub display_name: String,
}

/// A random `adjective-animal` name, or `"player"` as a fallback.
pub fn default_display_name() -> String {
    petname::petname(2, "-").unwrap_or_else(|| "player".to_string())
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            mode: Mode::SinglePlayer,
            difficulty: Difficulty::Easy,
            first_move: FirstMove::Random,
            friendly_opening: false,
            display_advanced: false,
            mute_audio: false,
            display_name: default_display_name(),
        }
    }
}

impl GameSettings {
    /// AI iterations per seat, `[player1, player2]`. `None` is a human.
    pub fn computer_iterations(&self) -> [Option<usize>; 2] {
        match self.mode {
            Mode::SinglePlayer => [None, Some(self.difficulty.iterations())],
            Mode::Local | Mode::Online => [None, None],
        }
    }

    pub fn is_computer(&self, player: crate::game::connect_five::Connect5Player) -> bool {
        use crate::game::connect_five::Connect5Player::*;
        let idx = match player {
            Player1 => 0,
            Player2 => 1,
        };
        self.computer_iterations()[idx].is_some()
    }
}

// ---------------------------------------------------------------------------
// In-game resources
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct GameBoard {
    pub game: crate::game::connect_five::ConnectFive,
}

#[derive(Resource, Default)]
pub struct PendingMove(pub Option<Move>);

/// Only present online: which seat is ours.
#[derive(Resource, Clone, Debug)]
pub struct NetMatch {
    pub local_player: crate::game::connect_five::Connect5Player,
}

/// Both players must agree before a rematch.
#[derive(Resource, Default, Debug)]
pub struct RematchState {
    pub local: bool,
    pub remote: bool,
}

/// Set while a rematch is queued, so `cleanup_board` keeps the connection.
#[derive(Resource, Default)]
pub struct RematchInProgress;

#[derive(Resource, Default)]
pub struct GameOverState {
    pub is_over: bool,
    pub winning_positions: Vec<(usize, usize)>,
    pub winner: Option<Connect5Player>,
    pub overlay_shown: bool,
    /// Time the win was detected. Drives the staggered win-line reveal.
    pub revealed_at: Option<f32>,
    /// How many winning positions have been highlighted so far.
    pub revealed_count: usize,
    /// The player hid the game-over modal to look at the board.
    pub inspecting: bool,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Bundled assets: flat in the bundle on iOS, in `Contents/Resources` in a macOS .app.
/// Android and `cargo run` use Bevy's default.
fn find_asset_path() -> Option<std::path::PathBuf> {
    if cfg!(target_os = "android") {
        return None;
    }

    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    if cfg!(target_os = "ios") {
        // iOS: assets sit alongside the executable in the .app bundle.
        let ios_assets = exe_dir.join("assets");
        if ios_assets.exists() {
            return Some(ios_assets);
        }
        return None;
    }

    // macOS .app bundle layout
    let bundle_assets = exe_dir.join("..").join("Resources").join("assets");
    if let Ok(p) = bundle_assets.canonicalize() {
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Gives iOS the real screen size up front instead of 1280x720.
fn build_primary_window() -> Window {
    // `mut` is only used on iOS.
    #[allow(unused_mut)]
    let mut window = Window {
        title: "Connect 5".to_string(),
        ..default()
    };

    #[cfg(target_os = "ios")]
    {
        let mut w = 0.0_f32;
        let mut h = 0.0_f32;
        let mut s = 0.0_f32;
        unsafe { ios_screen_bounds(&mut w, &mut h, &mut s) };
        if w > 0.0 && h > 0.0 && s > 0.0 {
            let physical_w = (w * s) as u32;
            let physical_h = (h * s) as u32;
            window.resolution =
                bevy::window::WindowResolution::new(physical_w, physical_h)
                    .with_scale_factor_override(s);
        }
    }

    window
}

/// Logs panics so they show up in Console.app before the abort.
fn install_panic_logger() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("<non-string panic>");
        bevy::log::error!("PANIC at {}: {}", location, payload);
        default(info);
    }));
}

pub fn main() {
    install_panic_logger();

    let mut app = App::new();

    let primary_window = build_primary_window();

    if let Some(asset_path) = find_asset_path() {
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(primary_window),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_path.to_string_lossy().to_string(),
                    ..default()
                }),
        );
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(primary_window),
            ..default()
        }));
    }

    app.add_plugins(super::ui_scale::UiScalePlugin)
        .init_state::<AppState>()
        .insert_resource(GameSettings::default())
        // Same as `theme.color.bg_root`, so screen changes don't flash.
        .insert_resource(ClearColor(Color::srgb_u8(0x16, 0x15, 0x12)))
        // Redraw only on events; `Lobby` and `InGame` switch to continuous.
        .insert_resource(idle_winit())
        .init_resource::<super::pause::Paused>()
        .init_resource::<SafeAreaInsets>()
        .init_resource::<PendingDeepLink>()
        .init_resource::<super::fatal::FatalError>()
        .init_resource::<super::lobby::DeclineCooldowns>()
        .init_resource::<super::net::PeerNameCache>()
        .add_message::<super::net::RemoteMoveMessage>()
        .add_message::<super::net::RemoteRematchMessage>()
        .add_message::<super::net::JoinDeclinedMessage>()
        .add_message::<super::net::JoinSucceededMessage>()
        // Created up front so the first `OnEnter` can use them.
        .init_resource::<super::audio::UiFont>()
        .init_resource::<super::audio::TitleFont>()
        .init_resource::<super::theme::Theme>()
        // Shared by Menu and Settings.
        .init_resource::<super::menu::InputBuffer>()
        // The legal cell under the cursor, shared by `hover_piece` and `update_legal_dots`.
        .init_resource::<super::board::HoveredCell>()
        // Global startup
        .add_systems(
            Startup,
            (
                // Load persisted settings BEFORE anything reads GameSettings.
                super::settings_io::load_settings,
                setup_camera,
                super::audio::setup_sounds,
                super::board::setup_board_assets,
                super::net::setup_net,
            ),
        )
        // Drain network events every frame, regardless of state
        .add_systems(
            Update,
            (
                super::net::poll_net_events,
                super::net::sync_display_name,
                super::audio::click_on_any_button,
                super::audio::sync_audio_volume,
                super::fatal::enforce_menu,
                super::fatal::drive_toast,
                super::lobby::handle_deep_link,
                super::settings_io::save_settings_on_change,
            ),
        );

    #[cfg(target_os = "ios")]
    {
        // Pinning resource so the systems below run on the main thread.
        app.insert_non_send_resource(IosMainThread);
        app.add_systems(
            Update,
            (apply_ios_window_size, refresh_ios_safe_area, poll_ios_url),
        );
        // Register the URL handler once iOS has set up the AppDelegate.
        app.add_systems(Update, register_ios_url_handler_once);
    }

    app
        // ---- Power saving: redraw continuously only while something moves.
        .add_systems(
            OnEnter(AppState::Loading),
            |mut s: ResMut<WinitSettings>| *s = idle_winit(),
        )
        .add_systems(
            OnEnter(AppState::Menu),
            (
                |mut s: ResMut<WinitSettings>| *s = idle_winit(),
                super::net::stop_net,
            ),
        )
        .add_systems(
            OnEnter(AppState::HowToPlay),
            |mut s: ResMut<WinitSettings>| *s = idle_winit(),
        )
        .add_systems(
            OnEnter(AppState::Settings),
            |mut s: ResMut<WinitSettings>| *s = idle_winit(),
        )
        .add_systems(
            OnEnter(AppState::Lobby),
            (
                |mut s: ResMut<WinitSettings>| *s = idle_winit(),
                super::net::start_net,
            ),
        )
        .add_systems(
            OnEnter(AppState::InGame),
            |mut s: ResMut<WinitSettings>, mut p: ResMut<super::pause::Paused>| {
                // `tune_in_game_power` adjusts this every frame.
                *s = idle_winit();
                p.0 = false; // never start a fresh match paused
            },
        )
        // ---- Loading state (cold-start splash while fonts load) ----
        .add_systems(OnEnter(AppState::Loading), super::loading::setup_loading)
        .add_systems(OnExit(AppState::Loading), super::loading::cleanup_loading)
        .add_systems(
            Update,
            super::loading::check_loading_done.run_if(in_state(AppState::Loading)),
        )
        // ---- Menu + Settings states (share interaction systems) ----
        .add_systems(OnEnter(AppState::Menu), super::menu::setup_menu)
        .add_systems(OnExit(AppState::Menu), super::menu::cleanup_menu)
        .add_systems(OnEnter(AppState::Settings), super::menu::setup_settings)
        .add_systems(OnExit(AppState::Settings), super::menu::cleanup_settings)
        .add_systems(
            Update,
            (
                super::menu::menu_interaction
                    .before(super::menu::update_menu_visuals),
                super::menu::text_input_system
                    .before(super::menu::update_menu_visuals),
                super::menu::tune_menu_power
                    .after(super::menu::menu_interaction)
                    .after(super::menu::text_input_system),
                super::menu::update_menu_visuals,
                super::menu::update_toggle_pills,
            )
                .run_if(in_state(AppState::Menu).or(in_state(AppState::Settings))),
        )
        // ---- How to play ----
        .add_systems(OnEnter(AppState::HowToPlay), super::how_to_play::setup)
        .add_systems(OnExit(AppState::HowToPlay), super::how_to_play::cleanup)
        .add_systems(
            Update,
            (
                super::how_to_play::interaction,
                super::how_to_play::escape_back,
                super::lobby::scroll_screen,
            )
                .run_if(in_state(AppState::HowToPlay)),
        )
        // ---- Lobby state ----
        .add_systems(OnEnter(AppState::Lobby), super::lobby::setup_lobby)
        .add_systems(OnExit(AppState::Lobby), super::lobby::cleanup_lobby)
        .add_systems(
            Update,
            (
                super::lobby::auto_host_on_ready,
                super::lobby::lobby_interaction,
                super::lobby::scroll_screen,
                super::lobby::update_lobby_status,
                super::lobby::update_status_pill_display,
                super::lobby::update_back_button_label,
                super::lobby::update_lobby_buttons,
                super::lobby::populate_qr_image,
                super::lobby::refresh_nearby_list,
                super::lobby::update_incoming_request_overlay,
                super::lobby::record_decline_cooldowns,
                super::lobby::update_nearby_join_text,
            )
                .run_if(in_state(AppState::Lobby)),
        )
        // ---- InGame state ----
        .add_systems(OnEnter(AppState::InGame), super::board::setup_board)
        .add_systems(OnExit(AppState::InGame), super::board::cleanup_board)
        .add_systems(
            Update,
            (
                super::board::handle_resize,
                (
                    super::board::human_input,
                    super::board::apply_remote_moves,
                    super::computer::poll_computer,
                )
                    .before(super::board::process_move),
                super::board::process_move.after(super::board::handle_resize),
                (
                    super::computer::start_computer_turn,
                    super::board::check_game_over,
                )
                    .after(super::board::process_move),
                super::board::reveal_win_line.after(super::board::check_game_over),
                super::board::animate_stones.after(super::board::process_move),
                super::board::hover_piece.after(super::board::handle_resize),
                super::board::update_legal_dots
                    .after(super::board::handle_resize)
                    .after(super::board::hover_piece),
                super::board::update_turn_indicator
                    .after(super::board::process_move)
                    .after(super::computer::start_computer_turn),
                super::game_over::game_over_system.after(super::board::reveal_win_line),
                super::pause::toggle_pause,
                super::pause::update_pause_overlay.after(super::pause::toggle_pause),
                super::pause::handle_pause_buttons,
                super::board::detect_disconnect,
                super::game_over::handle_rematch_input,
                super::game_over::apply_remote_rematch,
                super::game_over::start_rematch_when_both_consent
                    .after(super::game_over::handle_rematch_input)
                    .after(super::game_over::apply_remote_rematch),
                // After every system that adds or removes `StoneAnim`, or animations can freeze.
                tune_in_game_power
                    .after(super::board::animate_stones)
                    .after(super::board::process_move)
                    .after(super::board::reveal_win_line)
                    .after(super::computer::poll_computer)
                    .after(super::board::check_game_over),
            )
                .run_if(in_state(AppState::InGame)),
        )
        // Pause-menu overlays, in a second tuple to stay under Bevy's 20-system limit.
        // `menu_interaction` can't be registered twice, hence `handle_overlay_settings_toggles`.
        .add_systems(
            Update,
            (
                super::pause::handle_open_overlay_buttons,
                super::pause::handle_close_overlay_buttons,
                super::pause::handle_overlay_settings_toggles,
                super::pause::handle_pause_secondary_hover,
                super::menu::update_toggle_pills,
                super::game_over::handle_inspect_toggle,
                super::game_over::update_inspect_visibility,
            )
                .run_if(in_state(AppState::InGame)),
        )
        // ---- Rematching: bounce back to InGame to rebuild the board, keeping the connection.
        .add_systems(OnEnter(AppState::Rematching), super::game_over::start_rematch_bounce)
        .run();

    // iroh's background tasks can keep the process alive for seconds after quitting.
    std::process::exit(0);
}

fn setup_camera(mut commands: Commands) {
    // iOS doesn't pick our camera as the default UI camera on its own.
    commands.spawn((Camera2d, IsDefaultUiCamera));
}

/// Picks the cheapest redraw mode each frame: continuous while animating,
/// 100 ms while waiting on the AI or the opponent, and idle otherwise.
fn tune_in_game_power(
    mut settings: ResMut<WinitSettings>,
    computer: Option<Res<super::computer::ComputerState>>,
    net_match: Option<Res<NetMatch>>,
    stone_anims: Query<(), With<super::board::StoneAnim>>,
    game_over: Option<Res<GameOverState>>,
) {
    let target = if !stone_anims.is_empty() {
        WinitSettings::game()
    } else if computer.is_some_and(|c| c.thinking)
        || net_match.is_some()
        // Only until the game-over overlay appears.
        || game_over.is_some_and(|g| g.is_over && !g.overlay_shown)
    {
        let m = UpdateMode::reactive(Duration::from_millis(100));
        WinitSettings { focused_mode: m, unfocused_mode: m }
    } else {
        idle_winit()
    };

    if settings.focused_mode != target.focused_mode
        || settings.unfocused_mode != target.unfocused_mode
    {
        *settings = target;
    }
}

/// Low-power default: redraw on input, otherwise once a second (for the lobby countdown).
/// All window events must wake it, since iOS taps arrive as `WindowEvent::Touch`.
pub(crate) fn idle_winit() -> WinitSettings {
    WinitSettings {
        focused_mode: UpdateMode::reactive(Duration::from_secs(1)),
        unfocused_mode: UpdateMode::reactive_low_power(Duration::from_secs(60)),
    }
}

// ---- iOS: window-size override
// Winit doesn't get the real window bounds on iOS (legacy AppDelegate launch),
// so we read UIScreen.main.bounds over FFI. The proper fix is the UIScene lifecycle.

#[cfg(target_os = "ios")]
extern "C" {
    fn ios_screen_bounds(w: *mut f32, h: *mut f32, scale: *mut f32);
    fn ios_safe_area_insets(
        top: *mut f32,
        bottom: *mut f32,
        left: *mut f32,
        right: *mut f32,
    );
    fn ios_register_url_handler();
    fn ios_poll_url(buf: *mut u8, buf_len: i32) -> i32;
}

/// Deep link from iOS, e.g. `kvint://join/<id>`.
#[derive(Resource, Default)]
pub struct PendingDeepLink(pub Option<String>);

#[cfg(target_os = "ios")]
fn register_ios_url_handler_once(
    mut accum: Local<f32>,
    time: Res<Time>,
    _: NonSend<IosMainThread>,
) {
    // Re-install every second in case iOS swaps the delegate class. It's idempotent.
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    unsafe { ios_register_url_handler() };
}

#[cfg(target_os = "ios")]
fn poll_ios_url(mut pending: ResMut<PendingDeepLink>) {
    let mut buf = [0u8; 256];
    let len = unsafe { ios_poll_url(buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        return;
    }
    if let Ok(s) = std::str::from_utf8(&buf[..len as usize]) {
        bevy::log::info!("iOS deep-link received: {}", s);
        pending.0 = Some(s.to_string());
    }
}

/// Safe-area insets in points. Zero off iOS.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct SafeAreaInsets {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl SafeAreaInsets {
    /// Top offset for pills near the top edge, with a minimum margin for devices without a notch.
    pub fn top_pill_offset(&self) -> f32 {
        self.top.max(20.0) + 6.0
    }
}

/// Keeps the iOS FFI systems on the main thread, which UIKit requires.
#[cfg(target_os = "ios")]
#[derive(Default)]
pub struct IosMainThread;

#[cfg(target_os = "ios")]
fn refresh_ios_safe_area(
    mut insets: ResMut<SafeAreaInsets>,
    mut have_value: Local<bool>,
    _: NonSend<IosMainThread>,
) {
    if *have_value {
        // Portrait-locked, so once we have a real value, stop calling into UIKit.
        return;
    }
    let mut t = 0.0_f32;
    let mut b = 0.0_f32;
    let mut l = 0.0_f32;
    let mut r = 0.0_f32;
    unsafe { ios_safe_area_insets(&mut t, &mut b, &mut l, &mut r) };
    // The top inset is the one that's reliably non-zero, so it's our "scene is up" check.
    if t > 0.0 {
        *insets = SafeAreaInsets { top: t, bottom: b, left: l, right: r };
        *have_value = true;
    }
}

#[cfg(target_os = "ios")]
fn apply_ios_window_size(
    mut windows: Query<&mut Window>,
    mut logged_once: Local<bool>,
    mut cached: Local<Option<(f32, f32, f32)>>,
    _: NonSend<IosMainThread>,
) {
    // The bounds don't change in a portrait-locked app, so cache them.
    let (w, h, s) = if let Some(v) = *cached {
        v
    } else {
        let mut w = 0.0_f32;
        let mut h = 0.0_f32;
        let mut s = 0.0_f32;
        unsafe { ios_screen_bounds(&mut w, &mut h, &mut s) };
        if w <= 0.0 || h <= 0.0 || s <= 0.0 {
            return; // iOS hasn't resolved bounds yet; try next frame
        }
        *cached = Some((w, h, s));
        (w, h, s)
    };
    let physical_w = (w * s) as u32;
    let physical_h = (h * s) as u32;
    for mut window in windows.iter_mut() {
        // bevy_winit overwrites the size every frame, so re-apply it.
        // Set the scale first, so the logical and physical sizes use it.
        if window.resolution.scale_factor_override() != Some(s) {
            window.resolution.set_scale_factor_override(Some(s));
        }
        if window.resolution.physical_width() != physical_w
            || window.resolution.physical_height() != physical_h
        {
            window.resolution.set_physical_resolution(physical_w, physical_h);
        }
    }
    if !*logged_once {
        info!("iOS: applying window size {}x{} scale={} every frame", w, h, s);
        *logged_once = true;
    }
}
