//! Native data decoders and inspection renderer for Spectre VR/CD 1.1.
//! No Mac API implementation or CPU interpreter is part of this crate.
pub mod binary;
pub mod formats;
pub mod game;
pub mod model;
pub mod pickups;
pub mod radar;
pub mod recovered;
pub mod render;
pub mod resource;
pub mod scene;
pub mod viewer;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub type Result<T> = std::result::Result<T, String>;

pub mod model_slots;

mod collision_preview;

pub mod sound;
