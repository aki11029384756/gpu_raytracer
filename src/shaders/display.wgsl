@group(0) @binding(0) var render_texture: texture_2d<f32>;




const resolution = vec2<f32>(1920.0, 1080.0);

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Hardcoded positions for a fullscreen quad
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),  // Bottom-left
        vec2<f32>(1.0, -1.0),   // Bottom-right
        vec2<f32>(-1.0, 1.0),   // Top-left
        vec2<f32>(-1.0, 1.0),   // Top-left (again)
        vec2<f32>(1.0, -1.0),   // Bottom-right (again)
        vec2<f32>(1.0, 1.0),    // Top-right
    );

    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}


@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let coords = vec2<i32>(position.xy);
    let color = textureLoad(render_texture, coords, 0);


    //let gamma = 2.2;
    //let corrected = pow(color.rgb, vec3<f32>(1.0 / gamma));
    let corrected = color.rgb;
    return vec4<f32>(corrected, 1.0);
}