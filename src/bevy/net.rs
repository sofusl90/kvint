//! Networking: the iroh endpoint runs on its own tokio thread and talks to Bevy over mpsc channels.
//!
//! Wire format:
//!   [0x00, name_len, ...UTF-8] — JoinRequest (joiner → host)
//!   [0x01, host_first, _]      — Hello (host → joiner)
//!   [0x02, x, y]               — Move
//!   [0x03, _, _]               — Rematch
//!   [0x04, _, _]               — Decline (host → joiner)

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use iroh::address_lookup::{self, MdnsAddressLookup};
use iroh::endpoint_info::UserData;
use iroh::{endpoint::presets::N0, Endpoint, EndpointId, SecretKey};
use n0_future::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Notify};

use crate::game::connect_five::Move;

/// mDNS tags: `kvint` in the lobby, `kvint-host` while hosting. Only hosts are listed.
/// The display name follows after `|`, e.g. `kvint-host|brave-otter`.
const MDNS_TAG_IDLE: &str = "kvint";
const MDNS_TAG_HOST: &str = "kvint-host";
const MDNS_TAG_SEP: char = '|';

/// `tag|name`, or just `tag` if there's no name.
fn mdns_payload(tag: &str, name: &str) -> String {
    if name.is_empty() {
        tag.to_string()
    } else {
        format!("{tag}{MDNS_TAG_SEP}{name}")
    }
}

/// Splits a UserData payload into `(tag, optional_name)`.
fn split_payload(payload: &str) -> (&str, Option<&str>) {
    match payload.split_once(MDNS_TAG_SEP) {
        Some((tag, name)) if !name.is_empty() => (tag, Some(name)),
        Some((tag, _)) => (tag, None),
        None => (payload, None),
    }
}

const ALPN: &[u8] = b"kvint/1";

/// How long the host has to answer a join request before it's declined.
const ACCEPT_TIMEOUT_SECS: u64 = 30;

// ---------------------------------------------------------------------------
// Public API: commands, events, state resource
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum NetCommand {
    /// Start accepting an incoming connection.
    Host,
    /// Connect to the given peer.
    Join(EndpointId),
    /// Host's response to an incoming JoinRequest: yes, let them in.
    AcceptJoin,
    /// Host's response to an incoming JoinRequest: no, send Decline + close.
    DeclineJoin,
    /// Send a move on the active connection.
    SendMove(Move),
    /// Tell the peer we're ready for a rematch.
    SendRematch,
    /// Close the active connection (or cancel hosting/joining).
    Disconnect,
    /// Changes the name we advertise on mDNS.
    SetDisplayName(String),
}

#[derive(Debug, Clone)]
pub enum NetEvent {
    EndpointReady {
        node_id: String,
    },
    /// A peer wants to play. Declined automatically after 30 s. `name` may be empty.
    JoinRequested {
        remote_id: String,
        name: String,
    },
    /// The host declined; the lobby records a cooldown.
    JoinDeclined {
        remote_id: String,
    },
    /// Connection established + Hello handshake completed.
    Connected {
        remote_id: String,
        is_host: bool,
        /// True if the host plays first; false if the guest plays first.
        first_player_is_host: bool,
    },
    MoveReceived(Move),
    /// Peer requested a rematch.
    RematchRequested,
    /// A new kvint peer was discovered on the local network via mDNS.
    PeerDiscovered { node_id: String, name: Option<String> },
    /// A previously-discovered peer's mDNS record expired.
    PeerExpired { node_id: String },
    Disconnected {
        reason: String,
    },
    Error {
        message: String,
    },
}

/// High-level connection state for the UI to read.
#[derive(Debug, Clone, Default)]
pub enum NetStatus {
    /// No network thread. Set on app start and when entering the menu.
    #[default]
    Inactive,
    Booting,
    Ready,
    Hosting,
    Connecting,
    Connected {
        remote_id: String,
        is_host: bool,
        first_player_is_host: bool,
    },
    Error(String),
}

