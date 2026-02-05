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


struct HitInfo {
    hit: bool,
    distance: f32,
    position: vec3<f32>,
    normal: vec3<f32>,
    material_idx: u32,
}



@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> scene_info: SceneInfo;
@group(0) @binding(2) var<storage, read> vertices: array<Vertex>;
@group(0) @binding(3) var<storage, read> faces: array<Face>;
@group(0) @binding(4) var<storage, read> materials: array<Material>;
@group(0) @binding(5) var render_texture: texture_storage_2d<rgba32float, write>;
@group(0) @binding(6) var accumulation_input: texture_storage_2d<rgba32float, read>;
@group(0) @binding(7) var accumulation_output: texture_storage_2d<rgba32float, write>;
@group(0) @binding(8) var<uniform> rand_seed: u32;
@group(0) @binding(9) var<uniform> sample_count: u32;

const resolution = vec2<i32>(1920, 1080);



@compute @workgroup_size(8, 8, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
) {
    let pixel_i = vec2<i32>(gid.xy);
    let pixel_f = vec2<f32>(gid.xy);

    if (gid.x >= u32(resolution.x) || gid.y >= u32(resolution.y)) {
        return;
    }


    var color = vec3<f32>(0.0, 0.0, 0.0);
    var transmition = vec3<f32>(1.0, 1.0, 1.0); // When we hit an object we reduce transmition by its albedo

    let aspect_ratio = f32(resolution.x) / f32(resolution.y);
    var screen_pos = vec2<f32>((pixel_f - vec2<f32>(resolution)/2.)/vec2<f32>(resolution));
    screen_pos.x *= aspect_ratio;

    var pos = camera.position;

    var dir = normalize(vec3<f32>(camera.forward + (camera.right * screen_pos.x) + (camera.up * screen_pos.y)));


    let recursions: u32 = 4;

    for (var rec_idx = 0u; rec_idx < recursions; rec_idx = rec_idx + 1) {
        // First get the hit triangle
        var hit = HitInfo(
            false,
            1000.0,
            vec3<f32>(0.0),
            vec3<f32>(0.0),
            0u
        );

        for (var i = 0u; i < scene_info.num_faces; i = i + 1) {
            let face = faces[i];

            let v0 = vertices[face.indices.x].position;
            let v1 = vertices[face.indices.y].position;
            let v2 = vertices[face.indices.z].position;
            

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;

            let normal = face.normal0;

            let dir_dot_norm = dot(dir, normal);
            if dir_dot_norm > 0.01 { continue; }; // if we are parralell to or behind the face

            let dist = dot(v0 - pos, normal) / dir_dot_norm;

            if dist > hit.distance { continue; };

            let hit_pos = pos + dir * dist;

            let h = cross(dir, edge2);
            let a = dot(edge1, h);

            if abs(a) < 0.001 { continue; }

            let f = 1.0 / a;
            let s = pos - v0;
            let u = f * (dot(s, h));

            if u < 0.0 || u > 1.0 { continue; }

            let q = cross(s, edge1);
            let v = f * (dot(dir, q));

            if v < 0.0 || u + v > 1.0 { continue; }


            hit.distance = dist;
            hit.hit = true;
            hit.material_idx = face.material_idx;
            hit.normal = normal;
            hit.position = hit_pos;
        }

        if hit.hit {
            let material = materials[hit.material_idx];

            color += vec3<f32>(transmition * material.emission);
            transmition = transmition * material.albedo;

            dir = dir - 2.0 * dot(dir, hit.normal) * hit.normal;

            // apply some randomness for roughness
            if material.roughness > 0. {
                var rand_dir = vec3<f32>(
                    hash(u32(abs(dir.x) * 172342) ^ rand_seed * 84321 + sample_count * 19) - 0.5,
                    hash(u32(abs(dir.y) * 72345) ^ rand_seed * 91342 + sample_count * 3 ) - 0.5,
                    hash(u32(abs(dir.z) * 9234521) ^ rand_seed * 382994 + sample_count * 9) - 0.5
                ) * material.roughness * 2;

                dir = normalize(dir + rand_dir);
            }

            pos = hit.position;
        } else {
            break;
        }
    }

    let old_color = textureLoad(accumulation_input, pixel_i);
    let store_color = (old_color + vec4<f32>(color, 1.0));
    textureStore(accumulation_output, pixel_i, store_color);


    // Lastly we write the accumulated to render_texture
    textureStore(render_texture, pixel_i, store_color / f32(sample_count+1));
}




fn hash(seed: u32) -> f32 {
    var state = seed * 747796405u + 2891336453u;
    var word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return f32((word >> 22u) ^ word) / 4294967295.0;
}