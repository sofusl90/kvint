//! Online lobby: invite with a QR code or a code, or join a nearby player.
//! Moves on to `AppState::InGame` once connected.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use iroh::EndpointId;
use qrcode::{Color as QrColor, EcLevel, QrCode};

use super::app::{AppState, GameSettings, Mode, PendingDeepLink, SafeAreaInsets};
use super::audio::{ui_text, TitleFont, UiFont};
use super::net::{
    IncomingJoinRequest, JoinDeclinedMessage, NearbyPeer, NetCommand, NetStatus, NetworkState,
    PeerNameCache,
};
use std::collections::HashMap;
use super::theme::{ButtonIntent, ButtonState, Theme};

#[derive(Component)]
pub struct LobbyRoot;

/// A screen whose root scrolls with `Overflow::scroll_y()`. `scroll_screen` needs this marker.
#[derive(Component)]
pub struct ScrollableScreen;

#[derive(Component, Clone)]
pub struct LobbyButton {
    pub action: LobbyAction,
    pub intent: ButtonIntent,
}

#[derive(Clone, PartialEq)]
pub enum LobbyAction {
    JoinFromClipboard,
    JoinNearby(String),
    Copy,
    Back,
    /// Host accepts an incoming join request.
    AcceptIncoming,
    /// Host declines an incoming join request.
    DeclineIncoming,
}

/// Per-peer decline cooldowns. App-level, so leaving the lobby doesn't reset them.
#[derive(Resource, Default)]
pub struct DeclineCooldowns {
    /// `remote_id` → cooldown_until_secs_since_startup
    map: HashMap<String, f32>,
}

/// Flat cooldown after a peer declines.
const DECLINE_COOLDOWN_SECS: f32 = 5.0;

impl DeclineCooldowns {
    pub fn record(&mut self, remote_id: &str, now: f32) {
        // Prune expired entries so the map doesn't grow forever.
        self.map.retain(|_, until| *until > now);
        self.map
            .insert(remote_id.to_string(), now + DECLINE_COOLDOWN_SECS);
    }

    /// Remaining cooldown in seconds; 0 if cleared / never set / expired.
    pub fn remaining(&self, remote_id: &str, now: f32) -> f32 {
        self.map
            .get(remote_id)
            .map(|until| (until - now).max(0.0))
            .unwrap_or(0.0)
    }

    /// Called after a successful connection.
    pub fn clear(&mut self, remote_id: &str) {
        self.map.remove(remote_id);
    }
}


#[derive(Component)]
pub struct StatusPill;
/// Text inside the StatusPill, coloured separately from the pill.
#[derive(Component)]
pub struct StatusPillText;
/// "You are brave-otter", so players can confirm who's who.
#[derive(Component)]
pub struct PetnameText;
/// QR code and Copy button. Hidden while connecting.
#[derive(Component)]
pub struct InviteCard;

/// Connecting…/Cancel card, shown while connecting.
#[derive(Component)]
pub struct ConnectingCard;

/// "— or —" between the Invite and Join cards.
#[derive(Component)]
pub struct OrDivider;

/// Shown in `JoinCard` while no one is nearby.
#[derive(Component)]
pub struct NearbyEmpty;

/// "Connecting…" or an error.
#[derive(Component)]
pub struct ConnectingHeading;

/// Label of a `LobbyButton`, coloured by `update_lobby_buttons`.
#[derive(Component)]
pub struct LobbyButtonLabel;

/// Image node that holds the QR code; populated when the NodeID arrives.
#[derive(Component)]
pub struct QrImageNode;
/// "Generating…" over the QR slot until the endpoint is online.
#[derive(Component)]
pub struct QrLoadingLabel;
/// Container for the dynamically-rebuilt nearby-peer list.
#[derive(Component)]
pub struct NearbyList;
/// Hidden when the nearby list is empty.
#[derive(Component)]
pub struct JoinCard;
/// So the QR is only encoded once per NodeID.
#[derive(Resource, Default)]
pub struct QrGenerated(pub Option<String>);

/// The last rendered peer list, so we only rebuild on changes.
#[derive(Resource, Default)]
pub struct NearbyRendered(pub Vec<NearbyPeer>);

/// Nearby-peer rows. Styled on their own, so `update_lobby_buttons` skips them.
#[derive(Component)]
pub struct NearbyRow;

/// The "Join" button of a nearby row; shows the cooldown countdown.
#[derive(Component)]
pub struct NearbyJoinBtn(pub String);

/// Back button text: "← Back", or "Cancel" while connecting.
#[derive(Component)]
pub struct BackButtonLabel;

/// The host's accept/decline modal for an incoming join.
#[derive(Component)]
pub struct IncomingRequestOverlay;

