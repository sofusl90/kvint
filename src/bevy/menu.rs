use bevy::input::keyboard::KeyboardInput;
#[cfg(not(target_os = "ios"))]
use bevy::input::ButtonState as KeyState;
use bevy::prelude::*;
use bevy::winit::WinitSettings;

use super::app::{default_display_name, AppState, Difficulty, FirstMove, GameSettings, Mode};
use super::audio::{ui_text, TitleFont, UiFont};
use super::theme::{ButtonIntent, ButtonState, Theme};

// iOS UITextField bridge, implemented in `gen/apple/Sources/kvint/main.mm`.
#[cfg(target_os = "ios")]
extern "C" {
    fn ios_text_input_show(initial: *const std::ffi::c_char);
    fn ios_text_input_hide();
    fn ios_text_input_poll(buf: *mut u8, buf_len: i32) -> i32;
    fn ios_text_input_dismissed() -> i32;
}

// ---------------------------------------------------------------------------
// Components & resources
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct SettingsRoot;

#[derive(Component, Clone)]
pub struct MenuButton(pub MenuAction);

#[derive(Clone, PartialEq)]
pub enum MenuAction {
    SetMode(Mode),
    SetDifficulty(Difficulty),
    SetFirstMove(FirstMove),
    /// The AI's first move goes to the bottom edge when it starts.
    ToggleFriendlyOpening,
    /// Settings-screen toggle: surface power-user controls in the main menu.
    ToggleDisplayAdvanced,
    /// Settings-screen toggle: mute all SFX.
    ToggleMuteAudio,
    FocusIterations,
    FocusName,
    RandomizeName,
    StartGame,
    HowToPlay,
    OpenSettings,
    BackFromSettings,
}

impl MenuAction {
    /// Which button intent (color) this action gets.
    fn intent(&self) -> ButtonIntent {
        match self {
            MenuAction::StartGame => ButtonIntent::Primary,
            MenuAction::HowToPlay
            | MenuAction::OpenSettings
            | MenuAction::BackFromSettings => ButtonIntent::Ghost,
            _ => ButtonIntent::Secondary,
        }
    }
}

/// The field being edited, for the iOS keyboard and desktop key events.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InputField {
    Iterations,
    Name,
}

// Desktop only; on iOS, UITextField does its own validation.
#[cfg(not(target_os = "ios"))]
impl InputField {
    /// Input beyond this many bytes is dropped.
    fn max_len(self) -> usize {
        match self {
            Self::Iterations => 8,
            Self::Name => 24,
        }
    }

    /// Whether the field accepts a given character.
    fn allows(self, ch: char) -> bool {
        match self {
            Self::Iterations => ch.is_ascii_digit(),
            Self::Name => ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ',
        }
    }
}

// Display-control markers — toggled in `update_menu_visuals`.
#[derive(Component)]
pub struct DifficultySection;
#[derive(Component)]
pub struct IterationsSection;
#[derive(Component)]
pub struct SlowAiWarning;
#[derive(Component)]
pub struct IterationsValueText;
#[derive(Component)]
pub struct NameValueText;
#[derive(Component)]
pub struct FirstMoveSection;

/// ON/OFF pill of a settings row, coloured by its action's state.
#[derive(Component)]
pub struct TogglePill(pub MenuAction);

/// "ON"/"OFF" text inside the pill.
#[derive(Component)]
pub struct TogglePillText;

/// Label of a segmented button; flips colour when the button is active.
#[derive(Component)]
pub struct SegLabel(pub MenuAction);

/// Text inside the Start button — flips wording based on selected mode.
#[derive(Component)]
pub struct StartButtonLabel;

#[derive(Resource, Default)]
pub struct InputBuffer {
    pub text: String,
    pub focused: Option<InputField>,
}

// ---------------------------------------------------------------------------
// Setup / cleanup
// ---------------------------------------------------------------------------

