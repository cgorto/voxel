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

#[derive(SpacetimeType)]
pub struct dbVec3 {
    x: f32,
    y: f32,
    z: f32,
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

#[table(name = camera, public)]
pub struct Camera {
    #[primary_key]
    identity: Identity,
    #[auto_inc]
    camera_id: u32,
    position: dbVec3,
    pitch: f32,
    roll: f32,
    yaw: f32,
    fov: f32,
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

#[reducer]
pub fn update_camera_data(
    ctx: &ReducerContext,
    posx: f32,
    posy: f32,
    posz: f32,
    roll: f32,
    pitch: f32,
    yaw: f32,
    fov: f32,
) -> Result<(), String> {
    log::info!("new camera");
    Ok(())
}

#[reducer]
pub fn update_ray(ctx: &ReducerContext, pxx: i32, pxy: i32, diff: f32) -> Result<(), String> {
    log::info!("casting ray from pixel: {}, {} value: {}", pxx, pxy, diff);
    Ok(())
}
