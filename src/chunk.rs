use bevy::prelude::{IVec2, IVec3};
use simdnoise::NoiseBuilder;

use super::World;

pub const CHUNK_SIZE_X: usize = 32;
pub const CHUNK_SIZE_Y: usize = 96;
pub const CHUNK_SIZE_Z: usize = 32;
const AO: bool = true;

#[derive(Clone, Copy)]
enum Face {
    Front,
    Back,
    Right,
    Left,
    Top,
    Bottom,
}

enum BlockTexture {
    Single(u16),            // all
    Sides(u16, u16, u16),   // top, bottom, sides
    Opisite(u16, u16, u16), // front and bottom, right and left, top and bottom
}

const FACES: [Face; 6] = [
    Face::Front,
    Face::Back,
    Face::Right,
    Face::Left,
    Face::Top,
    Face::Bottom,
];
const AO_LEVELS: [f32; 4] = [1.0, 0.6, 0.6, 0.4];
const FACE_DIR: [[i32; 3]; 6] = [
    [0, 0, 1],
    [0, 0, -1],
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
];
const CORNERS: [[[i32; 3]; 4]; 6] = [
    [[-1, -1, 1], [1, -1, 1], [1, 1, 1], [-1, 1, 1]],
    [[-1, -1, -1], [-1, 1, -1], [1, 1, -1], [1, -1, -1]],
    [[1, -1, -1], [1, 1, -1], [1, 1, 1], [1, -1, 1]],
    [[-1, -1, -1], [-1, -1, 1], [-1, 1, 1], [-1, 1, -1]],
    [[-1, 1, -1], [-1, 1, 1], [1, 1, 1], [1, 1, -1]],
    [[-1, -1, -1], [1, -1, -1], [1, -1, 1], [-1, -1, 1]],
];
const BLOCKS: [BlockTexture; 7] = [
    BlockTexture::Single(153),
    BlockTexture::Sides(0, 2, 3),
    BlockTexture::Single(1),
    BlockTexture::Single(2),
    BlockTexture::Single(4),
    BlockTexture::Opisite(2 * 16 + 12, 2 * 16 + 13, 3 * 16 + 14),
    BlockTexture::Opisite(3 * 16 + 12, 3 * 16 + 11, 2 * 16 + 11),
];
const MASK: [[[i32; 3]; 2]; 6] = [
    [[0, 1, 1], [1, 0, 1]],
    [[0, 1, 1], [1, 0, 1]],
    [[1, 0, 1], [1, 1, 0]],
    [[1, 0, 1], [1, 1, 0]],
    [[0, 1, 1], [1, 1, 0]],
    [[0, 1, 1], [1, 1, 0]],
];

pub struct Chunk {
    chunk_id: IVec2, // :(
    pub values: Box<[[[u16; CHUNK_SIZE_Z]; CHUNK_SIZE_Y]; CHUNK_SIZE_X]>,
}

impl Chunk {
    pub fn new(chunk_id: IVec2) -> Self {
        Chunk {
            chunk_id: chunk_id,
            values: Box::new([[[0; CHUNK_SIZE_Z]; CHUNK_SIZE_Y]; CHUNK_SIZE_X]),
        }
    }

    #[inline(always)]
    fn try_index(&self, world: &World, pos: IVec3) -> Option<u16> {
        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return None;
        }

        if pos.x < 0 || pos.x >= CHUNK_SIZE_X as i32 || pos.z < 0 || pos.z >= CHUNK_SIZE_Z as i32 {
            let x = div_floor(pos.x, CHUNK_SIZE_X as i32);
            let z = div_floor(pos.z, CHUNK_SIZE_Z as i32);
            let offset_chunk_id = self.chunk_id + IVec2::new(x, z);
            if let Some(chunk) = world.chunks.get(&offset_chunk_id) {
                let pos = pos - IVec3::new(x * CHUNK_SIZE_X as i32, 0, z * CHUNK_SIZE_Z as i32);
                return Some(chunk.values[pos.x as usize][pos.y as usize][pos.z as usize]);
            } else {
                return None;
            }
        }