pub fn setup_lobby(
    mut commands: Commands,
    mut net: ResMut<NetworkState>,
    settings: Res<GameSettings>,
    font: Res<UiFont>,
    title_font: Res<TitleFont>,
    theme: Res<Theme>,
    safe_area: Res<SafeAreaInsets>,
) {
    commands.insert_resource(LobbyError::default());
    commands.insert_resource(QrGenerated::default());
    commands.insert_resource(NearbyRendered::default());
    // `DeclineCooldowns` is app-level, so don't insert it here.

    // Reset a stale `Connected` from the last match, or we'd jump straight to InGame.
    // Keep `Inactive` so `start_net` still spawns the thread.
    net.status = match &net.status {
        NetStatus::Booting => NetStatus::Booting,
        NetStatus::Inactive => NetStatus::Inactive,
        _ => NetStatus::Ready,
    };

    // Debug only: `KVINT_FAKE_PEERS=10 cargo run` fills the nearby list.
    #[cfg(debug_assertions)]
    if let Ok(s) = std::env::var("KVINT_FAKE_PEERS") {
        if let Ok(n) = s.parse::<usize>() {
            for i in 0..n {
                net.nearby.push(crate::bevy::net::NearbyPeer {
                    node_id: format!("{i:0>64x}"),
                    name: Some(format!("fake-peer-{i:02}")),
                });
            }
        }
    }

    let f = font.0.clone();
    let tf = title_font.0.clone();
    let c = &theme.color;

    // Outer scroll container. Single vertical scroll, no nesting.
    commands
        .spawn((
            LobbyRoot,
            ScrollableScreen,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(c.bg_root),
        ))
        .with_children(|root| {
            // Clear the dynamic island and the home indicator.
            let top_pad = safe_area.top.max(20.0) + theme.space.md;
            let bottom_pad = safe_area.bottom + theme.space.md;
            root.spawn(Node {
                width: Val::Percent(100.0),
                max_width: Val::Px(theme.size.column_max),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                padding: UiRect {
                    left: Val::Px(theme.space.lg),
                    right: Val::Px(theme.space.lg),
                    top: Val::Px(top_pad),
                    bottom: Val::Px(bottom_pad),
                },
                row_gap: Val::Px(theme.space.lg),
                ..default()
            })
            .with_children(|col| {
                spawn_header_row(col, &tf, &f, &theme);
                spawn_petname(col, &settings.display_name, &f, &theme);
                spawn_invite_card(col, &f, &theme);
                spawn_or_divider(col, &f, &theme);
                spawn_join_card(col, &f, &theme);
                spawn_connecting_card(col, &f, &theme);
                spawn_back_button(col, &f, &theme);
            });
        });
}

/// Starts hosting once the network is `Ready`, so nearby players can find us.
/// Runs every frame, since `Booting` can turn `Ready` at any time.
pub fn auto_host_on_ready(mut net: ResMut<NetworkState>) {
    if matches!(net.status, NetStatus::Ready) {
        net.send(NetCommand::Host);
        net.status = NetStatus::Hosting;
    }
}

pub fn cleanup_lobby(
    mut commands: Commands,
    roots: Query<Entity, With<LobbyRoot>>,
    overlays: Query<Entity, With<IncomingRequestOverlay>>,
    net: Res<NetworkState>,
) {
    // Drop any host/join attempt, but keep the connection: we may be going to InGame.
    if matches!(net.status, NetStatus::Hosting | NetStatus::Connecting) {
        net.send(NetCommand::Disconnect);
    }
    for e in roots.iter() {
        commands.entity(e).despawn();
    }
    for e in overlays.iter() {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<LobbyError>();
    commands.remove_resource::<QrGenerated>();
    commands.remove_resource::<NearbyRendered>();
    // `DeclineCooldowns` stays: it's app-level.
    commands.remove_resource::<IncomingJoinRequest>();
}

fn spawn_header_row(
    parent: &mut ChildSpawnerCommands,
    title_font: &Handle<Font>,
    body_font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(theme.space.sm),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new("Play online"),
                ui_text(title_font, theme.font_size.h2),
                TextColor(theme.color.headline),
            ));
            // Status pill, only shown for errors.
            col.spawn((
                StatusPill,
                Node {
                    height: Val::Px(22.0),
                    padding: UiRect::horizontal(Val::Px(theme.space.md)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: theme.radius.pill(),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(theme.color.surface),
            ))
            .with_children(|pill| {
                pill.spawn((
                    StatusPillText,
                    Text::new("starting"),
                    ui_text(body_font, theme.font_size.caption),
                    TextColor(theme.color.text_muted),
                ));
            });
        });
}

fn spawn_petname(
    parent: &mut ChildSpawnerCommands,
    name: &str,
    body_font: &Handle<Font>,
    theme: &Theme,
) {
    // Muted "you are" followed by the name in the accent colour.
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(theme.space.xs + 2.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new("you are"),
                ui_text(body_font, theme.font_size.body),
                TextColor(theme.color.text_muted),
            ));
            row.spawn((
                PetnameText,
                Text::new(name.to_string()),
                ui_text(body_font, theme.font_size.body),
                TextColor(theme.color.accent),
            ));
        });
}

fn spawn_invite_card(
    parent: &mut ChildSpawnerCommands,
    body_font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            InviteCard,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    left: Val::Px(theme.space.md),
                    right: Val::Px(theme.space.md),
                    top: Val::Px(theme.space.lg),
                    bottom: Val::Px(theme.space.lg),
                },
                row_gap: Val::Px(theme.space.sm),
                align_items: AlignItems::Stretch,
                border_radius: theme.radius.lg(),
                ..default()
            },
            BackgroundColor(theme.color.surface),
            theme.shadow.elev_1(),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new("Invite a friend"),
                ui_text(body_font, theme.font_size.heading),
                TextColor(theme.color.text_primary),
            ));
            // The QR code, filled in by `populate_qr_image`. "Generating…" shows until then.
            card.spawn((
                QrImageNode,
                ImageNode::default(),
                Node {
                    width: Val::Px(140.0),
                    height: Val::Px(140.0),
                    align_self: AlignSelf::Center,
                    margin: UiRect::vertical(Val::Px(theme.space.sm)),
                    border_radius: theme.radius.sm(),
                    overflow: Overflow::clip(),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(theme.color.input_bg),
            ))
            .with_children(|slot| {
                slot.spawn((
                    QrLoadingLabel,
                    Text::new("Generating…"),
                    ui_text(body_font, theme.font_size.caption),
                    TextColor(theme.color.text_muted),
                ));
            });
            card.spawn((
                Text::new("Show this to a friend, or:"),
                ui_text(body_font, theme.font_size.small),
                TextColor(theme.color.text_muted),
            ));
            spawn_action_button(
                card,
                "Copy code",
                LobbyAction::Copy,
                ButtonIntent::Secondary,
                body_font,
                theme,
            );
        });
}

