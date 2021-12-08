use super::{IVec3, Perlin, NoiseFn, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z, AO};

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

        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return None;
        }

        Some(self.values[pos.x as usize][pos.y as usize][pos.z as usize])
    }

    pub fn generate(&mut self, pos: IVec3) {
        let perlin = Perlin::default();

        fn evaluate(perlin: Perlin, x: f64, y: f64, z: f64) -> bool {
            let p = perlin.get([x / 20.0, y / 20.0, z / 20.0]);
            p + y * 0.1 - 20.0 < 0.0
        }

        // values
        for x in 0..CHUNK_SIZE_X {
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    let value = evaluate(
                        perlin,
                        (x as i32 + pos.x) as f64,
                        (y as i32 + pos.y) as f64,
                        (z as i32 + pos.z) as f64,
                    ) as u16;
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
                        (x as i32 + pos.x) as f64,
                        (y as i32 + pos.y) as f64,
                        (z as i32 + pos.z) as f64,
                    ) as u16;
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
                        (x as i32 + pos.x) as f64,
                        (y as i32 + pos.y) as f64,
                        (z as i32 + pos.z) as f64,
                    ) as u16;
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
                1 => 0.4,
                2 => 0.3,
                3 => 0.0,
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
                    if self.values[x][y][z] == 1 {
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

    fn add_face(&mut self, face: Face, o: IVec3, ao: [f32; 4], flip: bool) {
        let x = o.x as f32;
        let y = o.y as f32;
        let z = o.z as f32;

        self.uvs
            .extend([[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]]);
        self.ao.extend(ao);

        let a = self.vertices.len() as u32;
        self.indices.extend(match flip {
            false => [a, a + 1, a + 2, a, a + 2, a + 3],
            true => [a + 1, a + 3, a, a + 1, a + 2, a + 3],
        });

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
            }
        }
    }
}
