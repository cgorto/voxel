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
                .with_run_fn(DbConnection::run_threaded)
                .add_table(RemoteTables::voxel_grid),
        );
    }
}

fn on_connected(mut events: ReadStdbConnectedEvent, stdb: Res<StdbConnection<DbConnection>>) {
    for _ in events.read() {
        stdb.subscription_builder().subscribe_to_all_tables();
    }
}

fn on_voxel_grid_inserted(mut commands: Commands, mut events: ReadInsertEvent<VoxelGrid>) {
    for event in events.read() {
        for value in event.row.grid {
            if value > std::f32::EPSILON {}
        }
    }
}