fn spawn_or_divider(
    parent: &mut ChildSpawnerCommands,
    body_font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            OrDivider,
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|wrap| {
            wrap.spawn((
                Text::new("— or —"),
                ui_text(body_font, theme.font_size.small),
                TextColor(theme.color.text_muted),
            ));
        });
}

fn spawn_join_card(
    parent: &mut ChildSpawnerCommands,
    body_font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            JoinCard,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    left: Val::Px(theme.space.md),
                    right: Val::Px(theme.space.md),
                    top: Val::Px(theme.space.lg),
                    bottom: Val::Px(theme.space.lg),
                },
                row_gap: Val::Px(theme.space.sm),
                align_items: AlignItems::Stretch,
                border_radius: theme.radius.lg(),
                ..default()
            },
            BackgroundColor(theme.color.surface),
            theme.shadow.elev_1(),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new("Join a friend"),
                ui_text(body_font, theme.font_size.heading),
                TextColor(theme.color.text_primary),
            ));
            // Rebuilt by `refresh_nearby_list` when the peers change.
            card.spawn((
                NearbyList,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(theme.space.xs + 2.0),
                    ..default()
                },
            ));
            card.spawn((
                NearbyEmpty,
                Text::new("Looking for players nearby…"),
                ui_text(body_font, theme.font_size.small),
                TextColor(theme.color.text_muted),
            ));
            card.spawn((
                Text::new("Got a code from a friend?"),
                ui_text(body_font, theme.font_size.small),
                TextColor(theme.color.text_muted),
                Node { margin: UiRect::top(Val::Px(theme.space.sm)), ..default() },
            ));
            spawn_action_button(
                card,
                "Paste code",
                LobbyAction::JoinFromClipboard,
                ButtonIntent::Primary,
                body_font,
                theme,
            );
        });
}

fn spawn_connecting_card(
    parent: &mut ChildSpawnerCommands,
    body_font: &Handle<Font>,
    theme: &Theme,
) {
    parent
        .spawn((
            ConnectingCard,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme.space.xl)),
                row_gap: Val::Px(theme.space.lg),
                align_items: AlignItems::Center,
                border_radius: theme.radius.lg(),
                display: Display::None, // hidden until Connecting
                ..default()
            },
            BackgroundColor(theme.color.surface),
            theme.shadow.elev_1(),
        ))
        .with_children(|card| {
            card.spawn((
                ConnectingHeading,
                Text::new("Connecting…"),
                ui_text(body_font, theme.font_size.heading),
                TextColor(theme.color.text_primary),
            ));
        });
}

fn spawn_back_button(
    parent: &mut ChildSpawnerCommands,
    body_font: &Handle<Font>,
    theme: &Theme,
) {
    let intent = ButtonIntent::Ghost;
    parent
        .spawn((
            LobbyButton { action: LobbyAction::Back, intent },
            Button,
            theme.button_node(theme.size.button_h, Val::Auto, 0.0),
            BackgroundColor(theme.button_bg(intent, ButtonState::Normal)),
        ))
        .with_children(|btn| {
            btn.spawn((
                LobbyButtonLabel,
                BackButtonLabel,
                Text::new("← Back"),
                ui_text(body_font, theme.font_size.body),
                TextColor(theme.button_text(intent, ButtonState::Normal)),
            ));
        });
}

fn spawn_action_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: LobbyAction,
    intent: ButtonIntent,
    body_font: &Handle<Font>,
    theme: &Theme,
) {
    let bg = theme.button_bg(intent, ButtonState::Normal);
    let text_color = theme.button_text(intent, ButtonState::Normal);
    // Hairline border, since secondary buttons match the card's background.
    let mut node = theme.button_node(theme.size.button_h, Val::Auto, 0.0);
    node.border = UiRect::all(Val::Px(theme.size.border_thin));
    parent
        .spawn((
            LobbyButton { action, intent },
            Button,
            node,
            BackgroundColor(bg),
            BorderColor::all(theme.color.border),
            theme.shadow.elev_2(),
        ))
        .with_children(|btn| {
            btn.spawn((
                LobbyButtonLabel,
                Text::new(label),
                ui_text(body_font, theme.font_size.body),
                TextColor(text_color),
            ));
        });
}

