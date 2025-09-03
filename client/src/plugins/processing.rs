use std::borrow::Cow;

use bevy::{
    camera::primitives::Aabb,
    math::bounding::{Aabb3d, RayCast3d},
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::ExtractResourcePlugin,
        render_asset::RenderAssets,
        render_graph::{self, Node, RenderGraph, RenderLabel},
        render_resource::{
            BindGroupLayoutEntries, PipelineCache, ShaderStages, binding_types::texture_storage_2d,
            *,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};

use crate::prelude::*;

pub struct ImageProcessingPlugin;

const SHADER_ASSET_PATH: &str = "shaders/difference_mask.wgsl";
const WORKGROUP_SIZE: u32 = 8;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ProcessingLabel;
impl Plugin for ImageProcessingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractResourcePlugin::<CameraTextures>::default(),
            ExtractResourcePlugin::<DisplayTexture>::default(),
            ExtractResourcePlugin::<FrameInfo>::default(),
            ExtractResourcePlugin::<VoxelInfo>::default(),
        ));
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .add_systems(RenderStartup, init_processing_pipeline)
            .add_systems(
                Render,
                prepare_bind_group.in_set(RenderSystems::PrepareBindGroups),
            );
        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(ProcessingLabel, ProcessingNode::default());
        render_graph.add_node_edge(ProcessingLabel, bevy::render::graph::CameraDriverLabel);
    }
}

fn init_processing_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let texture_bind_group_layout = render_device.create_bind_group_layout(
        "DifferenceMask",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::ReadOnly),
                texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::ReadOnly),
                texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::WriteOnly),
            ),
        ),
    );
    let shader = asset_server.load(SHADER_ASSET_PATH);
    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        layout: vec![texture_bind_group_layout.clone()],
        shader: shader.clone(),
        entry_point: Some(Cow::from("diff")),
        zero_initialize_workgroup_memory: true,
        ..default()
    });

    commands.insert_resource(ProcessingPipeline {
        texture_bind_group_layout,
        pipeline,
    });
}

fn prepare_bind_group(
    mut commands: Commands,
    pipeline: Res<ProcessingPipeline>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    camera_images: Res<CameraTextures>,
    display_texture: Res<DisplayTexture>,
    voxel_info: Res<VoxelInfo>,
    frame_info: Res<FrameInfo>,
    render_device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let current = gpu_images.get(&camera_images.current).unwrap();
    let prev = gpu_images.get(&camera_images.prev).unwrap();
    let target = gpu_images.get(&display_texture.handle).unwrap();

    let bind_group_0 = render_device.create_bind_group(
        None,
        &pipeline.texture_bind_group_layout,
        &BindGroupEntries::sequential((
            &current.texture_view,
            &prev.texture_view,
            &target.texture_view,
        )),
    );
    commands.insert_resource(ProcessingBindGroup(bind_group_0));

    let bound = Aabb3d::new(
        voxel_info.grid_center,
        Vec3::splat(0.5 * (voxel_info.n as f32 * voxel_info.voxel_size)),
    );
    let rot = Quat::from_euler(
        EulerRot::YXZ,
        frame_info.yaw,
        frame_info.pitch,
        frame_info.roll,
    );
    let dir = rot * Dir3::NEG_Z;
    let ray = RayCast3d::new(frame_info.camera_position, dir, 2.0 * voxel_info.voxel_size);
    if let Some(_) = ray.aabb_intersection_at(&bound) {
    } else {
        return;
    }
}

enum ProcessingState {
    Loading,
    Init,
}

struct ProcessingNode {
    state: ProcessingState,
}
impl Default for ProcessingNode {
    fn default() -> Self {
        Self {
            state: ProcessingState::Loading,
        }
    }
}

impl Node for ProcessingNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<ProcessingPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        match self.state {
            ProcessingState::Loading => {
                match pipeline_cache.get_compute_pipeline_state(pipeline.pipeline) {
                    CachedPipelineState::Ok(_) => self.state = ProcessingState::Init,
                    CachedPipelineState::Err(
                        bevy::shader::PipelineCacheError::ShaderNotLoaded(_),
                    ) => {}
                    CachedPipelineState::Err(err) => {
                        panic!("Initializing assets/{SHADER_ASSET_PATH}: \n{err}")
                    }
                    _ => {}
                }
            }
            ProcessingState::Init => {}
        }
    }

    fn run(
        &self,
        graph: &mut render_graph::RenderGraphContext,
        render_context: &mut bevy::render::renderer::RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let bind_groups = &world.resource::<ProcessingBindGroup>().0;
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ProcessingPipeline>();
        let images = world.resource::<CameraTextures>();
        if !images.new_frame {
            info!("No new frame");
            return Ok(());
        }
        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());
        match self.state {
            ProcessingState::Loading => {}
            ProcessingState::Init => {
                let pipe = pipeline_cache
                    .get_compute_pipeline(pipeline.pipeline)
                    .unwrap();
                pass.set_bind_group(0, bind_groups, &[]);
                pass.set_pipeline(pipe);
                pass.dispatch_workgroups(
                    images.size.x as u32 / WORKGROUP_SIZE,
                    images.size.y as u32 / WORKGROUP_SIZE,
                    1,
                );
            }
        }
        Ok(())
    }
}
