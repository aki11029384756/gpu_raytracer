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

struct SceneInfo {
    num_faces: u32,
    num_materials: u32,
    _pad: vec2<u32>,
};

struct Vertex {
    position: vec3<f32>,
    _pad: f32,
};

struct Material {
    albedo: vec3<f32>,
    roughness: f32,

    emission: vec3<f32>,
    _pad: f32,
};

struct Face {
    indices: vec3<u32>,
    material_idx: u32,

    normal0: vec3<f32>,
    _pad1: f32,

    normal1: vec3<f32>,
    _pad2: f32,

    normal2: vec3<f32>,
    _pad3: f32,
};




@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> scene_info: SceneInfo;
@group(0) @binding(2) var<storage, read> vertices: array<Vertex>;
@group(0) @binding(3) var<storage, read> faces: array<Face>;
@group(0) @binding(4) var<storage, read> materials: array<Material>;
@group(0) @binding(5) var render_texture: texture_storage_2d<rgba32float, write>;
@group(0) @binding(6) var accumulation_input: texture_storage_2d<rgba32float, read>;
@group(0) @binding(7) var accumulation_output: texture_storage_2d<rgba32float, write>;


@compute @workgroup_size(8, 8, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
) {
    let pixel = vec2<i32>(gid.xy);

    let color = vec4<f32>(1.0, 0.0, 0.0, 1.0); // red

    textureStore(accumulation_output, pixel, color);
}