/// Set by `start_net` and cleared by `stop_net`. `send()` does nothing while they're `None`.
/// `shutdown` lets `stop_net` cancel long awaits, so leaving the lobby doesn't leave threads behind.
#[derive(Resource, Default)]
pub struct NetworkState {
    pub node_id: Option<String>,
    pub status: NetStatus,
    /// Hosting kvint peers that mDNS has spotted on this network.
    pub nearby: Vec<NearbyPeer>,
    cmd_tx: Option<mpsc::UnboundedSender<NetCommand>>,
    evt_rx: Option<Mutex<mpsc::UnboundedReceiver<NetEvent>>>,
    shutdown: Option<Arc<Notify>>,
}

/// A host found over mDNS, with its name if it sent one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NearbyPeer {
    pub node_id: String,
    pub name: Option<String>,
}

/// Node id → name, kept after a peer stops hosting.
/// A joiner stops advertising right before connecting, so the host needs this to show its name.
#[derive(Resource, Default, Debug)]
pub struct PeerNameCache(pub std::collections::HashMap<String, String>);

impl NetworkState {
    pub fn send(&self, cmd: NetCommand) {
        // Only fails if the net thread is gone, and the UI gets an error event then.
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }
}

/// The host's pending accept/decline prompt. An empty `name` falls back to the cache or a short id.
#[derive(Resource, Debug, Clone)]
pub struct IncomingJoinRequest {
    pub remote_id: String,
    pub name: String,
}

/// The host declined us; the lobby starts a cooldown for that peer.
#[derive(Message, Debug, Clone)]
pub struct JoinDeclinedMessage(pub String);

/// A connection succeeded; the lobby clears that peer's cooldown.
#[derive(Message, Debug, Clone)]
pub struct JoinSucceededMessage(pub String);

/// Bevy event message — fired when the net thread reports a remote move.
#[derive(Message, Debug, Clone)]
pub struct RemoteMoveMessage(pub Move);

/// Fired when the peer asks for a rematch.
#[derive(Message, Debug, Clone)]
pub struct RemoteRematchMessage;

// ---------------------------------------------------------------------------
// Bevy plugin wiring
// ---------------------------------------------------------------------------

/// The network only starts in the lobby (`start_net`), since mDNS keeps Wi-Fi awake.
pub fn setup_net(mut commands: Commands) {
    commands.insert_resource(NetworkState::default());
}

/// Starts the iroh endpoint and the mDNS thread. Does nothing if they're already running.
pub fn start_net(
    mut net: ResMut<NetworkState>,
    settings: Res<super::app::GameSettings>,
) {
    if !matches!(net.status, NetStatus::Inactive) {
        return;
    }
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<NetCommand>();
    let (evt_tx, evt_rx) = mpsc::unbounded_channel::<NetEvent>();
    let shutdown = Arc::new(Notify::new());
    spawn_net_thread(cmd_rx, evt_tx, shutdown.clone());
    let _ = cmd_tx.send(NetCommand::SetDisplayName(settings.display_name.clone()));
    net.cmd_tx = Some(cmd_tx);
    net.evt_rx = Some(Mutex::new(evt_rx));
    net.shutdown = Some(shutdown);
    net.status = NetStatus::Booting;
}

/// Stops the network thread. Safe to call when it isn't running.
pub fn stop_net(mut net: ResMut<NetworkState>) {
    if matches!(net.status, NetStatus::Inactive) {
        return;
    }
    if let Some(shutdown) = &net.shutdown {
        // `notify_one` stores a permit, so the signal isn't lost if the thread isn't waiting yet.
        shutdown.notify_one();
    }
    net.cmd_tx = None;
    net.evt_rx = None;
    net.shutdown = None;
    net.node_id = None;
    net.nearby.clear();
    net.status = NetStatus::Inactive;
}

