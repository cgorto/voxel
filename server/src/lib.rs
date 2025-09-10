use spacetimedb::{Identity, ReducerContext, SpacetimeType, Table, Timestamp, reducer, table};

#[derive(SpacetimeType)]
pub struct Voxel {
    x: u32,
    y: u32,
    z: u32,
}
impl Voxel {
    pub fn idx(&self, grid_size: u32) -> u32 {
        (self.x + self.y * grid_size + self.z * grid_size * grid_size) as u32
    }
    pub fn from_idx(idx: u32, grid_size: u32) -> Self {
        let z = idx / (grid_size * grid_size);
        let rem = idx % (grid_size * grid_size);
        let y = rem / grid_size;
        let x = rem % grid_size;

        Voxel { x, y, z }
    }
}

const GRID_SIZE: usize = 100;
#[table(name = voxel_grid, public)]
pub struct VoxelGrid {
    #[primary_key]
    #[auto_inc]
    pub id: u32,
    pub voxel_size: f32,
    pub grid: Vec<f32>,
}

#[table(name = voxel_entry, public)]
pub struct VoxelEntry {
    #[primary_key]
    #[auto_inc]
    #[index(btree)]
    pub id: u32,
    pub value: f32,
}

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) -> Result<(), String> {
    Ok(())
}

#[spacetimedb::reducer(client_connected)]
pub fn identity_connected(_ctx: &ReducerContext) {
    // Called everytime a new client connects
}

#[spacetimedb::reducer(client_disconnected)]
pub fn identity_disconnected(_ctx: &ReducerContext) {
    // Called everytime a client disconnects
}

#[spacetimedb::reducer]
pub fn update_voxel(ctx: &ReducerContext, voxel: Voxel, value: f32) -> Result<(), String> {
    log::info!("voxel update");
    let idx = voxel.idx(GRID_SIZE.try_into().unwrap());
    if let Some(mut entry) = ctx.db.voxel_entry().id().find(idx) {
        entry.value += value //THIS **WILL NOT** WORK WITH MULTIPLE CLIENTS
    } else {
    }

    Ok(())
}