pub fn lobby_interaction(
    mut commands: Commands,
    buttons: Query<(&Interaction, &LobbyButton), Changed<Interaction>>,
    mut net: ResMut<NetworkState>,
    mut next_state: ResMut<NextState<AppState>>,
    mut errors: ResMut<LobbyError>,
    cooldowns: Res<DeclineCooldowns>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    for (interaction, btn) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Any new action clears stale error text
        errors.0 = None;
        match &btn.action {
            LobbyAction::JoinFromClipboard => {
                if matches!(net.status, NetStatus::Ready | NetStatus::Hosting) {
                    match read_clipboard() {
                        Ok(text) => match extract_join_id(&text).parse::<EndpointId>() {
                            Ok(id) => {
                                if matches!(net.status, NetStatus::Hosting) {
                                    net.send(NetCommand::Disconnect);
                                }
                                net.send(NetCommand::Join(id));
                                net.status = NetStatus::Connecting;
                            }
                            Err(e) => errors.0 = Some(format!("Clipboard isn't a valid code: {e}")),
                        },
                        Err(e) => errors.0 = Some(e),
                    }
                }
            }
            LobbyAction::JoinNearby(node_id) => {
                // Ignore taps while this peer is in cooldown.
                if cooldowns.remaining(node_id, now) > 0.0 {
                    continue;
                }
                // Joining while hosting drops our host attempt; the net thread handles Disconnect → Join.
                if matches!(net.status, NetStatus::Ready | NetStatus::Hosting) {
                    match node_id.parse::<EndpointId>() {
                        Ok(id) => {
                            if matches!(net.status, NetStatus::Hosting) {
                                net.send(NetCommand::Disconnect);
                            }
                            net.send(NetCommand::Join(id));
                            net.status = NetStatus::Connecting;
                        }
                        Err(e) => errors.0 = Some(format!("Bad nearby code: {e}")),
                    }
                }
            }
            LobbyAction::AcceptIncoming => {
                net.send(NetCommand::AcceptJoin);
                commands.remove_resource::<IncomingJoinRequest>();
            }
            LobbyAction::DeclineIncoming => {
                net.send(NetCommand::DeclineJoin);
                commands.remove_resource::<IncomingJoinRequest>();
            }
            LobbyAction::Copy => {
                if let Some(id) = &net.node_id {
                    if let Err(e) = write_clipboard(id) {
                        errors.0 = Some(e);
                    }
                }
            }
            LobbyAction::Back => {
                // While connecting, Back cancels the attempt instead of leaving the lobby.
                if matches!(net.status, NetStatus::Connecting) {
                    net.send(NetCommand::Disconnect);
                    net.status = NetStatus::Ready;
                } else {
                    next_state.set(AppState::Menu);
                }
            }
        }
    }
}

/// Updates the status texts and card visibility, and moves to InGame once connected.
pub fn update_lobby_status(
    net: Res<NetworkState>,
    errors: Res<LobbyError>,
    theme: Res<Theme>,
    mut pill_bg: Query<&mut BackgroundColor, With<StatusPill>>,
    mut pill_text: Query<(&mut Text, &mut TextColor), (With<StatusPillText>, Without<ConnectingHeading>)>,
    mut connecting_text: Query<&mut Text, (With<ConnectingHeading>, Without<StatusPillText>)>,
    mut invite_q: Query<&mut Node, (With<InviteCard>, Without<JoinCard>, Without<OrDivider>, Without<ConnectingCard>)>,
    mut join_q: Query<&mut Node, (With<JoinCard>, Without<InviteCard>, Without<OrDivider>, Without<ConnectingCard>)>,
    mut divider_q: Query<&mut Node, (With<OrDivider>, Without<InviteCard>, Without<JoinCard>, Without<ConnectingCard>)>,
    mut connecting_q: Query<&mut Node, (With<ConnectingCard>, Without<InviteCard>, Without<JoinCard>, Without<OrDivider>)>,
    mut next_state: ResMut<NextState<AppState>>,
    mut last_pill: Local<Option<(String, bool)>>,
) {
    let (pill_label, pill_color, pill_tint, connecting_label) = match (&errors.0, &net.status) {
        (Some(err), _) => (err.clone(), theme.color.text_error, theme.color.surface, err.clone()),
        // Shouldn't happen in the lobby, but show it like Booting if it does.
        (_, NetStatus::Inactive | NetStatus::Booting) => ("starting".into(), theme.color.text_muted, theme.color.surface, "Starting up…".into()),
        (_, NetStatus::Ready) => ("ready".into(), theme.color.text_muted, theme.color.surface, "Connecting…".into()),
        (_, NetStatus::Hosting) => ("waiting".into(), theme.color.title_gold, theme.color.surface_raised, "Connecting…".into()),
        (_, NetStatus::Connecting) => ("connecting".into(), theme.color.title_gold, theme.color.surface_raised, "Connecting…".into()),
        (_, NetStatus::Connected { remote_id, .. }) => (
            format!("connected · {}", short_node_id(remote_id)),
            theme.color.title_gold,
            theme.color.surface_raised,
            format!("Connected to {}…", short_node_id(remote_id)),
        ),
        (_, NetStatus::Error(e)) => (e.clone(), theme.color.text_error, theme.color.surface, e.clone()),
    };

    // Only write on change: Bevy re-lays out all text on every write.
    // The key includes the error flag so an error can't reuse a normal label's colours.
    let key = (pill_label.clone(), errors.0.is_some());
    if last_pill.as_ref() != Some(&key) {
        for (mut t, mut col) in pill_text.iter_mut() {
            *t = Text::new(&key.0);
            *col = TextColor(pill_color);
        }
        for mut bg in pill_bg.iter_mut() {
            *bg = BackgroundColor(pill_tint);
        }
        for mut t in connecting_text.iter_mut() {
            *t = Text::new(&connecting_label);
        }
        *last_pill = Some(key);
    }

    // Connecting/Connected shows only the ConnectingCard; otherwise the rest.
    let in_flight = matches!(
        net.status,
        NetStatus::Connecting | NetStatus::Connected { .. }
    );
    let default_disp = if in_flight { Display::None } else { Display::Flex };
    let inflight_disp = if in_flight { Display::Flex } else { Display::None };
    set_display_if_changed(&mut invite_q, default_disp);
    set_display_if_changed(&mut join_q, default_disp);
    set_display_if_changed(&mut divider_q, default_disp);
    set_display_if_changed(&mut connecting_q, inflight_disp);

    // Auto-transition to InGame once the handshake completes.
    if matches!(net.status, NetStatus::Connected { .. }) {
        next_state.set(AppState::InGame);
    }
}

