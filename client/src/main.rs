use bevy::prelude::*;

use camera_client::AppPlugin;

fn main() {
    App::new().add_plugins(AppPlugin).run();
}
