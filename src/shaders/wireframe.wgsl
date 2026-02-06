struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

struct Camera {
    position: vec3<f32>,
    _pad1: f32,
    forward: vec3<f32>,
    _pad2: f32,
    right: vec3<f32>,
    _pad3: f32,
    up: vec3<f32>,
    _pad4: f32,
    focal_distance: f32,
    aperture_radius: f32,
    aspect_ratio: f32,
    frame: u32,
};

@group(0) @binding(0) var<uniform> camera: Camera;


@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform to camera space
    let relative_pos = in.position - camera.position;

    // Project onto camera plane
    let x = dot(relative_pos, camera.right)/(16.0/9.0);
    let y = -dot(relative_pos, camera.up);
    let z = dot(relative_pos, camera.forward);

    // Perspective divide
    out.clip_position = vec4<f32>(x, y, z, z);

    if abs(z-camera.focal_distance) < 0.05 {
        out.clip_position.z -= 0.1;
        out.clip_position.w -= 0.1;
    }

    return out;
}


@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0); // white
}