/// Sends the display name to the net thread when it changes, including at startup.
pub fn sync_display_name(
    settings: Res<super::app::GameSettings>,
    net: Res<NetworkState>,
    mut last_sent: Local<Option<String>>,
) {
    if !settings.is_changed() {
        return;
    }
    if last_sent.as_deref() == Some(settings.display_name.as_str()) {
        return;
    }
    net.send(NetCommand::SetDisplayName(settings.display_name.clone()));
    *last_sent = Some(settings.display_name.clone());
}

/// Drains net events into `NetworkState` and forwards moves and rematches to Bevy.
pub fn poll_net_events(
    mut commands: Commands,
    mut net: ResMut<NetworkState>,
    mut remote_moves: MessageWriter<RemoteMoveMessage>,
    mut remote_rematch: MessageWriter<RemoteRematchMessage>,
    mut join_declined: MessageWriter<JoinDeclinedMessage>,
    mut join_succeeded: MessageWriter<JoinSucceededMessage>,
    mut name_cache: ResMut<PeerNameCache>,
    sounds: Option<Res<super::audio::GameSounds>>,
) {
    // `None` when the network thread isn't running.
    let evts: Vec<NetEvent> = {
        let Some(evt_rx) = &net.evt_rx else { return };
        let mut rx = evt_rx.lock().expect("net evt rx poisoned");
        let mut buf = Vec::new();
        while let Ok(evt) = rx.try_recv() {
            buf.push(evt);
        }
        buf
    };

    for evt in evts {
        match evt {
            NetEvent::EndpointReady { node_id } => {
                net.node_id = Some(node_id);
                if matches!(net.status, NetStatus::Booting) {
                    net.status = NetStatus::Ready;
                }
            }
            NetEvent::JoinRequested { remote_id, name } => {
                // The name in the JoinRequest is the most reliable, so cache it too.
                if !name.is_empty() {
                    name_cache.0.insert(remote_id.clone(), name.clone());
                }
                // One prompt at a time; the host only accepts between sessions anyway.
                commands.insert_resource(IncomingJoinRequest { remote_id, name });
            }
            NetEvent::JoinDeclined { remote_id } => {
                // The lobby records the cooldown in `record_decline_cooldowns`.
                net.status = NetStatus::Ready;
                join_declined.write(JoinDeclinedMessage(remote_id));
            }
            NetEvent::Connected {
                remote_id,
                is_host,
                first_player_is_host,
            } => {
                join_succeeded.write(JoinSucceededMessage(remote_id.clone()));
                net.status = NetStatus::Connected {
                    remote_id,
                    is_host,
                    first_player_is_host,
                };
                if let Some(s) = &sounds {
                    super::audio::play_connect(&mut commands, s);
                }
            }
            NetEvent::MoveReceived(mv) => {
                remote_moves.write(RemoteMoveMessage(mv));
            }
            NetEvent::RematchRequested => {
                remote_rematch.write(RemoteRematchMessage);
            }
            NetEvent::PeerDiscovered { node_id, name } => {
                // Cache the name for after the peer stops hosting (see `PeerNameCache`).
                bevy::log::info!(
                    "net: PeerDiscovered node_id={node_id} name={name:?}"
                );
                if let Some(n) = &name {
                    name_cache.0.insert(node_id.clone(), n.clone());
                }
                // Update in place if the name changed; new peers go at the end.
                if let Some(existing) = net.nearby.iter_mut().find(|p| p.node_id == node_id) {
                    if existing.name != name {
                        existing.name = name;
                    }
                } else {
                    net.nearby.push(NearbyPeer { node_id, name });
                }
            }
            NetEvent::PeerExpired { node_id } => {
                net.nearby.retain(|p| p.node_id != node_id);
            }
            NetEvent::Disconnected { .. } => {
                // The endpoint still works, so go back to Ready.
                net.status = NetStatus::Ready;
            }
            NetEvent::Error { message } => {
                // Errors while connecting or hosting are transient and the session re-listens,
                // so only log them. Other errors are shown.
                match net.status {
                    NetStatus::Connecting => {
                        bevy::log::info!("net: connect failed: {message}");
                        net.status = NetStatus::Ready;
                    }
                    NetStatus::Hosting => {
                        bevy::log::info!("net: host transient error: {message}");
                        // Stay in Hosting — host_session loops on its own.
                    }
                    _ => {
                        net.status = NetStatus::Error(message);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tokio thread
// ---------------------------------------------------------------------------

fn spawn_net_thread(
    cmd_rx: mpsc::UnboundedReceiver<NetCommand>,
    evt_tx: mpsc::UnboundedSender<NetEvent>,
    shutdown: Arc<Notify>,
) {
    let evt_tx_err = evt_tx.clone();
    if let Err(e) = std::thread::Builder::new()
        .name("kvint-net".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = evt_tx.send(NetEvent::Error {
                        message: format!("tokio runtime: {e}"),
                    });
                    return;
                }
            };
            runtime.block_on(net_main(cmd_rx, evt_tx, shutdown));
        })
    {
        let _ = evt_tx_err.send(NetEvent::Error {
            message: format!("net thread: {e}"),
        });
    }
}

/// What we advertise on mDNS. Both writers push it with [`publish_advert`].
#[derive(Default)]
struct AdvertState {
    name: String,
    hosting: bool,
}

fn publish_advert(endpoint: &Endpoint, state: &AdvertState) {
    let tag = if state.hosting { MDNS_TAG_HOST } else { MDNS_TAG_IDLE };
    if let Ok(ud) = UserData::try_from(mdns_payload(tag, &state.name)) {
        endpoint.set_user_data_for_address_lookup(Some(ud));
    }
}

async fn net_main(
    cmd_rx: mpsc::UnboundedReceiver<NetCommand>,
    evt_tx: mpsc::UnboundedSender<NetEvent>,
    shutdown: Arc<Notify>,
) {
    let secret = SecretKey::generate();
    // Race against shutdown, so `stop_net` never waits on it.
    let endpoint = tokio::select! {
        biased;
        _ = shutdown.notified() => return,
        res = Endpoint::builder(N0)
            .secret_key(secret)
            .alpns(vec![ALPN.to_vec()])
            .bind() => match res {
            Ok(ep) => ep,
            Err(e) => {
                let _ = evt_tx.send(NetEvent::Error {
                    message: format!("bind: {e}"),
                });
                return;
            }
        }
    };

    // The relay handshake takes 1–2 s, so race it against shutdown.
    tokio::select! {
        biased;
        _ = shutdown.notified() => return,
        _ = endpoint.online() => {}
    }

    // Starts idle with no name.
    let advert = Arc::new(Mutex::new(AdvertState::default()));
    publish_advert(&endpoint, &advert.lock().expect("advert poisoned"));

    // Handle name changes here so they work in any session, and forward everything else.
    let (filt_tx, filt_rx) = mpsc::unbounded_channel();
    {
        let advert = advert.clone();
        let endpoint = endpoint.clone();
        let mut cmd_rx = cmd_rx;
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    NetCommand::SetDisplayName(name) => {
                        let mut a = advert.lock().expect("advert poisoned");
                        a.name = name;
                        publish_advert(&endpoint, &a);
                    }
                    other => {
                        if filt_tx.send(other).is_err() {
                            return;
                        }
                    }
                }
            }
        });
    }
    let mut filt_rx = filt_rx;

    // mDNS discovery on the local network. Optional, since codes work without it.
    if let Ok(mdns) = MdnsAddressLookup::builder().build(endpoint.id()) {
        if let Ok(lookup) = endpoint.address_lookup() {
            lookup.add(mdns.clone());
            let evt_tx_for_mdns = evt_tx.clone();
            let our_id = endpoint.id();
            tokio::spawn(async move {
                let mut stream = mdns.subscribe().await;
                while let Some(event) = stream.next().await {
                    match event {
                        address_lookup::DiscoveryEvent::Discovered { endpoint_info, .. } => {
                            // Skip ourselves.
                            if endpoint_info.endpoint_id == our_id {
                                continue;
                            }
                            let payload = endpoint_info
                                .data
                                .user_data()
                                .map(|u| u.as_ref().to_string());
                            let id = endpoint_info.endpoint_id.to_string();
                            let (tag, name) = match payload.as_deref() {
                                Some(p) => {
                                    let (t, n) = split_payload(p);
                                    (Some(t), n.map(|s| s.to_string()))
                                }
                                None => (None, None),
                            };
                            match tag {
                                Some(MDNS_TAG_HOST) => {
                                    // Actively hosting — joinable.
                                    let _ = evt_tx_for_mdns.send(
                                        NetEvent::PeerDiscovered { node_id: id, name },
                                    );
                                }
                                Some(MDNS_TAG_IDLE) => {
                                    // Not hosting anymore: drop it from the nearby list.
                                    let _ = evt_tx_for_mdns
                                        .send(NetEvent::PeerExpired { node_id: id });
                                }
                                _ => {
                                    // Not a kvint peer, ignore.
                                }
                            }
                        }
                        address_lookup::DiscoveryEvent::Expired { endpoint_id, .. } => {
                            let _ = evt_tx_for_mdns.send(NetEvent::PeerExpired {
                                node_id: endpoint_id.to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            });
        }
    }

    let _ = evt_tx.send(NetEvent::EndpointReady {
        node_id: endpoint.id().to_string(),
    });

    // After a session, wait 1 s before the next so iroh can tear down the old state.
    // Shorter can miss an mDNS announcement. Cold-start joins skip the wait.
    let mut just_finished_session = false;
    let settle = std::time::Duration::from_millis(1000);
    while let Some(cmd) = filt_rx.recv().await {
        match cmd {
            NetCommand::Host => {
                if just_finished_session {
                    bevy::log::info!("net: settling {settle:?} before re-host");
                    tokio::time::sleep(settle).await;
                }
                bevy::log::info!("net: starting host session");
                host_session(&endpoint, &mut filt_rx, &evt_tx, &advert).await;
                bevy::log::info!("net: host session ended");
                just_finished_session = true;
            }
            NetCommand::Join(peer) => {
                if just_finished_session {
                    bevy::log::info!("net: settling {settle:?} before join");
                    tokio::time::sleep(settle).await;
                }
                bevy::log::info!("net: starting join session to {peer}");
                join_session(&endpoint, peer, &mut filt_rx, &evt_tx, &advert, &shutdown).await;
                bevy::log::info!("net: join session ended");
                just_finished_session = true;
            }
            _ => {} // ignore SendMove / Disconnect / SetDisplayName (filtered) while idle
        }
    }

    endpoint.close().await;
}

/// Advertises us as hosting while alive, and as idle again when dropped.
struct MdnsHostingGuard<'a> {
    endpoint: &'a Endpoint,
    advert: &'a Arc<Mutex<AdvertState>>,
}

impl<'a> MdnsHostingGuard<'a> {
    fn new(endpoint: &'a Endpoint, advert: &'a Arc<Mutex<AdvertState>>) -> Self {
        {
            let mut a = advert.lock().expect("advert poisoned");
            a.hosting = true;
            publish_advert(endpoint, &a);
        }
        Self { endpoint, advert }
    }
}

impl Drop for MdnsHostingGuard<'_> {
    fn drop(&mut self) {
        let mut a = self.advert.lock().expect("advert poisoned");
        a.hosting = false;
        publish_advert(self.endpoint, &a);
    }
}

async fn host_session(
    endpoint: &Endpoint,
    cmd_rx: &mut mpsc::UnboundedReceiver<NetCommand>,
    evt_tx: &mpsc::UnboundedSender<NetEvent>,
    advert: &Arc<Mutex<AdvertState>>,
) {
    let _hosting = MdnsHostingGuard::new(endpoint, advert);

    // Re-listen after declines, timeouts and handshake errors, so the host stays reachable.
    // Only a disconnect, a closed endpoint or a finished game ends the loop.
    'session: loop {
        // Wait for an incoming connection, but allow user to cancel.
        let incoming = loop {
            tokio::select! {
                inc = endpoint.accept() => {
                    match inc {
                        Some(i) => break i,
                        None => {
                            let _ = evt_tx.send(NetEvent::Error {
                                message: "endpoint closed".into(),
                            });
                            return;
                        }
                    }
                }
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(NetCommand::Disconnect) | None => return,
                        _ => {} // ignore other commands while waiting
                    }
                }
            }
        };

        bevy::log::info!("net: host accept fired — completing handshake");
        // Every step races `cmd_rx`, so a disconnect always wins.
        let conn = tokio::select! {
            r = incoming => match r {
                Ok(c) => c,
                Err(e) => {
                    bevy::log::warn!("net: host incoming.await failed: {e}");
                    let _ = evt_tx.send(NetEvent::Error { message: format!("accept: {e}") });
                    continue 'session;
                }
            },
            cmd = cmd_rx.recv() => {
                if matches!(cmd, Some(NetCommand::Disconnect) | None) { return; }
                continue 'session;
            }
        };
        bevy::log::info!("net: host conn established with {} — awaiting bidi stream", conn.remote_id());

        let (mut send, mut recv) = tokio::select! {
            r = conn.accept_bi() => match r {
                Ok(s) => s,
                Err(e) => {
                    bevy::log::warn!("net: host accept_bi failed: {e}");
                    let _ = evt_tx.send(NetEvent::Error { message: format!("accept_bi: {e}") });
                    continue 'session;
                }
            },
            cmd = cmd_rx.recv() => {
                if matches!(cmd, Some(NetCommand::Disconnect) | None) {
                    conn.close(0u32.into(), b"cancelled");
                    return;
                }
                continue 'session;
            }
        };
        bevy::log::info!("net: host bidi opened — awaiting JoinRequest frame");

        // Wait for the JoinRequest. It opens the stream (QUIC opens streams lazily) and carries the name.
        let probe_frame = tokio::select! {
            r = read_frame(&mut recv) => Some(r),
            cmd = cmd_rx.recv() => {
                if matches!(cmd, Some(NetCommand::Disconnect) | None) {
                    conn.close(0u32.into(), b"cancelled");
                    return;
                }
                None
            }
        };
        let joiner_name = match probe_frame {
            Some(Ok(Frame::JoinRequest { name })) => name,
            Some(Ok(_)) => {
                bevy::log::warn!("net: host probe got non-JoinRequest frame");
                let _ = evt_tx.send(NetEvent::Error {
                    message: "protocol: expected JoinRequest".into(),
                });
                continue 'session;
            }
            Some(Err(e)) => {
                bevy::log::warn!("net: host probe read failed: {e}");
                let _ = evt_tx.send(NetEvent::Error { message: format!("hello probe: {e}") });
                continue 'session;
            }
            None => continue 'session,
        };

        // Ask the user, and decline automatically after 30 s.
        let remote_id = conn.remote_id().to_string();
        bevy::log::info!(
            "net: incoming join from {remote_id} (name={joiner_name:?}) — surfacing to UI"
        );
        let _ = evt_tx.send(NetEvent::JoinRequested {
            remote_id,
            name: joiner_name,
        });
        let timeout_at =
            tokio::time::Instant::now() + std::time::Duration::from_secs(ACCEPT_TIMEOUT_SECS);
        let accept = 'await_decision: loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(NetCommand::AcceptJoin) => break 'await_decision true,
                        Some(NetCommand::DeclineJoin) => break 'await_decision false,
                        Some(NetCommand::Disconnect) | None => {
                            conn.close(0u32.into(), b"cancelled");
                            return;
                        }
                        _ => {} // ignore unrelated commands while waiting
                    }
                }
                _ = tokio::time::sleep_until(timeout_at) => {
                    break 'await_decision false; // auto-decline after 30s
                }
            }
        };

        if !accept {
            let _ = send_frame(&mut send, Frame::Decline).await;
            // Finish the stream before closing, so the joiner reads the Decline instead of an error.
            let _ = send.finish();
            conn.close(0u32.into(), b"declined");
            continue 'session; // listen for the next peer
        }

        // Coin flip for who starts.
        let first_player_is_host = rand::random::<bool>();
        if let Err(e) = send_frame(&mut send, Frame::Hello { first_player_is_host }).await {
            let _ = evt_tx.send(NetEvent::Error { message: format!("hello: {e}") });
            continue 'session;
        }

        let _ = evt_tx.send(NetEvent::Connected {
            remote_id: conn.remote_id().to_string(),
            is_host: true,
            first_player_is_host,
        });

        play_session(&mut send, &mut recv, cmd_rx, evt_tx).await;

        let _ = send.finish();
        conn.close(0u32.into(), b"bye");
        return; // game ended — exit host_session
    }
}