        Some(self.values[pos.x as usize][pos.y as usize][pos.z as usize])
    }

    pub fn generate(&mut self, pos: IVec3) {
        fn evaluate(noise: &Vec<f32>, x: i32, y: i32, z: i32) -> u16 {
            let p = noise
                [x as usize + y as usize * CHUNK_SIZE_X + z as usize * CHUNK_SIZE_X * CHUNK_SIZE_Y];
            if p + y as f32 * 0.12 - 5.0 < 0.0 {
                1
            } else {
                0
            }
        }

        let (noise, _, _) = NoiseBuilder::gradient_3d_offset(
            pos.x as f32,
            CHUNK_SIZE_X,
            0.0,
            CHUNK_SIZE_Y,
            pos.z as f32,
            CHUNK_SIZE_Z,
        )
        .generate();

        // values
        for x in 0..CHUNK_SIZE_X {
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    let value = evaluate(
                        &noise, x as i32, // + pos.x
                        y as i32, // + pos.y
                        z as i32, // + pos.z
                    );
                    self.values[x][y][z] = value;
                }
            }
        }

        // // x_neighbor
        // for l in 0..2 {
        //     let layer = self.x_neighbor[l].insert([[0; CHUNK_SIZE_Y]; CHUNK_SIZE_Z + 2]);
        //     for y in 0..CHUNK_SIZE_Y {
        //         for z in 0..CHUNK_SIZE_Z {
        //             let x = l as i32 * (CHUNK_SIZE_X as i32 + 1) - 1;
        //             let value = evaluate(
        //                 &noise,
        //                 (x as i32 + pos.x),
        //                 (y as i32 + pos.y),
        //                 (z as i32 + pos.z),
        //             );
        //             layer[z + 1][y] = value;
        //         }
        //     }
        // }

        // // z_neighbor
        // for l in 0..2 {
        //     let layer = self.z_neighbor[l].insert([[0; CHUNK_SIZE_Y]; CHUNK_SIZE_X]);
        //     for y in 0..CHUNK_SIZE_Y {
        //         for x in 0..CHUNK_SIZE_X {
        //             let z = l as i32 * (CHUNK_SIZE_Z as i32 + 1) - 1;
        //             let value = evaluate(
        //                 &noise,
        //                 (x as i32 + pos.x),
        //                 (y as i32 + pos.y),
        //                 (z as i32 + pos.z),
        //             );
        //             layer[x][y] = value;
        //         }
        //     }
        // }
    }

    pub fn generate_mesh(&self, world: &World) -> TmpMesh {
        #[inline]
        fn get_ao(e1: bool, e2: bool, c: bool) -> usize {
            if e1 && e2 {
                return 3;
            }

            return e1 as usize + e2 as usize + c as usize;
        }

        let mut tmp_mesh = TmpMesh::new(8192);

        for x in 0..CHUNK_SIZE_X {
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    if self.values[x][y][z] != 0 {
                        let pos = IVec3::new(x as i32, y as i32, z as i32);
                        for face in FACES {
                            let dir = FACE_DIR[face as usize].into();
                            let dir_pos = pos + dir;
                            let dir_value = self.try_index(world, dir_pos).unwrap_or(1);

                            if dir_value == 0 {
                                let mut ao = [0, 0, 0, 0];
                                if AO {
                                    for i in 0..4 {
                                        let offset: IVec3 = CORNERS[face as usize][i].into();
                                        let e1 = self
                                            .try_index(
                                                world,
                                                offset * IVec3::from(MASK[face as usize][0]) + pos,
                                            )
                                            .unwrap_or(0)
                                            != 0;
                                        let e2 = self
                                            .try_index(
                                                world,
                                                offset * IVec3::from(MASK[face as usize][1]) + pos,
                                            )
                                            .unwrap_or(0)
                                            != 0;
                                        let c =
                                            self.try_index(world, offset + pos).unwrap_or(0) != 0;
                                        ao[i as usize] = get_ao(e1, e2, c);
                                    }
                                }

                                let flip = ao[0] + ao[2] > ao[1] + ao[3];
                                tmp_mesh.add_face(
                                    face,
                                    IVec3::new(pos.x, pos.y, pos.z),
                                    [
                                        AO_LEVELS[ao[0]],
                                        AO_LEVELS[ao[1]],
                                        AO_LEVELS[ao[2]],
                                        AO_LEVELS[ao[3]],
                                    ],
                                    flip,
                                    Chunk::texture(face, &BLOCKS[self.values[x][y][z] as usize]),
                                );
                            }
                        }
                    }
                }
            }
        }

        tmp_mesh
    }

    #[inline]
    fn texture(face: Face, block: &BlockTexture) -> u16 {
        match block {
            BlockTexture::Single(i) => *i,
            BlockTexture::Sides(t, b, s) => match face {
                Face::Front => *s,
                Face::Back => *s,
                Face::Right => *s,
                Face::Left => *s,
                Face::Top => *t,
                Face::Bottom => *b,
            },
            BlockTexture::Opisite(f, r, t) => match face {
                Face::Front => *f,
                Face::Back => *f,
                Face::Right => *r,
                Face::Left => *r,
                Face::Top => *t,
                Face::Bottom => *t,
            },
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
    fn new(capacity: usize) -> Self {
        TmpMesh {
            vertices: Vec::with_capacity(capacity),
            normals: Vec::with_capacity(capacity),
            uvs: Vec::with_capacity(capacity),
            ao: Vec::with_capacity(capacity),
            indices: Vec::with_capacity(capacity * 2),
        }
    }

    #[inline]
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
        let tex_x = texture_id as f32 - tex_y * 16.0;

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

pub const fn div_floor(lhs: i32, rhs: i32) -> i32 {
    let d = lhs / rhs;
    let r = lhs % rhs;
    if (r > 0 && rhs < 0) || (r < 0 && rhs > 0) {
        d - 1
    } else {
        d
    }
}