pub fn setup_menu(
    mut commands: Commands,
    _settings: Res<GameSettings>,
    mut input: ResMut<InputBuffer>,
    font: Res<UiFont>,
    title_font: Res<TitleFont>,
    theme: Res<Theme>,
) {
    // Reset the input; the resources themselves outlive Menu and Settings.
    input.text.clear();
    input.focused = None;

    let f = font.0.clone();
    let tf = title_font.0.clone();
    commands
        .spawn((
            MenuRoot,
            full_screen_column(&theme),
            BackgroundColor(theme.color.bg_root),
        ))
        .with_children(|root| {
            root.spawn(menu_inner_column(&theme))
                .with_children(|col| {
                    // ---- Top: title and mode row.
                    spawn_title(col, &tf, &theme);
                    spawn_mode_row(col, &f, &theme);

                    col.spawn(Node {
                        height: Val::Px(theme.space.lg),
                        ..default()
                    });
                    spawn_hairline(col, &theme, theme.color.text_muted, 0.30, 0.0);

                    // ---- Per-mode settings, shown or hidden by `update_menu_visuals`.
                    spawn_difficulty_section(col, &f, &theme);
                    spawn_iterations_section(col, &f, &theme);
                    spawn_first_move_section(col, &f, &theme);

                    // ---- Start sits low, in thumb reach, with the footer below it.
                    col.spawn(Node {
                        flex_grow: 1.0,
                        ..default()
                    });
                    spawn_start_button(col, &f, &theme);
                    col.spawn(Node {
                        height: Val::Px(theme.space.md),
                        ..default()
                    });
                    spawn_footer(col, &f, &theme);
                });
        });
}

fn full_screen_column(theme: &Theme) -> Node {
    let _ = theme;
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    }
}

fn menu_inner_column(theme: &Theme) -> Node {
    // Sections set their own top margins, the same in Menu and Settings.
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        max_width: Val::Px(theme.size.column_max),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Stretch,
        padding: UiRect::all(Val::Px(theme.space.xl)),
        ..default()
    }
}

/// Thin divider line.
fn spawn_hairline(
    parent: &mut ChildSpawnerCommands,
    theme: &Theme,
    color: Color,
    alpha: f32,
    margin: f32,
) {
    let _ = theme;
    let mut c = color.to_srgba();
    c.alpha = alpha;
    parent.spawn((
        Node {
            height: Val::Px(1.0),
            width: Val::Percent(100.0),
            margin: UiRect::vertical(Val::Px(margin)),
            ..default()
        },
        BackgroundColor(c.into()),
    ));
}

pub fn cleanup_menu(mut commands: Commands, roots: Query<Entity, With<MenuRoot>>) {
    for e in roots.iter() {
        commands.entity(e).despawn();
    }
    // Hide the keyboard so it doesn't stay up on the next screen.
    hide_native_keyboard();
}

// ---------------------------------------------------------------------------
// Settings screen, reached from the menu footer.
// ---------------------------------------------------------------------------

