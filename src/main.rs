#![allow(non_upper_case_globals)]

use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::PerspectiveProjection,
        mesh::{Indices, VertexAttributeValues},
        pipeline::{PipelineDescriptor, PrimitiveTopology::TriangleList, RenderPipeline},
        render_graph::{base, AssetRenderResourcesNode, RenderGraph},
        renderer::RenderResources,
        shader::{ShaderStage, ShaderStages},
    },
    tasks::{AsyncComputeTaskPool, Task},
};

use bevy_flycam::{FlyCam, MovementSettings, NoCameraPlayerPlugin};
use futures_lite::future;
use noise::{NoiseFn, Perlin};

const CHUNK_SIZE_X: usize = 32;
const CHUNK_SIZE_Y: usize = 384;
const CHUNK_SIZE_Z: usize = 32;

const VIEW_DISTANCE: usize = 16;
const AO: bool = true;

fn main() {
    App::build()
        .insert_resource(Msaa { samples: 4 })
        .add_plugins(DefaultPlugins)
        .add_plugin(NoCameraPlayerPlugin)
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_asset::<ChunkMaterial>()
        .insert_resource(MovementSettings {
            sensitivity: 0.00012, // default: 0.00012
            speed: 8.0,           // default: 12.0
        })
        .add_startup_system(setup.system())
        .add_startup_system(spawn_chunk_tasks.system())
        .add_system(fps_system.system())
        .add_system(handle_chunk_tasks.system())
        .run();
}

#[derive(RenderResources, Default, TypeUuid)]
#[uuid = "0320b9b8-b3a3-4baa-8bfa-c94008177b17"]
struct ChunkMaterial;
struct ChunkMaterialHandle(Handle<ChunkMaterial>);
struct ChunkPipelineHandle(Handle<PipelineDescriptor>);

const VERTEX_SHADER: &str = include_str!("chunk.vert");
const FRAGMENT_SHADER: &str = include_str!("chunk.frag");

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut pbr_materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
    mut pipelines: ResMut<Assets<PipelineDescriptor>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut render_graph: ResMut<RenderGraph>,
    asset_server: Res<AssetServer>,
) {
    // Create a new shader pipeline
    let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
        vertex: shaders.add(Shader::from_glsl(ShaderStage::Vertex, VERTEX_SHADER)),
        fragment: Some(shaders.add(Shader::from_glsl(ShaderStage::Fragment, FRAGMENT_SHADER))),
    }));
    commands.insert_resource(ChunkPipelineHandle(pipeline_handle));

    // Add an AssetRenderResourcesNode to our Render Graph. This will bind
    // MyMaterialWithVertexColorSupport resources to our shader
    render_graph.add_system_node(
        "chunk_material",
        AssetRenderResourcesNode::<ChunkMaterial>::new(true),
    );

    // Add a Render Graph edge connecting our new "my_material" node to the main pass node. This
    // ensures "my_material" runs before the main pass
    render_graph
        .add_node_edge("chunk_material", base::node::MAIN_PASS)
        .unwrap();

    // Create a new material
    let chunk_material_handle = chunk_materials.add(ChunkMaterial {});
    commands.insert_resource(ChunkMaterialHandle(chunk_material_handle));

    // camera
    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(-2.0, 210.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
            perspective_projection: PerspectiveProjection {
                fov: 1.48353,
                near: 0.05,
                far: 10000.0,
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(FlyCam);
    // origin
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 0.5 })),
        material: pbr_materials.add(bevy::prelude::Color::rgb(0.1, 0.1, 0.1).into()),
        transform: Transform::from_xyz(0.0, 0.0, 0.0),
        ..Default::default()
    });
    // light
    commands.spawn_bundle(LightBundle {
        transform: Transform::from_xyz(3.0, 8.0, 5.0),
        ..Default::default()
    });
    // sun
    commands.spawn_bundle(LightBundle {
        transform: Transform::from_xyz(5000.0, 10000.0, 2000.0),
        light: Light {
            intensity: 1000000000.0,
            range: 1000000.0,
            ..Default::default()
        },
        ..Default::default()
    });
    // ui
    commands.spawn_bundle(UiCameraBundle::default());
    // fps
    commands.spawn_bundle(TextBundle {
        text: Text {
            sections: vec![TextSection {
                value: "0.00".to_string(),
                style: TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 40.0,
                    color: Color::rgb(0.0, 0.0, 0.0),
                    ..Default::default()
                },
            }],
            ..Default::default()
        },
        style: Style {
            position_type: PositionType::Absolute,
            position: Rect {
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    });
}

fn fps_system(diagnostics: Res<Diagnostics>, mut query: Query<&mut Text>) {
    if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(average) = fps.average() {
            for mut text in query.iter_mut() {
                text.sections[0].value = format!("{:.1}", average);
            }
        }
    }
}

