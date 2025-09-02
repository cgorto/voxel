use spacetimedb::{Identity, ReducerContext, SpacetimeType, Table, Timestamp, reducer, table};

#[derive(SpacetimeType)]
pub struct Voxel {
    x: u32,
    y: u32,
    z: u32,
}
impl Voxel {
    pub fn idx(&self, grid_size: u32) -> usize {
        (self.x + self.y * grid_size + self.z * grid_size * grid_size) as usize
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

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) -> Result<(), String> {
    let new_grid = vec![0.0; (GRID_SIZE * GRID_SIZE * GRID_SIZE)];
    ctx.db.voxel_grid().try_insert(VoxelGrid {
        id: 0,
        voxel_size: 1.0,
        grid: new_grid,
    })?;
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
    let idx = voxel.idx(GRID_SIZE.try_into().unwrap());
    for mut grid in ctx.db.voxel_grid().iter() {
        if idx < grid.grid.len().try_into().unwrap() {
            grid.grid[idx] = value;
            ctx.db.voxel_grid().id().update(grid);
        }
    }
    Ok(())
}