/// Touch state for `scroll_screen`, including velocity for momentum.
#[derive(Default)]
pub struct ActiveTouch {
    id: u64,
    last_pos: Vec2,
    /// Set once the drag passes the dead zone, so taps still work.
    scrolling: bool,
    /// Smoothed velocity in px/s.
    velocity: f32,
    /// Time of the last Moved event, in seconds since startup.
    last_time: f32,
}

/// Scrolls the `ScrollableScreen` with the mouse wheel or touch, with momentum after release.
/// `Overflow::scroll_y()` only clips, so without this nothing scrolls.
pub fn scroll_screen(
    mut wheel: MessageReader<MouseWheel>,
    mut touches: MessageReader<TouchInput>,
    mut q: Query<(&mut ScrollPosition, &ComputedNode), With<ScrollableScreen>>,
    mut active: Local<Option<ActiveTouch>>,
    mut momentum: Local<f32>,
    time: Res<Time>,
) {
    let Ok((mut pos, computed)) = q.single_mut() else {
        return;
    };
    let max_y = ((computed.content_size().y - computed.size().y).max(0.0)
        * computed.inverse_scale_factor()) as f32;
    let now = time.elapsed_secs();
    let dt = time.delta_secs();

    // ---- Mouse wheel (no momentum — wheel is already discrete) ----
    for ev in wheel.read() {
        let dy = match ev.unit {
            MouseScrollUnit::Line => -ev.y * 24.0,
            MouseScrollUnit::Pixel => -ev.y,
        };
        pos.y = (pos.y + dy).clamp(0.0, max_y);
        // The wheel stops any momentum.
        *momentum = 0.0;
    }

    // ---- Touch ----
    const DRAG_THRESHOLD_PX: f32 = 8.0;
    // Velocity smoothing (0 = none, 1 = never updates).
    const VELOCITY_SMOOTH: f32 = 0.5;
    // Velocity decays by e^(-DECAY*dt): 10% is left after 0.6 s.
    const MOMENTUM_DECAY: f32 = 4.0;
    // Below this magnitude (logical px/s) we snap to 0 to avoid eternal jitter.
    const MOMENTUM_EPS: f32 = 4.0;

    for ev in touches.read() {
        match ev.phase {
            TouchPhase::Started => {
                *active = Some(ActiveTouch {
                    id: ev.id,
                    last_pos: ev.position,
                    scrolling: false,
                    velocity: 0.0,
                    last_time: now,
                });
                // New touch absorbs any in-flight momentum.
                *momentum = 0.0;
            }
            TouchPhase::Moved => {
                let Some(state) = active.as_mut() else { continue };
                if state.id != ev.id {
                    continue;
                }
                let dy = ev.position.y - state.last_pos.y;
                if !state.scrolling
                    && (ev.position - state.last_pos).length() > DRAG_THRESHOLD_PX
                {
                    state.scrolling = true;
                }
                if state.scrolling {
                    pos.y = (pos.y - dy).clamp(0.0, max_y);
                    let sample_dt = (now - state.last_time).max(1e-3);
                    let sample = dy / sample_dt;
                    state.velocity =
                        state.velocity * (1.0 - VELOCITY_SMOOTH) + sample * VELOCITY_SMOOTH;
                }
                state.last_pos = ev.position;
                state.last_time = now;
            }
            TouchPhase::Ended | TouchPhase::Canceled => {
                if let Some(state) = active.as_ref() {
                    if state.id == ev.id {
                        // Only a real scroll gets momentum, not a tap.
                        if state.scrolling {
                            *momentum = state.velocity;
                        }
                    }
                }
                if let Some(state) = active.as_ref() {
                    if state.id == ev.id {
                        *active = None;
                    }
                }
            }
        }
    }

    // ---- Momentum integration ----
    if momentum.abs() > MOMENTUM_EPS && active.is_none() {
        let dy = *momentum * dt;
        pos.y = (pos.y - dy).clamp(0.0, max_y);
        // Exponential decay.
        *momentum *= (-MOMENTUM_DECAY * dt).exp();
        // Stop at the rails so we don't keep decaying when pinned.
        if pos.y == 0.0 || pos.y == max_y {
            *momentum = 0.0;
        }
    } else if momentum.abs() <= MOMENTUM_EPS {
        *momentum = 0.0;
    }
}

