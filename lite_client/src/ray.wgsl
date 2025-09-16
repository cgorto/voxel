@group(0) @binding(0) var difference: texture_storage_2d<r8unorm, read>;
@group(0) @binding(1) var<uniform> u: RaymarchUniforms;
@group(0) @binding(2) var<storage, read_write> hits: Hits;
@group(0) @binding(3) var<storage, read_write> counter: atomic<u32>,

struct RaymarchUniforms {
    camera_pos: vec3<f32>,
    camera_rotation: mat3x3<f32>,
    screen_size: vec2<f32>,
    grid_center: vec3<f32>,
    voxel_n: u32,
    voxel_size: f32,
    focal_length: f32,
    changed_threshold: f32,
}

struct VoxelHit {
    voxel_id: u32,
    value: f32,
}

struct Hits {
    items: array<VoxelHit>,
}

@compute @workgroup_size(8,8,1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let pixel_coord = vec2<i32>(invocation_id.xy);
    let screen_size = vec2<i32>(u.screen_size);
    
    if (pixel_coord.x >= screen_size.x || pixel_coord.y >= screen_size.y) {
        return;
    }
    let diff = textureLoad(difference, pixel_coord).r;
    if (diff <= u.changed_threshold) {
        return;
    }
    let uc = f32(pixel_coord.x);
    let v = f32(pixel_coord.y);
    let width = f32(screen_size.x);
    let height = f32(screen_size.y);

    let x = uc - 0.5 * width;
    let y = -(v - 0.5 * height);
    let z = -u.focal_length;

    var ray_cam = vec3<f32>(x,y,z);
    ray_cam = normalize(ray_cam);

    let ray_world = normalize(u.camera_rotation * ray_cam);
    cast_ray_into_grid(u.camera_pos, ray_world, u.voxel_n, u.voxel_size, u.grid_center, diff);
}
