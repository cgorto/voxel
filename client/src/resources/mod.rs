use crate::module_bindings::*;
use crate::prelude::*;
use bevy::render::{
    extract_resource::ExtractResource,
    render_resource::{BindGroup, BindGroupLayout, CachedComputePipelineId, ShaderType},
    storage::ShaderStorageBuffer,
};

#[derive(Resource, Clone, ExtractResource)]
pub struct CameraTextures {
    pub current: Handle<Image>,
    pub prev: Handle<Image>,
    pub size: IVec2,
    pub new_frame: bool,
}

#[derive(Resource, Clone, ExtractResource)]
pub struct DisplayTexture {
    pub handle: Handle<Image>,
}

#[derive(Resource)]
pub struct ProcessingPipeline {
    pub texture_bind_group_layout: BindGroupLayout,
    // pub raymarch_bind_group_layout: BindGroupLayout,
    pub diff_pipeline: CachedComputePipelineId,
    // pub raymarch_pipeline: CachedComputePipelineId,
}

#[derive(Resource)]
pub struct ProcessingBindGroup(pub BindGroup);

#[derive(Resource, ExtractResource, Clone)]
pub struct VoxelHitBuffer(pub [Handle<ShaderStorageBuffer>; 2]);

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct VoxelHit {
    pub pos_idx: u32,
    pub value: f32,
}
impl Default for VoxelHit {
    fn default() -> Self {
        VoxelHit {
            pos_idx: u32::MAX,
            value: -50.0,
        }
    }
}
impl VoxelHit {
    pub fn voxel(&self, grid_size: u32) -> UVec3 {
        let z = self.pos_idx / (grid_size * grid_size);
        let rem = self.pos_idx % (grid_size * grid_size);
        let y = rem / grid_size;
        let x = rem % grid_size;

        UVec3 { x, y, z }
    }
    pub fn id_from_pos(&mut self, grid_size: u32, voxel: UVec3) {
        self.pos_idx = (voxel.x + voxel.y * grid_size + voxel.z * grid_size * grid_size) as u32
    }
}

#[derive(Resource, Default, ExtractResource, Clone)]
pub struct FrameInfo {
    pub camera_position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub fov: f32,
}

#[derive(Resource, Default, ExtractResource, Clone)]
pub struct VoxelInfo {
    pub n: u32,
    pub voxel_size: f32,
    pub grid_center: Vec3,
}

#[derive(Event, BufferedEvent)]
pub struct VoxelHitEvent {
    pub voxel: Voxel,
    pub value: f32,
}

#[derive(Event, BufferedEvent)]
pub struct RaycastEvent {
    pub pixel: IVec2,
    pub diff: f32,
}
