use bevy::audio::{GlobalVolume, Volume};
use bevy::prelude::*;

use crate::game::connect_five::Connect5Player;

#[derive(Resource)]
pub struct GameSounds {
    pub place_p1: Handle<AudioSource>,
    pub place_p2: Handle<AudioSource>,
    pub win: Handle<AudioSource>,
    pub click: Handle<AudioSource>,
    pub connect: Handle<AudioSource>,
    pub disconnect: Handle<AudioSource>,
}

/// Body font (Inter) for all UI text.
/// `FromWorld`, because the first `OnEnter` can run before Startup commands are applied.
#[derive(Resource)]
pub struct UiFont(pub Handle<Font>);

impl FromWorld for UiFont {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        UiFont(asset_server.load("Inter-Regular.ttf"))
    }
}

/// Title font (Cinzel), for screen titles only. Loaded like `UiFont`.
#[derive(Resource)]
pub struct TitleFont(pub Handle<Font>);

impl FromWorld for TitleFont {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        TitleFont(asset_server.load("Cinzel-Regular.ttf"))
    }
}

pub fn setup_sounds(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(GameSounds {
        place_p1: asset_server.load("sounds/place_p1.wav"),
        place_p2: asset_server.load("sounds/place_p2.wav"),
        win: asset_server.load("sounds/win.wav"),
        click: asset_server.load("sounds/click.ogg"),
        connect: asset_server.load("sounds/connect.ogg"),
        disconnect: asset_server.load("sounds/disconnect.ogg"),
    });
}

/// Convenience: a `TextFont` with our serif font at the requested size.
pub fn ui_text(font: &Handle<Font>, size: f32) -> TextFont {
    TextFont { font: font.clone(), font_size: size, ..default() }
}

/// A sound that despawns itself when done. Without `DESPAWN`, every click leaked an entity.
fn spawn_oneshot(commands: &mut Commands, source: Handle<AudioSource>) {
    commands.spawn((AudioPlayer::new(source), PlaybackSettings::DESPAWN));
}

pub fn play_place_sound(commands: &mut Commands, sounds: &GameSounds, player: Connect5Player) {
    let handle = match player {
        Connect5Player::Player1 => &sounds.place_p1,
        Connect5Player::Player2 => &sounds.place_p2,
    };
    spawn_oneshot(commands, handle.clone());
}

pub fn play_win_sound(commands: &mut Commands, sounds: &GameSounds) {
    spawn_oneshot(commands, sounds.win.clone());
}

pub fn play_click(commands: &mut Commands, sounds: &GameSounds) {
    spawn_oneshot(commands, sounds.click.clone());
}

pub fn play_connect(commands: &mut Commands, sounds: &GameSounds) {
    spawn_oneshot(commands, sounds.connect.clone());
}

pub fn play_disconnect(commands: &mut Commands, sounds: &GameSounds) {
    spawn_oneshot(commands, sounds.disconnect.clone());
}

/// Mirrors `mute_audio` into `GlobalVolume`. Sounds already playing aren't affected.
pub fn sync_audio_volume(
    settings: Res<super::app::GameSettings>,
    mut volume: ResMut<GlobalVolume>,
) {
    if !settings.is_changed() {
        return;
    }
    let target = if settings.mute_audio {
        Volume::Linear(0.0)
    } else {
        Volume::Linear(1.0)
    };
    volume.volume = target;
}

/// Plays a click once whenever any button is pressed.
pub fn click_on_any_button(
    mut commands: Commands,
    sounds: Option<Res<GameSounds>>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
) {
    let Some(sounds) = sounds else { return };
    for interaction in buttons.iter() {
        if *interaction == Interaction::Pressed {
            play_click(&mut commands, &sounds);
            // One click per frame is enough.
            return;
        }
    }
}
