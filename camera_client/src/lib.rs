use bevy::prelude::*;

mod components;
mod module_bindings;
mod plugins;
mod resources;

mod prelude {
    pub use super::*;
    pub use {components::*, plugins::*, resources::*};
}

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((DefaultPlugins, plugins::camera::VoxelCameraPlugin));
    }
}