pub fn setup_settings(
    mut commands: Commands,
    mut input: ResMut<InputBuffer>,
    font: Res<UiFont>,
    title_font: Res<TitleFont>,
    theme: Res<Theme>,
) {
    input.text.clear();
    input.focused = None;

    let f = font.0.clone();
    let tf = title_font.0.clone();
    commands
        .spawn((
            SettingsRoot,
            full_screen_column(&theme),
            BackgroundColor(theme.color.bg_root),
        ))
        .with_children(|root| {
            root.spawn(menu_inner_column(&theme))
                .with_children(|col| {
                    // Same title style as the main menu.
                    col.spawn((
                        Text::new("Settings"),
                        ui_text(&tf, theme.font_size.h1),
                        TextColor(theme.color.headline),
                        Node {
                            margin: UiRect::vertical(Val::Px(theme.space.lg)),
                            align_self: AlignSelf::Center,
                            ..default()
                        },
                    ));

                    spawn_hairline(col, &theme, theme.color.title_gold, 0.18, theme.space.md);

                    // Name field row.
                    col.spawn(Node {
                        margin: UiRect::top(Val::Px(theme.space.lg)),
                        ..section_node(&theme)
                    })
                        .with_children(|sec| {
                            sub_label(sec, "Name", &f, &theme);
                            sec.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(theme.space.sm + 2.0),
                                ..default()
                            })
                            .with_children(|row| {
                                row.spawn((
                                    MenuButton(MenuAction::FocusName),
                                    Button,
                                    Node {
                                        flex_grow: 1.0,
                                        height: Val::Px(theme.size.button_h_compact),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border_radius: theme.radius.sm(),
                                        ..default()
                                    },
                                    BackgroundColor(theme.color.input_bg),
                                ))
                                .with_children(|input| {
                                    input.spawn((
                                        NameValueText,
                                        Text::new(""),
                                        ui_text(&f, theme.font_size.body),
                                        TextColor(theme.color.text_primary),
                                    ));
                                });
                                row.spawn((
                                    MenuButton(MenuAction::RandomizeName),
                                    Button,
                                    Node {
                                        height: Val::Px(theme.size.button_h_compact),
                                        padding: UiRect::horizontal(Val::Px(theme.space.md)),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border_radius: theme.radius.sm(),
                                        ..default()
                                    },
                                    BackgroundColor(theme.button_bg(
                                        ButtonIntent::Secondary,
                                        ButtonState::Normal,
                                    )),
                                ))
                                .with_children(|btn| {
                                    btn.spawn((
                                        Text::new("Randomize"),
                                        ui_text(&f, theme.font_size.small),
                                        TextColor(theme.button_text(
                                            ButtonIntent::Secondary,
                                            ButtonState::Normal,
                                        )),
                                    ));
                                });
                            });
                        });

                    spawn_settings_toggles(col, &f, &theme);

                    // Spacer + back link pinned to bottom.
                    col.spawn(Node { flex_grow: 1.0, ..default() });
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_self: AlignSelf::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_text_button(row, "← Back", MenuAction::BackFromSettings, &f, &theme);
                    });
                });
        });
}

/// The settings toggle rows, shared by Settings and the pause overlay.
pub fn spawn_settings_toggles(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    theme: &Theme,
) {
    spawn_toggle_row(
        parent,
        "Show advanced options",
        MenuAction::ToggleDisplayAdvanced,
        font,
        theme,
    );
    spawn_toggle_row(
        parent,
        "AI opens at bottom",
        MenuAction::ToggleFriendlyOpening,
        font,
        theme,
    );
    spawn_toggle_row(parent, "Mute sounds", MenuAction::ToggleMuteAudio, font, theme);
}

/// Label on the left, ON/OFF pill on the right; the whole row is tappable.
pub(crate) fn spawn_toggle_row(
    parent: &mut ChildSpawnerCommands,
    label_text: &str,
    action: MenuAction,
    font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            MenuButton(action.clone()),
            Button,
            Node {
                margin: UiRect::top(Val::Px(theme.space.md)),
                height: Val::Px(theme.size.button_h_compact),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(theme.space.md)),
                border_radius: theme.radius.sm(),
                ..default()
            },
            BackgroundColor(theme.color.input_bg),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label_text),
                ui_text(font, theme.font_size.body),
                TextColor(theme.color.text_primary),
            ));
            btn.spawn((
                TogglePill(action),
                Node {
                    padding: UiRect::axes(Val::Px(theme.space.sm), Val::Px(theme.space.xs)),
                    border_radius: theme.radius.sm(),
                    ..default()
                },
                BackgroundColor(theme.color.surface_raised),
            ))
            .with_children(|pill| {
                pill.spawn((
                    TogglePillText,
                    Text::new("OFF"),
                    ui_text(font, theme.font_size.small),
                    TextColor(theme.color.text_muted),
                ));
            });
        });
}

