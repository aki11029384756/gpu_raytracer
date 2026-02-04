@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> scene_info: SceneInfo;
@group(0) @binding(2) var<storage, read> vertices: array<Vertex>;
@group(0) @binding(3) var<storage, read> faces: array<Face>;
@group(0) @binding(4) var<storage, read> materials: array<Material>;
@group(0) @binding(5) var render_texture: texture_storage_2d<rgba32float, write>;
@group(0) @binding(6) var accumulation_input: texture_storage_2d<rgba32float, read>;
@group(0) @binding(7) var accumulation_output: texture_storage_2d<rgba32float, write>;


@compute @workgroup_size(8, 8, 1)
fn main() {

}