use bytemuck::bytes_of;
use nokhwa::{
    pixel_format::RgbAFormat,
    utils::{CameraIndex, RequestedFormat},
    *,
};
use std::mem::size_of;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, Buffer, BufferBinding, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePipeline, Device, Extent3d, ImageCopyTextureBase, Instance,
    PipelineLayoutDescriptor, Queue, ShaderStages, StorageTextureAccess, Texture, TextureDimension,
    TextureFormat, TextureUsages, TextureViewDescriptor, TextureViewDimension, include_wgsl,
};
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::*,
};

use futures::executor::block_on;
fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
}

struct App {
    window: Option<Window>,
    state: Option<RenderState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
    }
}

struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

struct RenderState {
    pub cam: Camera,
    pub device: Device,
    pub queue: Queue,
    pub diff_bind_group: BindGroup,
    pub diff_pipeline: ComputePipeline,
    pub ray_bind_group: BindGroup,
    pub ray_pipeline: ComputePipeline,
    pub voxel_info: VoxelInfo,
    pub counter: Buffer,
    pub hit_buffer: Buffer,
    pub readback_buffer: Buffer,
    pub counter_readback: Buffer,
    pub size: Extent3d,
}

struct VoxelInfo {
    center: Vec3,
    block_size: f32,
    n: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VoxelHit {
    pub voxel_id: u32, //x + y * N + z * N * N
    pub value: f32,
}

struct RaymarchUniforms {}
const MAX_RAYMARCH_STEPS: u32 = 64;
impl RenderState {
    async fn new(center: Vec3, block_size: f32, n: u32) -> Self {
        let index = CameraIndex::Index(0);
        let requested = RequestedFormat::new::<RgbAFormat>(
            utils::RequestedFormatType::AbsoluteHighestFrameRate,
        );
        let mut cam = Camera::new(index, requested).unwrap();

        let _ = cam.open_stream();

        let instance = Instance::new(Default::default());
        let adapter = instance.request_adapter(&Default::default()).await.unwrap();
        let (device, queue) = adapter
            .request_device(&Default::default(), None)
            .await
            .unwrap();
        let base_frame: Texture = cam
            .frame_texture::<RgbAFormat>(&device, &queue, Some("base_frame"))
            .unwrap();
        let size = base_frame.size();
        //Init rendering pipeline
        let _ = cam.stop_stream();
        let previous_frame = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("previous_frame"),
            size: size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let diff_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("diff_texture"),
            size: size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("uniform_buffer"),
            size: std::mem::size_of::<RaymarchUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let width = size.width;
        let height = size.height;

        let max_hits = (width * height * MAX_RAYMARCH_STEPS) as u64;
        let hit_buffer_size = max_hits * std::mem::size_of::<VoxelHit>() as u64;

        let hit_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("voxel_buffer"),
            size: hit_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let counter = device.create_buffer(&BufferDescriptor {
            label: Some("counter"),
            size: 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let readback_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("readback_buffer"),
            size: hit_buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let counter_readback = device.create_buffer(&BufferDescriptor {
            label: Some("counter_readback"),
            size: 4,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_texture(
            ImageCopyTextureBase {
                texture: &base_frame,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            ImageCopyTextureBase {
                texture: &previous_frame,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            size,
        );
        queue.submit(std::iter::once(encoder.finish()));

        let texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadOnly,
                            format: TextureFormat::Rgba8Unorm,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadOnly,
                            format: TextureFormat::Rgba8Unorm,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadWrite,
                            format: TextureFormat::R8Unorm,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let raymarch_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("raymarch_bind_group_layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadOnly,
                            format: TextureFormat::R8Unorm,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("diff_pipeline_layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let diff_shader = device.create_shader_module(include_wgsl!("diff.wgsl"));
        let ray_shader = device.create_shader_module(include_wgsl!("ray.wgsl"));

        let diff_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("diff_compute"),
            layout: Some(&pipeline_layout),
            module: &diff_shader,
            entry_point: None, //change
            compilation_options: Default::default(),
            cache: Default::default(),
        });

        let raymarch_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("raymarch_pipeline_layout"),
            bind_group_layouts: &[&raymarch_bind_group_layout],
            push_constant_ranges: &[],
        });

        let ray_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ray_compute"),
            layout: Some(&raymarch_pipeline_layout),
            module: &ray_shader,
            entry_point: None,
            compilation_options: Default::default(),
            cache: Default::default(),
        });

        let base_texture_view = base_frame.create_view(&TextureViewDescriptor::default());
        let prev_texture_view = previous_frame.create_view(&TextureViewDescriptor::default());
        let diff_texture_view = diff_texture.create_view(&TextureViewDescriptor::default());
        let diff_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &texture_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&base_texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&prev_texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&diff_texture_view),
                },
            ],
        });

        let ray_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &raymarch_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&diff_texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &hit_buffer,
                        offset: 0, // i'm guessing we adjust this to be set to the counter, probably also want to set the size
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &counter,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let voxel_info = VoxelInfo {
            center,
            block_size,
            n,
        };

        RenderState {
            cam,
            device,
            queue,
            diff_bind_group,
            diff_pipeline,
            ray_bind_group,
            ray_pipeline,
            voxel_info,
            counter,
            hit_buffer,
            size,
            readback_buffer,
            counter_readback,
        }
    }
    pub fn frame(&self) {
        //copy new frame from camera to buffer here
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });
        let wg_x = self.size.width / 8;
        let wg_y = self.size.height / 8;
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        cpass.set_pipeline(&self.diff_pipeline);
        cpass.set_bind_group(0, &self.diff_bind_group, &[]);
        cpass.dispatch_workgroups(wg_x, wg_y, 1);

        //uhhhh prepare the bindgroup here ig?
        self.queue.write_buffer(&self.counter, 0, bytes_of(&0u32));

        cpass.set_pipeline(&self.ray_pipeline);
        cpass.set_bind_group(0, &self.ray_bind_group, &[]);
        cpass.dispatch_workgroups(wg_x, wg_y, 1);
        encoder.copy_buffer_to_buffer(
            &self.counter,
            0,
            &self.counter_readback,
            0,
            self.counter.size(),
        );
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn readback(&self) -> Vec<VoxelHit> {
        let (tx, rx) = flume::unbounded();
        self.counter_readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
        self.device.poll(wgpu::MaintainBase::Wait);
        rx.recv().unwrap().unwrap();
        let out_counter = self.counter_readback.slice(..).get_mapped_range();
        let count: u32 = bytemuck::cast_slice(&out_counter)[0];

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("readback_encoder"),
            });
        let copy_size = (count * size_of::<VoxelHit>() as u32) as u64;
        encoder.copy_buffer_to_buffer(&self.hit_buffer, 0, &self.readback_buffer, 0, copy_size);
        self.queue.submit(std::iter::once(encoder.finish()));
        let (tx, rx) = flume::unbounded();
        self.readback_buffer
            .slice(..copy_size)
            .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
        self.device.poll(wgpu::MaintainBase::Wait);
        rx.recv().unwrap().unwrap();
        let hits_bytes = self.readback_buffer.slice(..copy_size).get_mapped_range();
        let hits: Vec<VoxelHit> = bytemuck::cast_slice(&hits_bytes).to_vec();
        //send hits or w/e
        hits
    }
}
