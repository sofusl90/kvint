// Bevy systems need complex queries and many parameters.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

pub mod game;
pub mod hand_crafted_model;
pub mod bevy;

pub const BOARD_SIZE: usize = 11;
pub const C: f32 = 0.81;

/// Entry point for all platforms. iOS starts through `start_app` below.
#[::bevy::prelude::bevy_main]
pub fn main() {
    crate::bevy::app::main();
}

/// Entry symbol invoked from the iOS scaffolding's Objective-C bridge.
#[no_mangle]
#[inline(never)]
#[cfg(target_os = "ios")]
pub extern "C" fn start_app() {
    main();
}