pub fn cleanup_settings(mut commands: Commands, roots: Query<Entity, With<SettingsRoot>>) {
    for e in roots.iter() {
        commands.entity(e).despawn();
    }
    hide_native_keyboard();
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

fn spawn_title(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, theme: &Theme) {
    parent.spawn((
        Text::new("KVINT"),
        ui_text(font, theme.font_size.display),
        TextColor(theme.color.headline),
        Node {
            margin: UiRect::vertical(Val::Px(theme.space.lg)),
            align_self: AlignSelf::Center,
            ..default()
        },
    ));
}

fn spawn_mode_row(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, theme: &Theme) {
    parent
        .spawn((
            Node {
                margin: UiRect::top(Val::Px(theme.space.lg)),
                ..section_node(theme)
            },
        ))
        .with_children(|sec| {
            seg_row(sec, theme, |row| {
                seg_button(row, "Single Player", MenuAction::SetMode(Mode::SinglePlayer), font, theme);
                seg_button(row, "Local 2P", MenuAction::SetMode(Mode::Local), font, theme);
            });
            seg_row(sec, theme, |row| {
                seg_button(row, "Online", MenuAction::SetMode(Mode::Online), font, theme);
            });
        });
}

fn spawn_difficulty_section(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            DifficultySection,
            Node {
                margin: UiRect::top(Val::Px(theme.space.lg)),
                ..section_node(theme)
            },
        ))
        .with_children(|sec| {
            sub_label(sec, "Difficulty", font, theme);
            seg_row(sec, theme, |row| {
                seg_button(row, "Novice", MenuAction::SetDifficulty(Difficulty::Easy), font, theme);
                seg_button(row, "Apprentice", MenuAction::SetDifficulty(Difficulty::Medium), font, theme);
                seg_button(row, "Adept", MenuAction::SetDifficulty(Difficulty::Hard), font, theme);
                seg_button(row, "Master", MenuAction::SetDifficulty(Difficulty::Extreme), font, theme);
            });
            sec.spawn((
                SlowAiWarning,
                Text::new("May be slow."),
                ui_text(font, theme.font_size.small),
                TextColor(theme.color.text_warning),
                Node {
                    margin: UiRect::top(Val::Px(theme.space.xs)),
                    align_self: AlignSelf::Center,
                    ..default()
                },
            ));
        });
}

/// MCTS iterations input, only in Single Player with advanced options on.
fn spawn_iterations_section(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            IterationsSection,
            Node {
                margin: UiRect::top(Val::Px(theme.space.lg)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(theme.space.sm + 2.0),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                Text::new("Iterations"),
                ui_text(font, theme.font_size.small),
                TextColor(theme.color.text_muted),
            ));
            row.spawn((
                MenuButton(MenuAction::FocusIterations),
                Button,
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(theme.size.button_h_compact),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: theme.radius.sm(),
                    ..default()
                },
                BackgroundColor(theme.color.input_bg),
            ))
            .with_children(|input| {
                input.spawn((
                    IterationsValueText,
                    Text::new("100,000"),
                    ui_text(font, theme.font_size.body),
                    TextColor(theme.color.text_primary),
                ));
            });
        });
}

fn spawn_first_move_section(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            FirstMoveSection,
            Node {
                margin: UiRect::top(Val::Px(theme.space.lg)),
                ..section_node(theme)
            },
        ))
        .with_children(|sec| {
            sub_label(sec, "First move", font, theme);
            seg_row(sec, theme, |row| {
                seg_button(row, "Player 1", MenuAction::SetFirstMove(FirstMove::Player1), font, theme);
                seg_button(row, "Player 2", MenuAction::SetFirstMove(FirstMove::Player2), font, theme);
                seg_button(row, "Random", MenuAction::SetFirstMove(FirstMove::Random), font, theme);
            });
        });
}

fn spawn_start_button(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, theme: &Theme) {
    parent
        .spawn((
            MenuButton(MenuAction::StartGame),
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(theme.size.button_h_tall + 4.0),
                margin: UiRect::top(Val::Px(theme.space.sm)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: theme.radius.md(),
                ..default()
            },
            BackgroundColor(theme.button_bg(ButtonIntent::Primary, ButtonState::Normal)),
            theme.shadow.elev_2(),
        ))
        .with_children(|btn| {
            btn.spawn((
                StartButtonLabel,
                Text::new("START GAME"),
                ui_text(font, theme.font_size.subtitle),
                TextColor(theme.color.text_on_primary),
            ));
        });
}

/// Text-only buttons separated by dots.
fn spawn_footer(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, theme: &Theme) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(theme.space.sm),
            align_self: AlignSelf::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            spawn_text_button(row, "How to play", MenuAction::HowToPlay, font, theme);
            spawn_middot(row, font, theme);
            spawn_text_button(row, "Settings", MenuAction::OpenSettings, font, theme);
        });
}

