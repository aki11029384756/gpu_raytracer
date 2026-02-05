use crate::my3d_lib::*;
use glam::Quat;
use glam::Vec3A as Vec3;


pub fn load_glb(path: &str) -> Vec<Mesh> {
    let mut meshes: Vec<Mesh> = vec![];

    // Import GLB
    let (gltf, buffers, _) = gltf::import(path).expect("Failed to load GLB from path");

    // Load global materials
    let mut global_materials: Vec<Material> = vec![];
    for mat in gltf.materials() {
        let pbr = mat.pbr_metallic_roughness();
        let base = pbr.base_color_factor();
        let albedo = Vec3::new(base[0] as f32, base[1] as f32, base[2] as f32);
        
        
        let emissive = mat.emissive_factor();
        let mut emission = Vec3::new(emissive[0] as f32, emissive[1] as f32, emissive[2] as f32);
        if let Some(strength) = mat.emissive_strength() {
            emission *= strength;
        }

        println!("Material: {:?}", mat.name());
        println!("  Albedo: {:?}", albedo);
        println!("  Emission: {:?}", emission);
        println!("  Roughness: {}", pbr.roughness_factor());


        let roughness = pbr.roughness_factor() as f32;

        global_materials.push(Material { albedo, emission, roughness });
    }
    if global_materials.is_empty() {
        global_materials.push(Material::default());
    }

    // Iterate nodes to apply transforms
    for node in gltf.nodes() {
        if let Some(mesh_gltf) = node.mesh() {
            // Get the node transform
            let transform = node.transform();

            // Decompose into TRS
            let (trs_translation, trs_rotation, trs_scale) = transform.decomposed();

            // Convert to glam types
            let position = Vec3::new(trs_translation[0], trs_translation[1], trs_translation[2]);
            let rotation = Quat::from_xyzw(trs_rotation[0], trs_rotation[1], trs_rotation[2], trs_rotation[3]);
            let scale    = Vec3::new(trs_scale[0], trs_scale[1], trs_scale[2]);


            for primitive in mesh_gltf.primitives() {
                let mut mesh = Mesh::default();
                mesh.position = position;
                mesh.scale = scale;
                mesh.rotation = rotation;

                // Copy global materials
                mesh.materials = global_materials.clone();

                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                // Positions
                let mut positions: Vec<Vec3> = Vec::new();
                if let Some(iter) = reader.read_positions() {
                    positions = iter
                        .map(|p| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32))
                        .collect();
                }
                mesh.vertices = positions.clone();

                // Normals
                let normals: Vec<Vec3> = if let Some(iter) = reader.read_normals() {
                    iter.map(|n| Vec3::new(n[0] as f32, n[1] as f32, n[2] as f32))
                        .collect()
                } else {
                    vec![Vec3::new(0.0, 1.0, 0.0); mesh.vertices.len()]
                };

                // Indices / Faces
                let material_idx = primitive.material().index().unwrap_or(0);

                if let Some(indices) = reader.read_indices() {
                    let indices: Vec<u32> = indices.into_u32().collect();

                    for tri in indices.chunks(3) {
                        if tri.len() < 3 { continue; }

                        let i0 = tri[0] as usize;
                        let i1 = tri[1] as usize;
                        let i2 = tri[2] as usize;

                        mesh.faces.push(Face {
                            indices: [i0, i1, i2],
                            normals: [normals[i0], normals[i1], normals[i2]],
                            material_idx,
                            edges: [Vec3::default(); 2],
                        });
                    }
                } else {
                    // Non-indexed fallback
                    for i in (0..mesh.vertices.len()).step_by(3) {
                        if i + 2 >= mesh.vertices.len() { break; }

                        mesh.faces.push(Face {
                            indices: [i, i + 1, i + 2],
                            normals: [normals[i], normals[i + 1], normals[i + 2]],
                            material_idx,
                            edges: [Vec3::default(); 2],
                        });
                    }
                }

                meshes.push(mesh);
            }
        }
        
    }

    meshes
}


