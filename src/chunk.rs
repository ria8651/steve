use super::{IVec3, AO, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use noise::{NoiseFn, Perlin};
use rand::{rngs::ThreadRng, Rng};

#[derive(Clone, Copy)]
enum Face {
    Front,
    Back,
    Right,
    Left,
    Top,
    Bottom,
}

static faces: [Face; 6] = [
    Face::Front,
    Face::Back,
    Face::Right,
    Face::Left,
    Face::Top,
    Face::Bottom,
];

pub struct Chunk {
    values: Box<[[[u16; CHUNK_SIZE_Z]; CHUNK_SIZE_Y]; CHUNK_SIZE_X]>,
    x_neighbor: Box<[Option<[[u16; CHUNK_SIZE_Y]; CHUNK_SIZE_Z + 2]>; 2]>,
    z_neighbor: Box<[Option<[[u16; CHUNK_SIZE_Y]; CHUNK_SIZE_X]>; 2]>,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            values: Box::new([[[0; CHUNK_SIZE_Z]; CHUNK_SIZE_Y]; CHUNK_SIZE_X]),
            x_neighbor: Box::new([None; 2]),
            z_neighbor: Box::new([None; 2]),
        }
    }

    fn try_index(&self, pos: IVec3) -> Option<u16> {
        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return None;
        }

        if pos.x < 0 || pos.x >= CHUNK_SIZE_X as i32 {
            let layer = (pos.x + 1) as usize / CHUNK_SIZE_X;
            if let Some(slice) = self.x_neighbor[layer] {
                return Some(slice[(pos.z + 1) as usize][pos.y as usize]);
            } else {
                return None;
            }
        }

        if pos.z < 0 || pos.z >= CHUNK_SIZE_Z as i32 {
            let layer = (pos.z + 1) as usize / CHUNK_SIZE_Z;
            if let Some(slice) = self.z_neighbor[layer] {
                return Some(slice[pos.x as usize][pos.y as usize]);
            } else {
                return None;
            }
        }

        Some(self.values[pos.x as usize][pos.y as usize][pos.z as usize])
    }

    pub fn generate(&mut self, pos: IVec3) {
        let perlin = Perlin::default();
        let mut rng = rand::thread_rng();

        fn evaluate(perlin: Perlin, rng: &mut ThreadRng, x: f64, y: f64, z: f64) -> u16 {
            let scale = 100.0;
            let p = perlin.get([x / scale, y / scale, z / scale]);
            if p + y * 0.02 - 1.0 < 0.0 {
                rng.gen_range(1..5)
            } else {
                0
            }
        }

        // values
        for x in 0..CHUNK_SIZE_X {
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    let value = evaluate(
                        perlin,
                        &mut rng,
                        (x as i32 + pos.x) as f64,
                        (y as i32 + pos.y) as f64,
                        (z as i32 + pos.z) as f64,
                    );
                    self.values[x][y][z] = value;
                }
            }
        }

        // x_neighbor
        for l in 0..2 {
            let layer = self.x_neighbor[l].insert([[0; CHUNK_SIZE_Y]; CHUNK_SIZE_Z + 2]);
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    let x = l as i32 * (CHUNK_SIZE_X as i32 + 1) - 1;
                    let value = evaluate(
                        perlin,
                        &mut rng,
                        (x as i32 + pos.x) as f64,
                        (y as i32 + pos.y) as f64,
                        (z as i32 + pos.z) as f64,
                    );
                    layer[z + 1][y] = value;
                }
            }
        }

        // z_neighbor
        for l in 0..2 {
            let layer = self.z_neighbor[l].insert([[0; CHUNK_SIZE_Y]; CHUNK_SIZE_X]);
            for y in 0..CHUNK_SIZE_Y {
                for x in 0..CHUNK_SIZE_X {
                    let z = l as i32 * (CHUNK_SIZE_Z as i32 + 1) - 1;
                    let value = evaluate(
                        perlin,
                        &mut rng,
                        (x as i32 + pos.x) as f64,
                        (y as i32 + pos.y) as f64,
                        (z as i32 + pos.z) as f64,
                    );
                    layer[x][y] = value;
                }
            }
        }
    }

    pub fn generate_mesh(&mut self) -> TmpMesh {
        fn get_ao(e1: bool, e2: bool, c: bool) -> u8 {
            if e1 && e2 {
                return 3;
            }

            return e1 as u8 + e2 as u8 + c as u8;
        }

        fn get_aooo(v: u8) -> f32 {
            match v {
                0 => 1.0,
                1 => 0.6,
                2 => 0.6,
                3 => 0.2,
                _ => -5.0,
            }
        }

        fn corner(face: Face, i: u8) -> IVec3 {
            match face {
                Face::Front => match i {
                    0 => IVec3::new(-1, -1, 1),
                    1 => IVec3::new(1, -1, 1),
                    2 => IVec3::new(1, 1, 1),
                    3 => IVec3::new(-1, 1, 1),
                    _ => IVec3::new(0, 0, 0),
                },
                Face::Back => match i {
                    0 => IVec3::new(-1, -1, -1),
                    1 => IVec3::new(-1, 1, -1),
                    2 => IVec3::new(1, 1, -1),
                    3 => IVec3::new(1, -1, -1),
                    _ => IVec3::new(0, 0, 0),
                },
                Face::Right => match i {
                    0 => IVec3::new(1, -1, -1),
                    1 => IVec3::new(1, 1, -1),
                    2 => IVec3::new(1, 1, 1),
                    3 => IVec3::new(1, -1, 1),
                    _ => IVec3::new(0, 0, 0),
                },
                Face::Left => match i {
                    0 => IVec3::new(-1, -1, -1),
                    1 => IVec3::new(-1, -1, 1),
                    2 => IVec3::new(-1, 1, 1),
                    3 => IVec3::new(-1, 1, -1),
                    _ => IVec3::new(0, 0, 0),
                },
                Face::Top => match i {
                    0 => IVec3::new(-1, 1, -1),
                    1 => IVec3::new(-1, 1, 1),
                    2 => IVec3::new(1, 1, 1),
                    3 => IVec3::new(1, 1, -1),
                    _ => IVec3::new(0, 0, 0),
                },
                Face::Bottom => match i {
                    0 => IVec3::new(-1, -1, -1),
                    1 => IVec3::new(1, -1, -1),
                    2 => IVec3::new(1, -1, 1),
                    3 => IVec3::new(-1, -1, 1),
                    _ => IVec3::new(0, 0, 0),
                },
            }
        }

        fn mask(face: Face) -> [IVec3; 2] {
            match face {
                Face::Front => [IVec3::new(0, 1, 1), IVec3::new(1, 0, 1)],
                Face::Back => [IVec3::new(0, 1, 1), IVec3::new(1, 0, 1)],
                Face::Right => [IVec3::new(1, 0, 1), IVec3::new(1, 1, 0)],
                Face::Left => [IVec3::new(1, 0, 1), IVec3::new(1, 1, 0)],
                Face::Top => [IVec3::new(0, 1, 1), IVec3::new(1, 1, 0)],
                Face::Bottom => [IVec3::new(0, 1, 1), IVec3::new(1, 1, 0)],
            }
        }

        let mut tmp_mesh = TmpMesh::new();

        for x in 0..CHUNK_SIZE_X {
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    if self.values[x][y][z] != 0 {
                        let pos = IVec3::new(x as i32, y as i32, z as i32);
                        for face in faces {
                            let dir = Chunk::face_dir(face);
                            let dir_pos = pos + dir;
                            let dir_value = self.try_index(dir_pos).unwrap_or(1);

                            if dir_value == 0 {
                                let mut ao = [0, 0, 0, 0];
                                if AO {
                                    for i in 0..4 {
                                        let offset = corner(face, i);
                                        let mask = mask(face);
                                        let e1 =
                                            self.try_index(offset * mask[0] + pos).unwrap_or(0)
                                                != 0;
                                        let e2 =
                                            self.try_index(offset * mask[1] + pos).unwrap_or(0)
                                                != 0;
                                        let c = self.try_index(offset + pos).unwrap_or(0) != 0;
                                        ao[i as usize] = get_ao(e1, e2, c);
                                    }
                                }

                                let flip = ao[0] + ao[2] > ao[1] + ao[3];
                                tmp_mesh.add_face(
                                    face,
                                    IVec3::new(pos.x, pos.y, pos.z),
                                    [
                                        get_aooo(ao[0]),
                                        get_aooo(ao[1]),
                                        get_aooo(ao[2]),
                                        get_aooo(ao[3]),
                                    ],
                                    flip,
                                    Chunk::texture(
                                        face,
                                        Chunk::block_textures(self.values[x][y][z]),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }

        tmp_mesh
    }

    fn face_dir(face: Face) -> IVec3 {
        match face {
            Face::Front => IVec3::new(0, 0, 1),
            Face::Back => IVec3::new(0, 0, -1),
            Face::Right => IVec3::new(1, 0, 0),
            Face::Left => IVec3::new(-1, 0, 0),
            Face::Top => IVec3::new(0, 1, 0),
            Face::Bottom => IVec3::new(0, -1, 0),
        }
    }

    fn block_textures(block: u16) -> BlockTexture {
        match block {
            1 => BlockTexture::Sides(0, 2, 3),
            2 => BlockTexture::Single(1),
            3 => BlockTexture::Single(2),
            4 => BlockTexture::Single(4),
            _ => BlockTexture::Single(9 * 16 + 9),
        }
    }

    fn texture(face: Face, block: BlockTexture) -> u16 {
        match block {
            BlockTexture::Single(i) => i,
            BlockTexture::Sides(t, b, s) => match face {
                Face::Front => s,
                Face::Back => s,
                Face::Right => s,
                Face::Left => s,
                Face::Top => t,
                Face::Bottom => b,
            },
            BlockTexture::Unique(f, b, r, l, t, u) => match face {
                Face::Front => f,
                Face::Back => b,
                Face::Right => r,
                Face::Left => l,
                Face::Top => t,
                Face::Bottom => u,
            },
        }
    }
}

enum BlockTexture {
    Single(u16),
    Sides(u16, u16, u16),                 // top, bottom, sides
    Unique(u16, u16, u16, u16, u16, u16), // front, back, right, left, top, bottom
}

pub struct TmpMesh {
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub ao: Vec<f32>,
    pub indices: Vec<u32>,
}

impl TmpMesh {
    fn new() -> Self {
        TmpMesh {
            vertices: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            ao: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn add_face(&mut self, face: Face, o: IVec3, ao: [f32; 4], flip: bool, texture_id: u16) {
        let x = o.x as f32;
        let y = o.y as f32;
        let z = o.z as f32;

        self.ao.extend(ao);

        let a = self.vertices.len() as u32;
        self.indices.extend(match flip {
            false => [a, a + 1, a + 2, a, a + 2, a + 3],
            true => [a + 1, a + 3, a, a + 1, a + 2, a + 3],
        });

        let tex_y = (texture_id / 16) as f32;
        let tex_x = texture_id as f32 - tex_y;

        let tl = [(0.0 + tex_x) / 16.0, (0.0 + tex_y) / 16.0];
        let tr = [(1.0 + tex_x) / 16.0, (0.0 + tex_y) / 16.0];
        let bl = [(0.0 + tex_x) / 16.0, (1.0 + tex_y) / 16.0];
        let br = [(1.0 + tex_x) / 16.0, (1.0 + tex_y) / 16.0];

        match face {
            Face::Front => {
                self.vertices.extend([
                    [0.0 + x, 0.0 + y, 1.0 + z],
                    [1.0 + x, 0.0 + y, 1.0 + z],
                    [1.0 + x, 1.0 + y, 1.0 + z],
                    [0.0 + x, 1.0 + y, 1.0 + z],
                ]);
                self.normals.extend([
                    [0.0, 0.0, 1.0],
                    [0.0, 0.0, 1.0],
                    [0.0, 0.0, 1.0],
                    [0.0, 0.0, 1.0],
                ]);
                self.uvs.extend([bl, br, tr, tl]);
            }
            Face::Back => {
                self.vertices.extend([
                    [0.0 + x, 0.0 + y, 0.0 + z],
                    [0.0 + x, 1.0 + y, 0.0 + z],
                    [1.0 + x, 1.0 + y, 0.0 + z],
                    [1.0 + x, 0.0 + y, 0.0 + z],
                ]);
                self.normals.extend([
                    [0.0, 0.0, -1.0],
                    [0.0, 0.0, -1.0],
                    [0.0, 0.0, -1.0],
                    [0.0, 0.0, -1.0],
                ]);
                self.uvs.extend([br, tr, tl, bl]);
            }
            Face::Right => {
                self.vertices.extend([
                    [1.0 + x, 0.0 + y, 0.0 + z],
                    [1.0 + x, 1.0 + y, 0.0 + z],
                    [1.0 + x, 1.0 + y, 1.0 + z],
                    [1.0 + x, 0.0 + y, 1.0 + z],
                ]);
                self.normals.extend([
                    [1.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                ]);
                self.uvs.extend([br, tr, tl, bl]);
            }
            Face::Left => {
                self.vertices.extend([
                    [0.0 + x, 0.0 + y, 0.0 + z],
                    [0.0 + x, 0.0 + y, 1.0 + z],
                    [0.0 + x, 1.0 + y, 1.0 + z],
                    [0.0 + x, 1.0 + y, 0.0 + z],
                ]);
                self.normals.extend([
                    [-1.0, 0.0, 0.0],
                    [-1.0, 0.0, 0.0],
                    [-1.0, 0.0, 0.0],
                    [-1.0, 0.0, 0.0],
                ]);
                self.uvs.extend([bl, br, tr, tl]);
            }
            Face::Top => {
                self.vertices.extend([
                    [0.0 + x, 1.0 + y, 0.0 + z],
                    [0.0 + x, 1.0 + y, 1.0 + z],
                    [1.0 + x, 1.0 + y, 1.0 + z],
                    [1.0 + x, 1.0 + y, 0.0 + z],
                ]);
                self.normals.extend([
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0],
                ]);
                self.uvs.extend([br, tr, tl, bl]);
            }
            Face::Bottom => {
                self.vertices.extend([
                    [0.0 + x, 0.0 + y, 0.0 + z],
                    [1.0 + x, 0.0 + y, 0.0 + z],
                    [1.0 + x, 0.0 + y, 1.0 + z],
                    [0.0 + x, 0.0 + y, 1.0 + z],
                ]);
                self.normals.extend([
                    [0.0, -1.0, 0.0],
                    [0.0, -1.0, 0.0],
                    [0.0, -1.0, 0.0],
                    [0.0, -1.0, 0.0],
                ]);
                self.uvs.extend([bl, br, tr, tl]);
            }
        }
    }
}