/// Text button with a faint outline, so it reads as tappable.
fn spawn_text_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: MenuAction,
    font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            MenuButton(action),
            Button,
            Node {
                padding: UiRect::axes(Val::Px(theme.space.md), Val::Px(theme.space.sm)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(theme.size.border_thin)),
                border_radius: theme.radius.sm(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(theme.color.border_subtle),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                ui_text(font, theme.font_size.small),
                TextColor(theme.color.text_primary),
            ));
        });
}

fn spawn_middot(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, theme: &Theme) {
    let mut muted = theme.color.text_muted.to_srgba();
    muted.alpha = 0.4;
    parent.spawn((
        Text::new("·"),
        ui_text(font, theme.font_size.small),
        TextColor(muted.into()),
    ));
}

// ---------------------------------------------------------------------------
// Building blocks
// ---------------------------------------------------------------------------

fn section_node(theme: &Theme) -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(theme.space.sm),
        ..default()
    }
}

/// Smaller, muted label for the secondary settings.
fn sub_label(parent: &mut ChildSpawnerCommands, text: &str, font: &Handle<Font>, theme: &Theme) {
    parent.spawn((
        Text::new(text),
        ui_text(font, theme.font_size.small),
        TextColor(theme.color.text_muted),
    ));
}

fn seg_row(
    parent: &mut ChildSpawnerCommands,
    theme: &Theme,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(theme.space.sm),
            ..default()
        })
        .with_children(build);
}

fn seg_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: MenuAction,
    font: &Handle<Font>,
    theme: &Theme,
) {
    let bg = theme.button_bg(ButtonIntent::Secondary, ButtonState::Normal);
    let text_color = theme.button_text(ButtonIntent::Secondary, ButtonState::Normal);
    let action_for_label = action.clone();
    parent
        .spawn((
            MenuButton(action),
            Button,
            Node {
                flex_grow: 1.0,
                height: Val::Px(theme.size.button_h),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: theme.radius.md(),
                ..default()
            },
            BackgroundColor(bg),
        ))
        .with_children(|btn| {
            btn.spawn((
                SegLabel(action_for_label),
                Text::new(label),
                ui_text(font, theme.font_size.body),
                TextColor(text_color),
            ));
        });
}

// ---------------------------------------------------------------------------
// Interaction
// ---------------------------------------------------------------------------

pub fn menu_interaction(
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    mut settings: ResMut<GameSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    mut input: ResMut<InputBuffer>,
) {
    for (interaction, btn) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        // Any other button press commits the text being edited, except refocusing it or Randomize.
        let preserve_focus = matches!(
            (&btn.0, input.focused),
            (MenuAction::FocusIterations, Some(InputField::Iterations))
                | (MenuAction::FocusName, Some(InputField::Name))
                | (MenuAction::RandomizeName, _)
        );
        if !preserve_focus {
            commit_input(&mut input, &mut settings);
        }

        match &btn.0 {
            MenuAction::SetMode(mode) => {
                settings.mode = *mode;
            }
            MenuAction::SetDifficulty(d) => {
                settings.difficulty = *d;
            }
            MenuAction::SetFirstMove(fm) => {
                settings.first_move = *fm;
            }
            MenuAction::ToggleFriendlyOpening => {
                settings.friendly_opening = !settings.friendly_opening;
            }
            MenuAction::ToggleDisplayAdvanced => {
                settings.display_advanced = !settings.display_advanced;
            }
            MenuAction::ToggleMuteAudio => {
                settings.mute_audio = !settings.mute_audio;
            }
            MenuAction::FocusIterations => {
                focus_field(
                    &mut input,
                    InputField::Iterations,
                    settings.difficulty.iterations().to_string(),
                );
            }
            MenuAction::FocusName => {
                focus_field(&mut input, InputField::Name, settings.display_name.clone());
            }
            MenuAction::RandomizeName => {
                settings.display_name = default_display_name();
                // Discard any partial edit and dismiss the keyboard if open.
                if matches!(input.focused, Some(InputField::Name)) {
                    input.focused = None;
                    input.text.clear();
                    hide_native_keyboard();
                }
            }
            MenuAction::StartGame => {
                if settings.mode == Mode::Online {
                    next_state.set(AppState::Lobby);
                } else {
                    next_state.set(AppState::InGame);
                }
            }
            MenuAction::HowToPlay => {
                next_state.set(AppState::HowToPlay);
            }
            MenuAction::OpenSettings => {
                next_state.set(AppState::Settings);
            }
            MenuAction::BackFromSettings => {
                next_state.set(AppState::Menu);
            }
        }
    }
}

