use nokhwa::{
    pixel_format::RgbAFormat,
    utils::{CameraIndex, RequestedFormat},
    *,
};
use wgpu::{
    Adapter, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, Buffer, BufferBinding, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePipeline, Device, ImageCopyTextureBase, Instance,
    PipelineCompilationOptions, PipelineLayoutDescriptor, Queue, RenderPipelineDescriptor,
    ShaderStages, StorageTextureAccess, Texture, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDescriptor, TextureViewDimension, VertexState, include_wgsl, util::DeviceExt,
};

fn main() {
    println!("Hello, world!");
}

struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

struct RenderState {
    pub cam: Camera,
    pub fps: u32,
    pub device: Device,
    pub queue: Queue,
    pub diff_bind_group: BindGroup,
    pub diff_pipeline: ComputePipeline,
    pub ray_bind_group: BindGroup,
    pub ray_pipeline: ComputePipeline,
    pub voxel_info: VoxelInfo,
}

struct VoxelInfo {
    center: Vec3,
    block_size: f32,
    n: u32,
}

struct VoxelHit {
    pub voxel_id: u32, //x + y * N + z * N * N
    pub value: f32,
}

impl RenderState {
    async fn new(center: Vec3, block_size: f32, n: u32) -> Self {
        let index = CameraIndex::Index(0);
        let requested = RequestedFormat::new::<RgbAFormat>(
            utils::RequestedFormatType::AbsoluteHighestFrameRate,
        );
        let mut cam = Camera::new(index, requested).unwrap();
        let fps = cam.frame_rate();

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
        let copy_base = ImageCopyTextureBase {
            texture: &base_frame,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        let copy_previous = ImageCopyTextureBase {
            texture: &previous_frame,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform_buffer"),
            contents: &[],
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let voxel_hit_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("voxel_buffer"),
            contents: &[],
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        });
        let counter = device.create_buffer(&BufferDescriptor {
            label: Some("counter"),
            size: 8,
            usage: BufferUsages::STORAGE | BufferUsages::MAP_READ | BufferUsages::MAP_WRITE,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_texture(copy_base, copy_previous, size);
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
                            has_dynamic_offset: true,
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

        let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

        let diff_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("diff_compute"),
            layout: Some(&pipeline_layout),
            module: &shader,
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
            layout: Some(&pipeline_layout),
            module: &shader,
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
                        buffer: &voxel_hit_buffer,
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

        RenderState {
            cam,
            fps,
            device,
            queue,
            diff_bind_group,
            diff_pipeline,
            ray_bind_group,
            ray_pipeline,
        }
    }
}