async fn join_session(
    endpoint: &Endpoint,
    peer: EndpointId,
    cmd_rx: &mut mpsc::UnboundedReceiver<NetCommand>,
    evt_tx: &mpsc::UnboundedSender<NetEvent>,
    advert: &Arc<Mutex<AdvertState>>,
    shutdown: &Arc<Notify>,
) {
    // Connecting can retry forever, so time out after 30 s and race against shutdown.
    let connect_timeout = std::time::Duration::from_secs(30);
    let conn = tokio::select! {
        biased;
        _ = shutdown.notified() => return,
        res = tokio::time::timeout(connect_timeout, endpoint.connect(peer, ALPN)) => match res {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                let _ = evt_tx.send(NetEvent::Error {
                    message: format!("connect: {e}"),
                });
                return;
            }
            Err(_) => {
                let _ = evt_tx.send(NetEvent::Error {
                    message: "connect timed out — is the host's code correct, and are they hosting?".into(),
                });
                return;
            }
        }
    };

    let (mut send, mut recv) = match conn.open_bi().await {
        Ok(s) => s,
        Err(e) => {
            let _ = evt_tx.send(NetEvent::Error {
                message: format!("open_bi: {e}"),
            });
            return;
        }
    };

    // The JoinRequest opens the stream and carries our name, which also works over relay.
    let our_name = advert.lock().expect("advert poisoned").name.clone();
    if let Err(e) = send_frame(&mut send, Frame::JoinRequest { name: our_name }).await {
        let _ = evt_tx.send(NetEvent::Error {
            message: format!("open: {e}"),
        });
        return;
    }

    // Read errors count as errors, not declines, so a flaky network doesn't start a cooldown.
    let first_player_is_host = match read_frame(&mut recv).await {
        Ok(Frame::Hello { first_player_is_host }) => first_player_is_host,
        Ok(Frame::Decline) => {
            bevy::log::info!("net: host declined our join");
            let _ = evt_tx.send(NetEvent::JoinDeclined {
                remote_id: conn.remote_id().to_string(),
            });
            return;
        }
        Ok(_) => {
            let _ = evt_tx.send(NetEvent::Error {
                message: "protocol: expected Hello or Decline".into(),
            });
            return;
        }
        Err(e) => {
            let _ = evt_tx.send(NetEvent::Error {
                message: format!("hello recv: {e}"),
            });
            return;
        }
    };

    let _ = evt_tx.send(NetEvent::Connected {
        remote_id: conn.remote_id().to_string(),
        is_host: false,
        first_player_is_host,
    });

    play_session(&mut send, &mut recv, cmd_rx, evt_tx).await;

    let _ = send.finish();
    conn.close(0u32.into(), b"bye");
}