/// Sets the text and focus, and opens the iOS keyboard.
fn focus_field(input: &mut InputBuffer, field: InputField, initial: String) {
    input.text = initial;
    input.focused = Some(field);
    show_native_keyboard(&input.text);
}

/// Opens the on-screen keyboard on iOS; no-op elsewhere.
fn show_native_keyboard(_initial: &str) {
    #[cfg(target_os = "ios")]
    {
        if let Ok(cs) = std::ffi::CString::new(_initial.to_string()) {
            unsafe { ios_text_input_show(cs.as_ptr()) };
        }
    }
}

fn hide_native_keyboard() {
    #[cfg(target_os = "ios")]
    unsafe {
        ios_text_input_hide();
    }
}

pub fn text_input_system(
    mut events: MessageReader<KeyboardInput>,
    mut settings: ResMut<GameSettings>,
    mut input: ResMut<InputBuffer>,
) {
    let Some(field) = input.focused else {
        // Drain messages so they don't pile up while nothing is focused.
        events.read().for_each(|_| {});
        return;
    };

    // On iOS, poll the hidden UITextField (see main.mm) and commit on Done.
    #[cfg(target_os = "ios")]
    {
        let _ = field; // referenced only by the desktop branch below
        let mut buf = [0u8; 256];
        let len = unsafe { ios_text_input_poll(buf.as_mut_ptr(), buf.len() as i32) };
        let new_bytes = if len > 0 { &buf[..len as usize] } else { &[][..] };
        // Compare bytes first; this runs every frame.
        if new_bytes != input.text.as_bytes() {
            input.text = std::str::from_utf8(new_bytes)
                .unwrap_or("")
                .to_string();
        }
        if unsafe { ios_text_input_dismissed() } != 0 {
            commit_input(&mut input, &mut settings);
        }
        // Drain hardware key events so they don't queue while focus is open.
        events.read().for_each(|_| {});
    }

    #[cfg(not(target_os = "ios"))]
    for ev in events.read() {
        if ev.state != KeyState::Pressed {
            continue;
        }
        match ev.key_code {
            KeyCode::Backspace => {
                input.text.pop();
                continue;
            }
            KeyCode::Enter | KeyCode::Escape => {
                commit_input(&mut input, &mut settings);
                return;
            }
            _ => {}
        }
        let Some(text) = &ev.text else { continue };
        for ch in text.chars() {
            if input.text.len() >= field.max_len() {
                break;
            }
            if field.allows(ch) {
                input.text.push(ch);
            }
        }
    }
}

/// Redraw continuously while a field is focused: iOS keystrokes don't wake winit,
/// so the field would look frozen.
pub fn tune_menu_power(input: Res<InputBuffer>, mut settings: ResMut<WinitSettings>) {
    let target = if input.focused.is_some() {
        WinitSettings::game()
    } else {
        super::app::idle_winit()
    };
    if settings.focused_mode != target.focused_mode
        || settings.unfocused_mode != target.unfocused_mode
    {
        *settings = target;
    }
}

// ---------------------------------------------------------------------------
// Visual update — runs every frame
// ---------------------------------------------------------------------------

