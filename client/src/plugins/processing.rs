use std::borrow::Cow;

use bevy::{
    asset::RenderAssetUsages,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::ExtractResourcePlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_graph::{self, Node, RenderGraph, RenderLabel},
        render_resource::{
            BindGroupLayoutEntries, PipelineCache, ShaderStages,
            binding_types::{storage_buffer, texture_storage_2d, uniform_buffer},
            *,
        },
        renderer::{RenderDevice, RenderQueue},
        storage::{GpuShaderStorageBuffer, ShaderStorageBuffer},
        texture::GpuImage,
    },
};

use crate::module_bindings::voxel_type::Voxel;
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
            ExtractResourcePlugin::<VoxelHitBuffer>::default(),
        ))
        .add_systems(Startup, setup)
        .add_event::<VoxelHitEvent>();
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
const MAX_RAYMARCH_STEPS: u32 = 64;
const TEST_VOXEL_SIZE: u32 = 10;
fn setup(mut commands: Commands, mut buffers: ResMut<Assets<ShaderStorageBuffer>>) {
    let buffer_size = 1000 * 1000 * 64; //placeholder, you'd take image size for worst case scenario ()
    let buffer = vec![VoxelHit::default(); buffer_size];
    let mut buffer = ShaderStorageBuffer::from(buffer);
    let mut counter = ShaderStorageBuffer::from(0u32);
    counter.buffer_description.usage |= BufferUsages::COPY_SRC;
    buffer.buffer_description.usage |= BufferUsages::COPY_SRC;
    let buffer = buffers.add(buffer);
    let counter = buffers.add(counter);
    commands
        .spawn(Readback::buffer(buffer.clone()))
        .observe(on_voxel_readback);
    //add readback here!
    commands.insert_resource(VoxelHitBuffer([buffer, counter]));
    info!("set up");
}

pub fn on_voxel_readback(trigger: On<ReadbackComplete>, mut events: EventWriter<VoxelHitEvent>) {
    let hits: Vec<VoxelHit> = trigger.event().to_shader_type();
    let mut counter = 0u32;
    for hit in hits {
        if hit.value < 0.0 || hit.pos_idx == u32::MAX {
            info!("count: {}", counter );
            return;
        }
        if hit.value > f32::EPSILON {
            let pos = hit.voxel(TEST_VOXEL_SIZE);
            let voxel = Voxel {
                x: pos.x,
                y: pos.y,
                z: pos.z,
            };
            info!("value: {}", hit.value);
            let value = hit.value;
            events.write(VoxelHitEvent { voxel, value });
            counter += 1;
        }
    }
    info!("total count: {}", counter);
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
                texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::ReadWrite),
            ),
        ),
    );
    let raymarch_bind_group_layout = render_device.create_bind_group_layout(
        "Raymarch",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::ReadWrite),
                uniform_buffer::<RaymarchUniforms>(false),
                storage_buffer::<u32>(false),
                storage_buffer::<VoxelHit>(false),
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
        layout: vec![
            texture_bind_group_layout.clone(),
            raymarch_bind_group_layout.clone(),
        ],
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
    voxel_info: Res<VoxelInfo>,
    frame_info: Res<FrameInfo>,
    render_device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    buffer: Res<VoxelHitBuffer>,
    buffers: Res<RenderAssets<GpuShaderStorageBuffer>>,
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
    // let hit_buffer = buffers.get(&buffer.0[0]).unwrap();
    let mut counter = StorageBuffer::from(0u32);
    counter.write_buffer(&render_device, &queue);

    let buffer_size = 1000 * 1000 * 64; //placeholder, you'd take image size for worst case scenario ()
    let hit_buffer_array = vec![VoxelHit::default(); buffer_size];
    let mut hit_buffer = StorageBuffer::from(hit_buffer_array);
    hit_buffer.write_buffer(&render_device, &queue);

    uniform_buffer.write_buffer(&render_device, &queue);
    let bind_group_1 = render_device.create_bind_group(
        None,
        &pipeline.raymarch_bind_group_layout,
        &BindGroupEntries::sequential((
            &target.texture_view,
            &uniform_buffer,
            &counter,
            &hit_buffer,
            // hit_buffer.buffer.as_entire_buffer_binding(),
        )),
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
                    CachedPipelineState::Ok(_) => {
                        match pipeline_cache.get_compute_pipeline_state(pipeline.raymarch_pipeline)
                        {
                            CachedPipelineState::Ok(_) => self.state = ProcessingState::Init,
                            _ => {}
                        }
                    }
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
                let pipe2 = pipeline_cache
                    .get_compute_pipeline(pipeline.raymarch_pipeline)
                    .unwrap();
                pass.set_bind_group(0, &bind_group[0], &[]);
                pass.set_bind_group(1, &bind_group[1], &[]);
                pass.set_pipeline(pipe);
                pass.dispatch_workgroups(
                    images.size.x as u32 / WORKGROUP_SIZE,
                    images.size.y as u32 / WORKGROUP_SIZE,
                    1,
                );
                pass.set_pipeline(pipe2);

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
