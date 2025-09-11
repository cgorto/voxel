use crate::module_bindings::*;
use crate::prelude::*;
use bevy_spacetimedb::*;
pub struct ConnectionPlugin;
const HOST: &str = "http://localhost:3000";
const DB_NAME: &str = "voxel";

impl Plugin for ConnectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            StdbPlugin::default()
                .with_uri(HOST)
                .with_module_name(DB_NAME)
                .with_run_fn(DbConnection::run_threaded),
        );
        // .add_systems(Update, send_voxel_update);
    }
}

pub fn send_raycast_update(
    mut events: EventReader<RaycastEvent>,
    stdb: Option<Res<StdbConnection<DbConnection>>>,
) {
    if let Some(stdb) = stdb {
        for event in events.read() {
            stdb.reducers()
                .update_ray(event.pixel.x, event.pixel.y, event.diff);
        }
    }
}