pub fn update_menu_visuals(
    settings: Res<GameSettings>,
    input: Res<InputBuffer>,
    theme: Res<Theme>,
    mut buttons: Query<(&Interaction, &MenuButton, &mut BackgroundColor)>,
    mut difficulty_section: Query<
        &mut Node,
        (
            With<DifficultySection>,
            Without<IterationsSection>,
            Without<FirstMoveSection>,
        ),
    >,
    mut iterations_section: Query<
        &mut Node,
        (
            With<IterationsSection>,
            Without<DifficultySection>,
            Without<FirstMoveSection>,
        ),
    >,
    mut first_move_section: Query<
        &mut Node,
        (
            With<FirstMoveSection>,
            Without<DifficultySection>,
            Without<IterationsSection>,
            Without<SlowAiWarning>,
        ),
    >,
    mut slow_ai_warning: Query<
        &mut Node,
        (
            With<SlowAiWarning>,
            Without<DifficultySection>,
            Without<IterationsSection>,
            Without<FirstMoveSection>,
        ),
    >,
    mut iter_text: Query<
        &mut Text,
        (
            With<IterationsValueText>,
            Without<SegLabel>,
            Without<StartButtonLabel>,
            Without<NameValueText>,
        ),
    >,
    mut name_text: Query<
        &mut Text,
        (
            With<NameValueText>,
            Without<IterationsValueText>,
            Without<SegLabel>,
            Without<StartButtonLabel>,
        ),
    >,
    mut seg_labels: Query<
        (&SegLabel, &mut Text, &mut TextColor),
        (
            Without<IterationsValueText>,
            Without<StartButtonLabel>,
            Without<NameValueText>,
        ),
    >,
    mut start_label: Query<
        &mut Text,
        (
            With<StartButtonLabel>,
            Without<IterationsValueText>,
            Without<SegLabel>,
            Without<NameValueText>,
        ),
    >,
) {
    // Toggle rows are skipped: only their pill changes colour (`update_toggle_pills`).
    for (interaction, btn, mut bg) in buttons.iter_mut() {
        if matches!(
            btn.0,
            MenuAction::ToggleFriendlyOpening
                | MenuAction::ToggleDisplayAdvanced
                | MenuAction::ToggleMuteAudio
        ) {
            continue;
        }
        let intent = btn.0.intent();
        let active = is_active(&btn.0, &settings, &input);
        let disabled = is_disabled(&btn.0);

        let state = if disabled {
            ButtonState::Disabled
        } else if active {
            ButtonState::Active
        } else {
            match interaction {
                Interaction::Hovered | Interaction::Pressed => ButtonState::Hovered,
                _ => ButtonState::Normal,
            }
        };
        *bg = BackgroundColor(theme.button_bg(intent, state));
    }

    // Section visibility
    let show_difficulty = settings.mode == Mode::SinglePlayer;
    let show_iterations = show_difficulty && settings.display_advanced;
    // Online, the host picks who starts at random.
    let show_first_move = settings.mode != Mode::Online;

    // Warn when the search is slow on mobile: Master, or a custom budget over 100k.
    let show_slow_warning = show_difficulty
        && match settings.difficulty {
            Difficulty::Extreme => true,
            Difficulty::Custom(n) => n > 100_000,
            _ => false,
        };

    set_display(difficulty_section.iter_mut(), show_difficulty);
    set_display(first_move_section.iter_mut(), show_first_move);
    set_display(iterations_section.iter_mut(), show_iterations);
    set_display(slow_ai_warning.iter_mut(), show_slow_warning);

    // Labels flip colour on filled buttons; first-move labels get friendlier names in Single Player.
    for (label, mut text, mut color) in seg_labels.iter_mut() {
        let active = is_active(&label.0, &settings, &input);
        let disabled = is_disabled(&label.0);
        let state = if disabled {
            ButtonState::Disabled
        } else if active {
            ButtonState::Active
        } else {
            ButtonState::Normal
        };
        *color = TextColor(theme.button_text(ButtonIntent::Secondary, state));

        if let MenuAction::SetFirstMove(fm) = &label.0 {
            let new_label = match (settings.mode, fm) {
                (Mode::SinglePlayer, FirstMove::Player1) => "You",
                (Mode::SinglePlayer, FirstMove::Player2) => "Computer",
                (_, FirstMove::Player1) => "Player 1",
                (_, FirstMove::Player2) => "Player 2",
                (_, FirstMove::Random) => "Random",
            };
            set_text_if_changed(&mut text, new_label);
        }
    }

    // Start button — wording reflects what the next click does
    for mut t in start_label.iter_mut() {
        let label = match settings.mode {
            Mode::Online => "FIND OPPONENT",
            _ => "START GAME",
        };
        set_text_if_changed(&mut t, label);
    }

    // Iterations input display
    for mut t in iter_text.iter_mut() {
        let display = if input.focused == Some(InputField::Iterations) {
            with_caret(&input.text)
        } else {
            format_number(settings.difficulty.iterations())
        };
        set_text_if_changed(&mut t, &display);
    }

    // Name input display
    for mut t in name_text.iter_mut() {
        let display = if input.focused == Some(InputField::Name) {
            with_caret(&input.text)
        } else {
            settings.display_name.as_str().to_string()
        };
        set_text_if_changed(&mut t, &display);
    }
}

