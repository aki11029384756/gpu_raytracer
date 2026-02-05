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
    _pad3: vec3<f32>,
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
    //let view_proj = camera.projection * camera.view;
    //out.clip_position = view_proj * vec4(in.position, 1.0);

    return out;
}