fn spawn_chunk_tasks(mut commands: Commands, thread_pool: Res<AsyncComputeTaskPool>) {
    let view_distance: i32 = VIEW_DISTANCE as i32;
    let mut chunks_to_load: Vec<IVec2> =
        Vec::with_capacity((view_distance * view_distance) as usize);

    for x in -view_distance..=view_distance {
        for y in -view_distance..=view_distance {
            let sqr_dist = x * x + y * y;
            if sqr_dist < view_distance * view_distance {
                chunks_to_load.push(IVec2::new(x, y));
            }
        }
    }

    chunks_to_load.sort_unstable_by(|a, b| {
        let a_sqr_dist = a.x * a.x + a.y * a.y;
        let b_sqr_dist = b.x * b.x + b.y * b.y;
        a_sqr_dist.cmp(&b_sqr_dist)
    });

    for chunk_id in chunks_to_load {
        let task = thread_pool.spawn(async move {
            let mut chunk = Chunk::new();
            chunk.generate(IVec3::new(
                chunk_id.x * CHUNK_SIZE_X as i32,
                0,
                chunk_id.y * CHUNK_SIZE_Z as i32,
            ));

            let tmp_mesh = chunk.generate_mesh();

            let mut mesh = Mesh::new(TriangleList);
            mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, tmp_mesh.vertices);
            mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, tmp_mesh.normals);
            mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, tmp_mesh.uvs);
            mesh.set_attribute("AO", VertexAttributeValues::from(tmp_mesh.ao));
            mesh.set_indices(Some(Indices::U32(tmp_mesh.indices)));

            ChunkTaskData {
                chunk_id: chunk_id,
                mesh: mesh,
            }
        });

        // Spawn new entity and add our new task as a component
        commands.spawn().insert(task);
    }
}

fn handle_chunk_tasks(
    mut commands: Commands,
    mut completed_chunks: Query<(Entity, &mut Task<ChunkTaskData>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    material_handle: Res<ChunkMaterialHandle>,
    pipeline_handle: Res<ChunkPipelineHandle>,
) {
    for (entity, mut task) in completed_chunks.iter_mut() {
        if let Some(chunk_task_data) = future::block_on(future::poll_once(&mut *task)) {
            // Add our new PbrBundle of components to our tagged entity
            commands
                .spawn_bundle(MeshBundle {
                    mesh: meshes.add(chunk_task_data.mesh), // use our cube with vertex colors
                    render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
                        pipeline_handle.0.clone(),
                    )]),
                    transform: Transform::from_xyz(
                        chunk_task_data.chunk_id.x as f32 * CHUNK_SIZE_X as f32,
                        0.0,
                        chunk_task_data.chunk_id.y as f32 * CHUNK_SIZE_Z as f32,
                    ),
                    ..Default::default()
                })
                .insert(material_handle.0.clone());

            // Task is complete, so remove task component from entity
            commands.entity(entity).remove::<Task<ChunkTaskData>>();
        }
    }
}

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

struct ChunkTaskData {
    chunk_id: IVec2,
    mesh: Mesh,
}

struct Chunk {
    values: Box<[[[u16; CHUNK_SIZE_Z]; CHUNK_SIZE_Y]; CHUNK_SIZE_X]>,
    x_neighbor: Box<[Option<[[u16; CHUNK_SIZE_Y]; CHUNK_SIZE_Z + 2]>; 2]>,
    z_neighbor: Box<[Option<[[u16; CHUNK_SIZE_Y]; CHUNK_SIZE_X]>; 2]>,
}

impl Chunk {
    fn new() -> Self {
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

    fn generate(&mut self, pos: IVec3) {
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

    fn generate_mesh(&mut self) -> TmpMesh {
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
                                    Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32),
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

struct TmpMesh {
    vertices: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    ao: Vec<f32>,
    indices: Vec<u32>,
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

    fn add_face(&mut self, face: Face, o: Vec3, ao: [f32; 4], flip: bool) {
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
                    [0.0 + o.x, 0.0 + o.y, 1.0 + o.z],
                    [1.0 + o.x, 0.0 + o.y, 1.0 + o.z],
                    [1.0 + o.x, 1.0 + o.y, 1.0 + o.z],
                    [0.0 + o.x, 1.0 + o.y, 1.0 + o.z],
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
                    [0.0 + o.x, 0.0 + o.y, 0.0 + o.z],
                    [0.0 + o.x, 1.0 + o.y, 0.0 + o.z],
                    [1.0 + o.x, 1.0 + o.y, 0.0 + o.z],
                    [1.0 + o.x, 0.0 + o.y, 0.0 + o.z],
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
                    [1.0 + o.x, 0.0 + o.y, 0.0 + o.z],
                    [1.0 + o.x, 1.0 + o.y, 0.0 + o.z],
                    [1.0 + o.x, 1.0 + o.y, 1.0 + o.z],
                    [1.0 + o.x, 0.0 + o.y, 1.0 + o.z],
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
                    [0.0 + o.x, 0.0 + o.y, 0.0 + o.z],
                    [0.0 + o.x, 0.0 + o.y, 1.0 + o.z],
                    [0.0 + o.x, 1.0 + o.y, 1.0 + o.z],
                    [0.0 + o.x, 1.0 + o.y, 0.0 + o.z],
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
                    [0.0 + o.x, 1.0 + o.y, 0.0 + o.z],
                    [0.0 + o.x, 1.0 + o.y, 1.0 + o.z],
                    [1.0 + o.x, 1.0 + o.y, 1.0 + o.z],
                    [1.0 + o.x, 1.0 + o.y, 0.0 + o.z],
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
                    [0.0 + o.x, 0.0 + o.y, 0.0 + o.z],
                    [1.0 + o.x, 0.0 + o.y, 0.0 + o.z],
                    [1.0 + o.x, 0.0 + o.y, 1.0 + o.z],
                    [0.0 + o.x, 0.0 + o.y, 1.0 + o.z],
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
