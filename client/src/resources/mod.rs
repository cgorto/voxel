use crate::prelude::*;
use bevy::render::{
    extract_resource::ExtractResource,
    render_resource::{BindGroup, BindGroupLayout, CachedComputePipelineId},
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
    pub pipeline: CachedComputePipelineId,
}

#[derive(Resource)]
pub struct ProcessingBindGroup(pub BindGroup);

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