/// "← Back" goes to the menu; "Cancel" stops a connection attempt.
pub fn update_back_button_label(
    net: Res<NetworkState>,
    mut q: Query<&mut Text, With<BackButtonLabel>>,
) {
    let target = if matches!(net.status, NetStatus::Connecting) {
        "Cancel"
    } else {
        "← Back"
    };
    for mut t in q.iter_mut() {
        if t.0 != target {
            *t = Text::new(target);
        }
    }
}

/// Shows the StatusPill only for errors. Separate from `update_lobby_status` to avoid query conflicts.
pub fn update_status_pill_display(
    net: Res<NetworkState>,
    errors: Res<LobbyError>,
    mut pill_q: Query<&mut Node, With<StatusPill>>,
) {
    let has_error = errors.0.is_some() || matches!(net.status, NetStatus::Error(_));
    let target = if has_error { Display::Flex } else { Display::None };
    for mut node in pill_q.iter_mut() {
        if node.display != target {
            node.display = target;
        }
    }
}

/// Writes `display` only on change, to avoid triggering change detection.
fn set_display_if_changed<F: bevy::ecs::query::QueryFilter>(
    q: &mut Query<&mut Node, F>,
    target: Display,
) {
    for mut n in q.iter_mut() {
        if n.display != target {
            n.display = target;
        }
    }
}

/// Shows the accept/decline modal while an `IncomingJoinRequest` exists.
pub fn update_incoming_request_overlay(
    mut commands: Commands,
    request: Option<Res<IncomingJoinRequest>>,
    name_cache: Res<PeerNameCache>,
    font: Res<UiFont>,
    theme: Res<Theme>,
    safe_area: Res<SafeAreaInsets>,
    overlays: Query<Entity, With<IncomingRequestOverlay>>,
) {
    let want = request.is_some();
    let has = !overlays.is_empty();
    if want == has {
        return;
    }
    if !want {
        // Resource went away (accept/decline pressed) — tear down.
        for e in overlays.iter() {
            commands.entity(e).despawn();
        }
        return;
    }

    let req = request.expect("checked above");
    // The name from the JoinRequest also works over relay. Then the mDNS cache, then a short id.
    let display_name = if !req.name.is_empty() {
        req.name.clone()
    } else {
        name_cache
            .0
            .get(&req.remote_id)
            .cloned()
            .unwrap_or_else(|| short_node_id(&req.remote_id))
    };

    let f = font.0.clone();
    let top = safe_area.top.max(20.0);
    commands
        .spawn((
            IncomingRequestOverlay,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::top(Val::Px(top)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.color.surface_overlay),
        ))
        .with_children(|root| {
            // Stretch, so the buttons fill the sheet.
            root.spawn((
                Node {
                    align_items: AlignItems::Stretch,
                    min_width: Val::Px(280.0),
                    ..theme.modal_node()
                },
                BackgroundColor(theme.color.surface),
                theme.shadow.elev_3(),
            ))
            .with_children(|sheet| {
                sheet.spawn((
                    Text::new(format!("{display_name} wants to play")),
                    ui_text(&f, theme.font_size.heading),
                    TextColor(theme.color.text_primary),
                    Node { align_self: AlignSelf::Center, ..default() },
                ));
                spawn_action_button(
                    sheet,
                    "Accept",
                    LobbyAction::AcceptIncoming,
                    ButtonIntent::Primary,
                    &f,
                    &theme,
                );
                spawn_action_button(
                    sheet,
                    "Decline",
                    LobbyAction::DeclineIncoming,
                    ButtonIntent::Destructive,
                    &f,
                    &theme,
                );
            });
        });
}

/// Starts a cooldown when a peer declines and clears it when a join succeeds.
pub fn record_decline_cooldowns(
    mut declines: MessageReader<JoinDeclinedMessage>,
    mut successes: MessageReader<super::net::JoinSucceededMessage>,
    mut cooldowns: ResMut<DeclineCooldowns>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    for msg in declines.read() {
        cooldowns.record(&msg.0, now);
    }
    for msg in successes.read() {
        cooldowns.clear(&msg.0);
    }
}

/// Shows the cooldown ("12s") or "Join" on each nearby row.
pub fn update_nearby_join_text(
    cooldowns: Res<DeclineCooldowns>,
    time: Res<Time>,
    btns: Query<(&NearbyJoinBtn, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let now = time.elapsed_secs();
    for (btn, children) in btns.iter() {
        let remaining_secs = cooldowns.remaining(&btn.0, now).ceil() as u32;
        for child in children.iter() {
            let Ok(mut text) = texts.get_mut(child) else {
                continue;
            };
            // Only allocate when the text changes; it ticks once a second.
            let stale = match remaining_secs {
                0 => text.0 != "Join",
                n => match text.0.strip_suffix('s').and_then(|s| s.parse::<u32>().ok()) {
                    Some(parsed) => parsed != n,
                    None => true,
                },
            };
            if stale {
                let label = if remaining_secs == 0 {
                    "Join".to_string()
                } else {
                    format!("{remaining_secs}s")
                };
                *text = Text::new(label);
            }
            break;
        }
    }
}

/// Button colours, written only on change.
pub fn update_lobby_buttons(
    theme: Res<Theme>,
    net: Res<NetworkState>,
    children_q: Query<&Children>,
    mut buttons: Query<(Entity, &Interaction, &LobbyButton, &mut BackgroundColor), Without<NearbyRow>>,
    mut labels: Query<&mut TextColor, With<LobbyButtonLabel>>,
) {
    let copy_disabled = net.node_id.is_none();
    for (entity, interaction, btn, mut bg) in buttons.iter_mut() {
        let intent = btn.intent;
        let state = if matches!(btn.action, LobbyAction::Copy) && copy_disabled {
            ButtonState::Disabled
        } else {
            match interaction {
                Interaction::Hovered | Interaction::Pressed => ButtonState::Hovered,
                _ => ButtonState::Normal,
            }
        };
        let target_bg = theme.button_bg(intent, state);
        if bg.0 != target_bg {
            *bg = BackgroundColor(target_bg);
        }

        let target_text = theme.button_text(intent, state);
        if let Ok(children) = children_q.get(entity) {
            for child in children.iter() {
                if let Ok(mut tc) = labels.get_mut(child) {
                    if tc.0 != target_text {
                        *tc = TextColor(target_text);
                    }
                }
            }
        }
    }
}


#[derive(Resource, Default)]
pub struct LobbyError(pub Option<String>);

// ---------------------------------------------------------------------------
// Clipboard — desktop only. Mobile gets a different UX later.
// ---------------------------------------------------------------------------

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn read_clipboard() -> Result<String, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("clipboard: {e}"))?;
    cb.get_text().map_err(|e| format!("clipboard: {e}"))
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn write_clipboard(text: &str) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("clipboard: {e}"))?;
    cb.set_text(text).map_err(|e| format!("clipboard: {e}"))
}

