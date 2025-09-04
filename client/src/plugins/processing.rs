use std::borrow::Cow;

use bevy::{
    asset::RenderAssetUsages,
    math::ops::tanh,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::ExtractResourcePlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_graph::{self, Node, RenderGraph, RenderLabel},
        render_resource::{
            BindGroupLayoutEntries, PipelineCache, ShaderStages,
            binding_types::{texture_storage_2d, texture_storage_3d, uniform_buffer},
            *,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};

use crate::prelude::*;

pub struct ImageProcessingPlugin;

const SHADER_ASSET_PATH: &str = "shaders/processing.wgsl";
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

const TEST_VOXEL_SIZE: u32 = 10;
fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let size = Extent3d {
        width: TEST_VOXEL_SIZE,
        height: TEST_VOXEL_SIZE,
        depth_or_array_layers: TEST_VOXEL_SIZE,
    };

    let mut image = Image::new_uninit(
        size,
        TextureDimension::D3,
        TextureFormat::R32Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC | TextureUsages::STORAGE_BINDING;
    let image = images.add(image);

    commands
        .spawn(Readback::texture(image.clone()))
        .observe(|event: On<ReadbackComplete>| {
            let diff: Vec<f32> = event.to_shader_type();
            info!("Voxel Grid: {:?}", diff);
        });
    commands.insert_resource(VoxelGridTexture(image));
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
    let raymarch_bind_group_layout = render_device.create_bind_group_layout(
        "Raymarch",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::ReadOnly),
                uniform_buffer::<RaymarchUniforms>(false),
                texture_storage_3d(TextureFormat::R32Float, StorageTextureAccess::ReadWrite),
            ),
        ),
    );
    let shader = asset_server.load(SHADER_ASSET_PATH);
    let diff_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        layout: vec![texture_bind_group_layout.clone()],
        shader: shader.clone(),
        entry_point: Some(Cow::from("diff")),
        zero_initialize_workgroup_memory: true,
        ..default()
    });
    let raymarch_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        layout: vec![raymarch_bind_group_layout.clone()],
        shader: shader.clone(),
        entry_point: Some(Cow::from("raymarch")),
        zero_initialize_workgroup_memory: true,
        ..default()
    });

    commands.insert_resource(ProcessingPipeline {
        texture_bind_group_layout,
        diff_pipeline,
        raymarch_bind_group_layout,
        raymarch_pipeline,
    });
}

#[derive(ShaderType)]
struct RaymarchUniforms {
    camera_pos: Vec3,
    camera_rotation: Mat3,
    screen_size: Vec2,
    grid_center: Vec3,
    voxel_n: i32,
    voxel_size: f32,
    focal_length: f32,
    changed_threshold: f32,
}
fn prepare_bind_group(
    mut commands: Commands,
    pipeline: Res<ProcessingPipeline>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    camera_images: Res<CameraTextures>,
    display_texture: Res<DisplayTexture>,
    voxel_grid: Res<VoxelGridTexture>,
    voxel_info: Res<VoxelInfo>,
    frame_info: Res<FrameInfo>,
    render_device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let current = gpu_images.get(&camera_images.current).unwrap();
    let prev = gpu_images.get(&camera_images.prev).unwrap();
    let target = gpu_images.get(&display_texture.handle).unwrap();
    let voxel = gpu_images.get(&voxel_grid.0).unwrap();

    let bind_group_0 = render_device.create_bind_group(
        None,
        &pipeline.texture_bind_group_layout,
        &BindGroupEntries::sequential((
            &current.texture_view,
            &prev.texture_view,
            &target.texture_view,
        )),
    );
    let focal_length = (current.size_2d().x as f32 * 0.5) / (frame_info.fov * 0.5).tan();
    let screen_size = target.size_2d();
    let size = Vec2 {
        x: screen_size.x as f32,
        y: screen_size.y as f32,
    };
    let mut uniform_buffer = UniformBuffer::from(RaymarchUniforms {
        camera_pos: frame_info.camera_position,
        camera_rotation: Mat3::from_euler(
            EulerRot::YXZ,
            frame_info.yaw,
            frame_info.pitch,
            frame_info.roll,
        ),
        screen_size: size,
        grid_center: voxel_info.grid_center,
        voxel_n: voxel_info.n as i32,
        voxel_size: voxel_info.voxel_size,
        focal_length,
        changed_threshold: 0.05,
    });
    uniform_buffer.write_buffer(&render_device, &queue);
    let bind_group_1 = render_device.create_bind_group(
        None,
        &pipeline.raymarch_bind_group_layout,
        &BindGroupEntries::sequential((&target.texture_view, &uniform_buffer, &voxel.texture_view)),
    );
    commands.insert_resource(ProcessingBindGroup([bind_group_0, bind_group_1]));
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
                match pipeline_cache.get_compute_pipeline_state(pipeline.diff_pipeline) {
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
        let bind_group = &world.resource::<ProcessingBindGroup>().0;
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
                    .get_compute_pipeline(pipeline.diff_pipeline)
                    .unwrap();
                pass.set_bind_group(0, &bind_group[0], &[]);
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