async fn play_session<S, R>(
    send: &mut S,
    recv: &mut R,
    cmd_rx: &mut mpsc::UnboundedReceiver<NetCommand>,
    evt_tx: &mpsc::UnboundedSender<NetEvent>,
) where
    S: AsyncWriteExt + Unpin,
    R: AsyncReadExt + Unpin,
{
    let mut buf = [0u8; 3];
    let reason = loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(NetCommand::SendMove(mv)) => {
                        if let Err(e) = send_frame(send, Frame::Move { x: mv.x, y: mv.y }).await {
                            break format!("send: {e}");
                        }
                    }
                    Some(NetCommand::SendRematch) => {
                        if let Err(e) = send_frame(send, Frame::Rematch).await {
                            break format!("send: {e}");
                        }
                    }
                    Some(NetCommand::Disconnect) | None => break "left match".to_string(),
                    _ => {}
                }
            }
            r = recv.read_exact(&mut buf) => {
                match r {
                    Ok(_) => match decode_fixed(&buf) {
                        Some(Frame::Move { x, y }) => {
                            let _ = evt_tx.send(NetEvent::MoveReceived(
                                Move::new(x as usize, y as usize),
                            ));
                        }
                        Some(Frame::Rematch) => {
                            let _ = evt_tx.send(NetEvent::RematchRequested);
                        }
                        _ => break "bad frame".into(),
                    },
                    Err(e) => break format!("read: {e}"),
                }
            }
        }
    };
    let _ = evt_tx.send(NetEvent::Disconnected { reason });
}

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum Frame {
    /// Joiner → host: `[0x00, name_len, ...UTF-8]`.
    JoinRequest { name: String },
    /// Host → joiner: accept signal + first-player decision.
    Hello { first_player_is_host: bool },
    /// Host → joiner: the join was declined.
    Decline,
    Move { x: u8, y: u8 },
    Rematch,
}

