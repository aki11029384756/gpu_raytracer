use glam::{quat, Quat};
use glam::Vec3A as Vec3;


#[derive(Clone, Copy)]
pub struct Face {
    pub indices: [usize; 3],
    pub normals: [Vec3; 3],
    pub material_idx: usize,
    pub edges: [Vec3; 2],
}

impl Default for Face {
    fn default() -> Self {
        Self {
            indices: [0; 3],
            normals: [Vec3::default(); 3],
            material_idx: 0,
            edges: [Vec3::default(); 2],
        }
    }
}


#[derive(Copy, Clone)]
pub struct Material {
    /// A materials ability to reflect light
    pub albedo: Vec3,

    /// The emitted light from the material
    pub emission: Vec3,

    /// How much reflected rays are scattered
    pub roughness: f32,
}

impl Default for Material {
    fn default() -> Material {
        Material {
            albedo: Vec3::new(1.0, 1.0, 1.0),
            emission: Vec3::default(),
            roughness: 1.0,
        }
    }
}


#[derive(Clone)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Face>,
    pub scale: Vec3,
    pub position: Vec3,
    pub rotation: Quat,
    pub materials: Vec<Material>,
}

impl Default for Mesh {
    fn default() -> Mesh {
        Mesh {
            vertices: Vec::default(),
            faces: Vec::default(),
            scale: Vec3::default(),
            position: Vec3::default(),
            rotation: Quat::default(),
            materials: Vec::default(),
        }
    }
}


pub struct World {
    pub meshes: Vec<Mesh>,
    pub baked_meshes: Vec<Mesh>,
}


impl World {
    fn bake_mesh(&self, mesh: &Mesh) -> Mesh {
        let mut baked = mesh.clone();

        // Apply scale → rotation → position to all vertices
        for vert in &mut baked.vertices {
            *vert *= baked.scale;         // scale
            *vert = baked.rotation * *vert; // rotate via Quat
            *vert += baked.position;      // translate
        }

        // Transform normals (rotate only, then normalize)
        for face in &mut baked.faces {
            for normal in &mut face.normals {
                *normal = baked.rotation * *normal;
                *normal = normal.normalize();
            }
        }

        // Recompute edges
        for face in &mut baked.faces {
            face.edges[0] = baked.vertices[face.indices[1]] - baked.vertices[face.indices[0]];
            face.edges[1] = baked.vertices[face.indices[2]] - baked.vertices[face.indices[0]];
        }

        baked
    }

    pub fn bake_meshes(&mut self) {
        self.baked_meshes = vec![];

        for mesh in &self.meshes {
            self.baked_meshes.push(self.bake_mesh(mesh));
        }
    }
}


pub struct RayHit {
    pub material: Material,
    pub distance: f32,
    pub position: Vec3,
    pub direction: Vec3,
    pub reflected_dir: Vec3,
}

impl Default for RayHit {
    fn default() -> RayHit {
        RayHit {
            material: Material::default(),
            distance: 0.0,
            position: Vec3::default(),
            direction: Vec3::default(),
            reflected_dir: Vec3::default(),
        }
    }
}