#[cfg(target_os = "ios")]
extern "C" {
    fn ios_clipboard_read(buf: *mut u8, buf_len: i32) -> i32;
    fn ios_clipboard_write(text: *const std::ffi::c_char);
}

#[cfg(target_os = "ios")]
fn read_clipboard() -> Result<String, String> {
    let mut buf = [0u8; 256];
    let len = unsafe { ios_clipboard_read(buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        return Err("Clipboard is empty".into());
    }
    std::str::from_utf8(&buf[..len as usize])
        .map(|s| s.to_string())
        .map_err(|e| format!("clipboard utf-8: {e}"))
}

#[cfg(target_os = "ios")]
fn write_clipboard(text: &str) -> Result<(), String> {
    let cs = std::ffi::CString::new(text).map_err(|e| format!("clipboard: {e}"))?;
    unsafe { ios_clipboard_write(cs.as_ptr()) };
    Ok(())
}

#[cfg(target_os = "android")]
fn read_clipboard() -> Result<String, String> {
    Err("Clipboard not yet supported on Android".into())
}

#[cfg(target_os = "android")]
fn write_clipboard(_text: &str) -> Result<(), String> {
    Err("Clipboard not yet supported on Android".into())
}

// ---------------------------------------------------------------------------
// QR code generation
// ---------------------------------------------------------------------------

const QR_SCALE: usize = 4; // image pixels per QR module
const QR_QUIET: usize = 2; // module-wide quiet zone around the code

/// QR codes hold a `kvint://` URL, so the iOS Camera offers "Open in Kvint".
/// The scheme is registered in `gen/apple/project.yml`.
const JOIN_URL_PREFIX: &str = "kvint://join/";

/// The NodeID from a `kvint://join/<id>` URL, or the input as-is.
pub fn extract_join_id(input: &str) -> &str {
    let trimmed = input.trim();
    trimmed.strip_prefix(JOIN_URL_PREFIX).unwrap_or(trimmed)
}

/// First 8 chars of a NodeID, for display.
fn short_node_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Turns a `kvint://join/<id>` deep link into a join, going to the lobby first if needed.
/// Waits until the network is ready.
pub fn handle_deep_link(
    mut pending: ResMut<PendingDeepLink>,
    mut net: ResMut<NetworkState>,
    mut settings: ResMut<GameSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    state: Res<State<AppState>>,
) {
    // Don't consume the URL yet: on a cold launch the network isn't ready on the first frame.
    let Some(url) = pending.0.as_ref().cloned() else { return };

    let id_str = extract_join_id(&url);
    let Ok(id) = id_str.parse::<EndpointId>() else {
        bevy::log::warn!("deep-link URL had invalid NodeID: {}", url);
        pending.0 = None;
        return;
    };

    // Don't interrupt a game. Drop the link.
    if matches!(*state.get(), AppState::InGame | AppState::Rematching) {
        bevy::log::info!("deep-link ignored: already in a game");
        pending.0 = None;
        return;
    }

    settings.mode = Mode::Online;

    if !matches!(state.get(), AppState::Lobby) {
        // Keep the link pending and join from the lobby next frame.
        next_state.set(AppState::Lobby);
        return;
    }

    match net.status {
        NetStatus::Ready | NetStatus::Hosting => {
            // Stop hosting before joining, as `JoinFromClipboard` does.
            if matches!(net.status, NetStatus::Hosting) {
                net.send(NetCommand::Disconnect);
            }
            net.send(NetCommand::Join(id));
            net.status = NetStatus::Connecting;
            pending.0 = None;
        }
        NetStatus::Inactive | NetStatus::Booting => {
            // The endpoint isn't up yet. Retry next frame.
        }
        _ => {
            // Already connecting or connected. Drop the link.
            pending.0 = None;
        }
    }
}

fn make_qr_image(node_id: &str, theme: &Theme) -> Image {
    let payload = format!("{JOIN_URL_PREFIX}{node_id}");
    let qr = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M)
        .expect("qr encode");
    let modules = qr.width();
    let dim = (modules + QR_QUIET * 2) * QR_SCALE;
    let mut data = vec![0u8; dim * dim * 4];

    // A light background so the QR reads well.
    let bg = theme.color.board_light.to_srgba().to_u8_array();
    for chunk in data.chunks_exact_mut(4) {
        chunk[0] = bg[0];
        chunk[1] = bg[1];
        chunk[2] = bg[2];
        chunk[3] = 255;
    }
    let fg = theme.color.special_mark_ink.to_srgba().to_u8_array();

    let colors = qr.to_colors();
    for my in 0..modules {
        for mx in 0..modules {
            if colors[my * modules + mx] == QrColor::Dark {
                let px_x = (mx + QR_QUIET) * QR_SCALE;
                let px_y = (my + QR_QUIET) * QR_SCALE;
                for dy in 0..QR_SCALE {
                    for dx in 0..QR_SCALE {
                        let idx = ((px_y + dy) * dim + (px_x + dx)) * 4;
                        data[idx] = fg[0];
                        data[idx + 1] = fg[1];
                        data[idx + 2] = fg[2];
                        data[idx + 3] = 255;
                    }
                }
            }
        }
    }

    Image::new(
        Extent3d {
            width: dim as u32,
            height: dim as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Encodes the QR once per NodeID and hides the "Generating…" label.
pub fn populate_qr_image(
    net: Res<NetworkState>,
    theme: Res<Theme>,
    mut generated: ResMut<QrGenerated>,
    mut images: ResMut<Assets<Image>>,
    mut qr_q: Query<&mut ImageNode, With<QrImageNode>>,
    mut loading_q: Query<&mut Node, With<QrLoadingLabel>>,
) {
    let Some(node_id) = &net.node_id else { return };
    // Hide the label even when the QR is cached, e.g. when re-entering the lobby.
    for mut node in loading_q.iter_mut() {
        if node.display != Display::None {
            node.display = Display::None;
        }
    }
    if generated.0.as_deref() == Some(node_id.as_str()) {
        return;
    }
    let handle = images.add(make_qr_image(node_id, &theme));
    for mut img_node in qr_q.iter_mut() {
        img_node.image = handle.clone();
    }
    generated.0 = Some(node_id.clone());
}

// ---------------------------------------------------------------------------
// mDNS nearby-peer list
// ---------------------------------------------------------------------------

/// Re-spawns the nearby-peer list whenever the discovered set changes.
pub fn refresh_nearby_list(
    mut commands: Commands,
    net: Res<NetworkState>,
    theme: Res<Theme>,
    mut rendered: ResMut<NearbyRendered>,
    list_q: Query<Entity, With<NearbyList>>,
    mut empty_q: Query<&mut Node, With<NearbyEmpty>>,
    font: Res<UiFont>,
) {
    let empty = net.nearby.is_empty();
    // Toggle the empty-state line: visible iff the list is empty.
    for mut node in empty_q.iter_mut() {
        let target = if empty { Display::Flex } else { Display::None };
        if node.display != target {
            node.display = target;
        }
    }
    if net.nearby == rendered.0 {
        return;
    }
    rendered.0 = net.nearby.clone();

    let Ok(list) = list_q.single() else { return };
    let f = font.0.clone();

    // Show at most 6 peers, then "+N more…".
    const MAX_VISIBLE_PEERS: usize = 6;
    let total = net.nearby.len();
    let visible = total.min(MAX_VISIBLE_PEERS);
    let hidden = total.saturating_sub(visible);

    commands.entity(list).despawn_related::<Children>();
    commands.entity(list).with_children(|parent| {
        for peer in net.nearby.iter().take(visible) {
            // The peer's name, or a short id for builds that don't send one.
            let label = peer
                .name
                .clone()
                .unwrap_or_else(|| format!("{}…", short_node_id(&peer.node_id)));
            let action = LobbyAction::JoinNearby(peer.node_id.clone());
            // Not a Button, so touches still scroll the list.
            parent
                .spawn((
                    Node {
                        height: Val::Px(theme.size.button_h_compact + 4.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::horizontal(Val::Px(theme.space.md)),
                        border_radius: theme.radius.md(),
                        ..default()
                    },
                    BackgroundColor(theme.color.surface_raised),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(label),
                        ui_text(&f, theme.font_size.small),
                        TextColor(theme.color.text_primary),
                    ));
                    // `NearbyRow` makes `update_lobby_buttons` leave its styling alone.
                    row.spawn((
                        LobbyButton { action, intent: ButtonIntent::Secondary },
                        NearbyRow,
                        NearbyJoinBtn(peer.node_id.clone()),
                        Button,
                        Node {
                            padding: UiRect::axes(
                                Val::Px(theme.space.md),
                                Val::Px(theme.space.xs),
                            ),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: theme.radius.sm(),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            LobbyButtonLabel,
                            Text::new("Join"),
                            ui_text(&f, theme.font_size.small),
                            TextColor(theme.color.accent),
                        ));
                    });
                });
        }
        if hidden > 0 {
            parent.spawn((
                Text::new(format!("+{hidden} more…")),
                ui_text(&f, theme.font_size.small),
                TextColor(theme.color.text_muted),
                Node {
                    align_self: AlignSelf::Center,
                    margin: UiRect::top(Val::Px(theme.space.xs)),
                    ..default()
                },
            ));
        }
    });
}