/// Only writes on change, so Bevy doesn't re-layout the text every frame.
fn set_text_if_changed(text: &mut Mut<Text>, new: &str) {
    if text.0 != new {
        **text = Text::new(new);
    }
}

/// Updates the ON/OFF pills. Their queries are empty outside Settings.
pub fn update_toggle_pills(
    settings: Res<GameSettings>,
    input: Res<InputBuffer>,
    theme: Res<Theme>,
    mut pills: Query<(&TogglePill, &mut BackgroundColor)>,
    mut texts: Query<(&ChildOf, &mut Text, &mut TextColor), With<TogglePillText>>,
    pill_lookup: Query<&TogglePill>,
) {
    for (pill, mut bg) in pills.iter_mut() {
        let on = is_active(&pill.0, &settings, &input);
        *bg = BackgroundColor(if on {
            theme.color.seg_active
        } else {
            theme.color.surface_raised
        });
    }
    for (parent, mut text, mut color) in texts.iter_mut() {
        let Ok(pill) = pill_lookup.get(parent.parent()) else { continue };
        let on = is_active(&pill.0, &settings, &input);
        set_text_if_changed(&mut text, if on { "ON" } else { "OFF" });
        *color = TextColor(if on {
            theme.color.seg_active_text
        } else {
            theme.color.text_muted
        });
    }
}

/// The text with a caret at the end. Keystrokes always append; there's no cursor movement.
fn with_caret(text: &str) -> String {
    if text.is_empty() {
        "|".to_string()
    } else {
        format!("{text}|")
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn set_display<'a>(iter: impl Iterator<Item = Mut<'a, Node>>, visible: bool) {
    for mut node in iter {
        node.display = if visible { Display::Flex } else { Display::None };
    }
}

fn is_active(action: &MenuAction, settings: &GameSettings, _input: &InputBuffer) -> bool {
    match action {
        MenuAction::SetMode(m) => settings.mode == *m,
        MenuAction::SetDifficulty(d) => match (d, settings.difficulty) {
            // Custom isn't one of the buttons, so no preset is active.
            (_, Difficulty::Custom(_)) => false,
            (a, b) => *a == b,
        },
        MenuAction::SetFirstMove(fm) => settings.first_move == *fm,
        MenuAction::ToggleFriendlyOpening => settings.friendly_opening,
        MenuAction::ToggleDisplayAdvanced => settings.display_advanced,
        MenuAction::ToggleMuteAudio => settings.mute_audio,
        _ => false,
    }
}

fn is_disabled(_action: &MenuAction) -> bool {
    false
}

fn commit_input(input: &mut InputBuffer, settings: &mut GameSettings) {
    let Some(field) = input.focused.take() else { return };
    match field {
        InputField::Iterations => {
            let val = input
                .text
                .parse::<usize>()
                .unwrap_or_else(|_| settings.difficulty.iterations())
                .clamp(100, 10_000_000);
            settings.difficulty = Difficulty::Custom(val);
        }
        InputField::Name => {
            let trimmed = input.text.trim();
            if !trimmed.is_empty() {
                settings.display_name = trimmed.to_string();
            }
            // An empty name keeps the old one.
        }
    }
    input.text.clear();
    hide_native_keyboard();
}

fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}
