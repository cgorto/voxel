
@group(0) @binding(0) var current: texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(1) var previous: texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(2) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8,8,1)
fn diff(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let location = vec2<i32>(i32(invocation_id.x), i32(invocation_id.y));
    let current_value = textureLoad(current, location);
    let previous_value = textureLoad(previous, location);
    let d = abs(to_grayscale(previous_value) - to_grayscale(current_value));
    var color = vec4<f32>(d,d,d,1.0);
    if d == 0.0 {
        color = vec4<f32>(0.0,1.0,0.0,1.0);
    }
    if d >= 0.5 {
       color = vec4<f32>(1.0,0.0,0.0,1.0); 
    }
    // let color = vec4<f32>(d,d,d,1.0);
    textureStore(output, location, color);
}

fn to_grayscale(color: vec4<f32>) -> f32 {
    return dot(color.rgb, vec3<f32>(0.299,0.587,0.114));
}