/// Decodes the fixed 3-byte frames. `JoinRequest` is handled by `read_frame`.
fn decode_fixed(b: &[u8; 3]) -> Option<Frame> {
    match b[0] {
        0x01 => Some(Frame::Hello { first_player_is_host: b[1] != 0 }),
        0x02 => Some(Frame::Move { x: b[1], y: b[2] }),
        0x03 => Some(Frame::Rematch),
        0x04 => Some(Frame::Decline),
        _ => None,
    }
}

async fn send_frame<S: AsyncWriteExt + Unpin>(
    send: &mut S,
    frame: Frame,
) -> std::io::Result<()> {
    match frame {
        Frame::JoinRequest { name } => {
            // Names are capped at 255 bytes.
            let bytes = name.as_bytes();
            let len = bytes.len().min(u8::MAX as usize) as u8;
            send.write_all(&[0x00, len]).await?;
            if len > 0 {
                send.write_all(&bytes[..len as usize]).await?;
            }
        }
        Frame::Hello { first_player_is_host } => {
            send.write_all(&[0x01, first_player_is_host as u8, 0]).await?;
        }
        Frame::Move { x, y } => {
            send.write_all(&[0x02, x, y]).await?;
        }
        Frame::Rematch => {
            send.write_all(&[0x03, 0, 0]).await?;
        }
        Frame::Decline => {
            send.write_all(&[0x04, 0, 0]).await?;
        }
    }
    send.flush().await?;
    Ok(())
}

async fn read_frame<R: AsyncReadExt + Unpin>(recv: &mut R) -> std::io::Result<Frame> {
    let mut type_byte = [0u8; 1];
    recv.read_exact(&mut type_byte).await?;
    match type_byte[0] {
        0x00 => {
            let mut len_byte = [0u8; 1];
            recv.read_exact(&mut len_byte).await?;
            let len = len_byte[0] as usize;
            let mut name_bytes = vec![0u8; len];
            if len > 0 {
                recv.read_exact(&mut name_bytes).await?;
            }
            // Invalid UTF-8 becomes an empty name rather than an error.
            let name = String::from_utf8(name_bytes).unwrap_or_default();
            Ok(Frame::JoinRequest { name })
        }
        b => {
            let mut tail = [0u8; 2];
            recv.read_exact(&mut tail).await?;
            let buf = [b, tail[0], tail[1]];
            decode_fixed(&buf).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "bad frame")
            })
        }
    }
}
