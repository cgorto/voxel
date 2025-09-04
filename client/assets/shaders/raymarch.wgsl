@group(1) @binding(0) var difference: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(1) var<uniform> u: RaymarchUniforms;
@group(1) @binding(2) var voxel_grid: texture_storage_3d<r32float, read_write>;

struct RaymarchUniforms {
    camera_pos: vec3<f32>,
    camera_rotation: mat3x3<f32>,
    screen_size: vec2<f32>,
    grid_center: vec3<f32>,
    voxel_n: i32,
    voxel_size: f32,
    focal_length: f32,
    changed_threshold: f32,
}

fn safe_div(a: f32, b:f32) -> f32 {
    if (abs(b) < 1e-12) {
        return 1e30;
    }
    return a / b;
}


fn cast_ray_into_grid(
    camera_pos: vec3<f32>,
    dir: vec3<f32>,
    voxel_n: i32,
    voxel_size: f32,
    grid_center: vec3<f32>
    diff: f32
) {
    var step_count = 0;
    let half_size = 0.5 * (f32(u.voxel_n) * u.voxel_size);
    let half_grid = vec3<f32>(half_size);
    let grid_min = grid_center - half_grid;
    let grid_max = grid_center + half_grid;

    var t_min = 0.0;
    var t_max = 1e30;

    for (var i = 0; i < 3; i++) {
        let origin = camera_pos[i];
        let d = u.look_dir[i];
        let mn = grid_min[i];
        let mx = grid_max[i];

        if (abs(d) < 1e-12 && (origin < mn || origin > mx)) {
            return;
        }
        
        let t1 = (mn - origin) / d;
        let t2 = (mx - origin) / d;
        let t_near = min(t1,t2);
        let t_far = max(t1,t2);
        t_min = max(t_min, t_near);
        t_max = min(t_max, t_far);

        if (t_min > t_max) {
            return;
        }

    }
    if (t_min < 0.0) {
        t_min = 0.0;
    }
    let start_world = camera_pos + t_min * dir;
    let f = (start_world - grid_min) / voxel_size;

    var ix = i32(f.x);
    var iy = i32(f.y);
    var iz = i32(f.z);

    if (ix < 0 || ix >= voxel_n || iy < 0 || iy >= voxel_n || iz < 0 || iz >= voxel_n) {
        return
    }

    let step_x = select(-1,1, dir.x >= 0.0);
    let step_y = select(-1,1, dir.y >= 0.0);
    let step_z = select(-1,1, dir.z >= 0.0);

    let nx_x = ix + select(0,1, step_x > 0);
    let nx_y = iy + select(0,1, step_y > 0);
    let nx_z = iz + select(0,1, step_z > 0);

    let next_bx = grid_min.x + f32(nx_x) * voxel_size;
    let next_by = grid_min.y + f32(nx_y) * voxel_size;
    let next_bz = grid_min.z + f32(nx_z) * voxel_size;

    var t_max_x = safe_div(next_bx - camera_pos.x, dir.x);
    var t_max_y = safe_div(next_by - camera_pos.y, dir.y);
    var t_max_z = safe_div(next_bz - camera_pos.z, dir.z);

    let t_delta_x = safe_div(voxel_size, abs(dir.x));
    let t_delta_y = safe_div(voxel_size, abs(dir.y));
    let t_delta_z = safe_div(voxel_size, abs(dir.z));

    var t_current = t_min;

    while(t_current <=t_max && step_count < 64) {
        
        let voxel_coord = vec3<i32>(ix,iy,iz);
        let current_val = textureLoad(voxel_grid, voxel_coord).r;
        textureStore(voxel_grid,voxel_coord, vec4<f32>(current_val+val, 0.0,0.0,1.0));

        if (t_max_x < t_max_y && t_max_x < t_max_z) {
            ix += step_x;
            t_current = t_max_x;
            t_max_x += t_delta_x;
        } else if ( t_max_y < t_max_z) {
            iy += step_y;
            t_current = t_max_y;
            t_max_y += t_delta_y;
        } else {
            iz += step_z;
            t_max_z += t_delta_z;
        }

        step_count ++;

        if (ix < 0 || ix >= voxel_n || iy < 0 || iy >= voxel_n || iz < 0 || iz >= voxel_n) {
            break;
        }
    }
}

@compute @workgroup_size(8,8,1)
fn raymarch(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let pixel_coord = vec2<i32>(invocation_id.xy);
    let screen_size = vec2<i32>(u.screen_size);

    if (pixel_coord.x >= screen_size.x || pixel_coord.y >= screen_size.y) {
        return;
    }
    let diff = textureLoad(difference, pixel_coord).r;
    if (diff <= u.changed_threshold) {
        return;
    }
    let u = f32(pixel_coord.x);
    let v = f32(pixel_coord.y);
    let width = f32(screen_size.x);
    let height = f32(screen_size.y);

    let x = u - 0.5 * width;
    let y = -(v - 0.5 * height);
    let z = -u.focal_length;

    var ray_cam = vec3<f32>(x,y,z);
    ray_cam = normalize(ray_cam);

    let ray_world = normalize(u.camera_rotation * ray_cam);
    cast_ray_into_grid(u.camera_pos, ray_cam, u.voxel_n, u.voxel_size, u.grid_center, diff);

}
