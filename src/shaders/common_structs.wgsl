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