// fn load_file(path: &str) -> (String) {
//     std::fs::read_to_string(path).expect("Failed to read file")
// }
//
//
// pub fn parse(path: &str) -> Mesh {
//     let mut mesh: Mesh = Mesh::default();
//
//     // Check if there is a material file, aka a .mtl file to use for material
//     let material_path = format!("{}{}", path.rsplit(".obj").last().unwrap(), &*".mtl".to_owned());
//     println!("Material path: {:?}", material_path);
//
//     let mut material_idx: usize = 0;
//     let mut materials: HashMap<String, usize> = HashMap::default();
//
//     if let Ok(text) = std::fs::read_to_string(&material_path) {
//         println!("Material loaded");
//
//         let mut current_name: String = String::new();
//         let mut current_material: Material = Material::default();
//
//         for line in text.lines() {
//             println!("{}", line);
//
//             if line.starts_with("newmtl ") {
//                 if !current_name.is_empty() {
//                     // Save the previous material
//                     materials.insert(current_name.clone(), material_idx);
//                     mesh.materials.push(current_material);
//                     material_idx += 1;
//                 }
//
//                 current_name = line.split_whitespace().last().unwrap().to_owned();
//                 current_material = Material::default();
//             } else if line.starts_with("Kd ") {
//                 let parts: Vec<&str> = line.split_whitespace().collect();
//
//                 current_material.albedo = Vec3::new(
//                     parts[1].parse().unwrap(),
//                     parts[2].parse().unwrap(),
//                     parts[3].parse().unwrap()
//                 );
//             } else if line.starts_with("Ke ") {
//                 // Emmision
//                 let parts: Vec<&str> = line.split_whitespace().collect();
//
//                 let emmision = Vec3::new( parts[1].parse().unwrap(), parts[2].parse().unwrap(), parts[3].parse().unwrap() );
//
//                 current_material.emission = emmision;
//
//             } else if line.starts_with("Ns ") {
//                 // Roughness type thing
//                 let parts: Vec<&str> = line.split_whitespace().collect();
//
//                 let ns: f32 = parts[1].parse().unwrap();
//
//                 let roughness: f32 = 1.0 - (ns / 1000.0).clamp(0.0, 1.0);
//
//                 current_material.roughness = roughness;
//             }
//         }
//
//         if !current_name.is_empty() {
//             materials.insert(current_name, material_idx);
//             mesh.materials.push(current_material);
//         }
//     }
//
//
//     let text = load_file(path);
//
//     let mut vertex_normals: Vec<Vec3> = vec![];
//
//     let mut curr_material_idx: usize = 0;
//
//     for line in text.lines() {
//         let parts = line.split(" ").collect::<Vec<&str>>();
//
//
//         if line.starts_with("v ") {
//             // Vertex declaration
//             let vert = Vec3::new(
//                 parts[1].parse().unwrap(),
//                 parts[2].parse().unwrap(),
//                 parts[3].parse().unwrap()
//             );
//             mesh.vertices.push(vert);
//
//         } else if line.starts_with("vn ") {
//             let normal = Vec3::new(
//                 parts[1].parse().unwrap(),
//                 parts[2].parse().unwrap(),
//                 parts[3].parse().unwrap()
//             );
//             vertex_normals.push(normal);
//         } else if line.starts_with("f ") {
//             let mut face: Face = Face::default();
//             face.material_idx = curr_material_idx;
//
//             for (i, part) in parts.iter().enumerate() {
//                 if i == 0 { continue; }
//
//                 let indices = part.split("/").collect::<Vec<&str>>();
//
//                 if i == 4 {
//                     mesh.faces.push(Face {
//                         indices: [
//                             face.indices[0],
//                             face.indices[2],
//                             indices[0].parse::<usize>().unwrap() - 1],
//                         normals: [
//                             face.normals[0],
//                             face.normals[2],
//                             vertex_normals[indices[2].parse::<usize>().unwrap() - 1]],
//                         material_idx: curr_material_idx,
//                         edges: [Vec3::default(); 2],
//                     });
//                     break;
//                 }
//
//                 face.indices[i - 1] = indices[0].parse::<usize>().unwrap() - 1;
//                 face.normals[i - 1] = vertex_normals[indices[2].parse::<usize>().unwrap() - 1];
//             }
//
//             mesh.faces.push(face);
//         } else if line.starts_with("usemtl ") {
//             curr_material_idx = materials.get(parts[1]).unwrap().clone();
//         }
//     }
//
//     if mesh.materials.len() == 0 {
//         println!("No materials found");
//         println!("Adding default material");
//
//         mesh.materials.push(Material::default());
//     }
//
//     mesh
// }