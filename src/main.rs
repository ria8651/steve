#![allow(non_upper_case_globals)]

// Imports
use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::mouse::MouseMotion,
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
use dashmap::{DashMap, DashSet};
use futures_lite::future;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

mod chunk;
use chunk::*;

const VIEW_DISTANCE: usize = 16;
const SPEED: f32 = 500.0;
const SENSITIVITY: f32 = 0.002;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
static COUNTER2: AtomicUsize = AtomicUsize::new(0);
static COUNTER3: AtomicUsize = AtomicUsize::new(0);

// Structs
#[derive(RenderResources, Default, TypeUuid)]
#[uuid = "0320b9b8-b3a3-4baa-8bfa-c94008177b17"]
struct ChunkMaterial {
    texture_atlas: Handle<Texture>,
}

#[derive(RenderResources, Default, TypeUuid)]
#[uuid = "93fb26fc-6c05-489b-9029-601edf703b6b"]
struct TextureAtlas {
    texture: Handle<Texture>,
}

struct ChunkComponent {
    chunk_id: IVec2,
}

struct ChunkMaterialHandle(Handle<ChunkMaterial>);
struct ChunkPipelineHandle(Handle<PipelineDescriptor>);

struct ChunkTask {
    id: IVec2,
    task: Task<ChunkTaskData>,
}

struct ChunkTaskData {
    mesh: Mesh,
}

pub struct World {
    chunks: DashMap<IVec2, Chunk>,
    generating_chunks: DashSet<IVec2>,
    meshing_chunks: DashSet<IVec2>,
    neighbor_count: DashMap<IVec2, usize>,
    meshing_queue: DashSet<IVec2>,
}

struct ChunkPriorityMap(Option<Vec<IVec2>>);

struct Character {
    velocity: Vec3,
    rotation: Vec2,
    current_chunk: IVec2,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            velocity: Vec3::new(0.0, 0.0, 0.0),
            rotation: Vec2::new(0.0, 0.0),
            current_chunk: IVec2::new(1000000000, 1000000000),
        }
    }
}

const neighbors: [[i32; 2]; 9] = [
    [0, 0],
    [-1, -1],
    [0, -1],
    [1, -1],
    [-1, 0],
    [1, 0],
    [-1, 1],
    [0, 1],
    [1, 1],
];

// Functions
fn main() {
    App::build()
        // .insert_resource(Msaa { samples: 4 })
        .insert_resource(WindowDescriptor {
            title: "Steve".to_string(),
            // vsync: false,
            ..Default::default()
        })
        .insert_resource(Arc::new(World {
            chunks: DashMap::new(),
            generating_chunks: DashSet::new(),
            meshing_chunks: DashSet::new(),
            neighbor_count: DashMap::new(),
            meshing_queue: DashSet::new(),
        }))
        .insert_resource(ChunkPriorityMap(None))
        .add_plugins(DefaultPlugins)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_asset::<ChunkMaterial>()
        .add_startup_system(setup.system())
        .add_startup_system(character_setup.system())
        .add_system(handle_chunk_tasks.system())
        .add_system(character_system.system())
        .add_system(fps_system.system())
        .run();
}

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
    // custom pipeline
    let texture_atlas_handle = asset_server.load("textures/terrain.png");

    let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
        vertex: shaders.add(Shader::from_glsl(
            ShaderStage::Vertex,
            include_str!("chunk.vert"),
        )),
        fragment: Some(shaders.add(Shader::from_glsl(
            ShaderStage::Fragment,
            include_str!("chunk.frag"),
        ))),
    }));
    commands.insert_resource(ChunkPipelineHandle(pipeline_handle));

    render_graph.add_system_node(
        "chunk_material",
        AssetRenderResourcesNode::<ChunkMaterial>::new(true),
    );

    render_graph
        .add_node_edge("chunk_material", base::node::MAIN_PASS)
        .unwrap();

    let chunk_material_handle = chunk_materials.add(ChunkMaterial {
        texture_atlas: texture_atlas_handle,
    });
    commands.insert_resource(ChunkMaterialHandle(chunk_material_handle));

    // camera
    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(-2.0, 50.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
            perspective_projection: PerspectiveProjection {
                fov: 1.48353,
                near: 0.05,
                far: 10000.0,
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Character::default());

    // origin
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 0.1 })),
        material: pbr_materials.add(bevy::prelude::Color::rgb(0.1, 0.1, 0.1).into()),
        transform: Transform {
            scale: Vec3::new(1.0, 10000.0, 1.0),
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

/// Grabs/ungrabs mouse cursor
fn toggle_grab_cursor(window: &mut Window) {
    window.set_cursor_lock_mode(!window.cursor_locked());
    window.set_cursor_visibility(!window.cursor_visible());
}

fn character_setup(mut windows: ResMut<Windows>) {
    toggle_grab_cursor(windows.get_primary_mut().unwrap());
}

fn character_system(
    mut commands: Commands,
    mut character: Query<(&mut Transform, &mut Character)>,
    keys: Res<Input<KeyCode>>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    time: Res<Time>,
    mut windows: ResMut<Windows>,
    thread_pool: Res<AsyncComputeTaskPool>,
    mut world: ResMut<Arc<World>>,
    mut chunk_priority_map: ResMut<ChunkPriorityMap>,
    chunk_entitys: Query<(Entity, &ChunkComponent)>,
    chunk_tasks: Query<(Entity, &ChunkTask)>,
) {
    let window = windows.get_primary_mut().unwrap();
    if keys.just_pressed(KeyCode::Escape) {
        toggle_grab_cursor(window);
    }

    if let Ok((mut transform, mut character)) = character.single_mut() {
        let current_chunk = IVec2::new(
            (transform.translation.x / CHUNK_SIZE_X as f32) as i32,
            (transform.translation.z / CHUNK_SIZE_Z as f32) as i32,
        );

        if character.current_chunk != current_chunk {
            character.current_chunk = current_chunk;
            // unload_chunks(
            //     &mut commands,
            //     &mut chunk_priority_map,
            //     chunk_entitys,
            //     chunk_tasks,
            //     &mut character,
            //     &mut world,
            // );

            update_chunk_state(
                current_chunk,
                commands,
                thread_pool,
                world,
                chunk_priority_map,
            );
        }

        if window.cursor_locked() {
            // movement
            let mut input = Vec3::new(
                (keys.pressed(KeyCode::D) as i32 - keys.pressed(KeyCode::A) as i32) as f32,
                (keys.pressed(KeyCode::Space) as i32 - keys.pressed(KeyCode::LShift) as i32) as f32,
                (keys.pressed(KeyCode::S) as i32 - keys.pressed(KeyCode::W) as i32) as f32,
            );
            if input != Vec3::ZERO {
                input.normalize();
            }
            input *= SPEED;
            let target_velocity = input.z * transform.local_z()
                + input.x * transform.local_x()
                + input.y * transform.local_y();
            let delta_time = time.delta_seconds();
            character.velocity = character.velocity
                + (target_velocity - character.velocity) * (1.0 - 0.9f32.powf(delta_time * 120.0));
            transform.translation += character.velocity * delta_time;
            // rotation
            let mut mouse_delta = Vec2::new(0.0, 0.0);
            for event in mouse_motion_events.iter() {
                mouse_delta += event.delta;
            }
            if mouse_delta != Vec2::ZERO {
                let sensitivity = SENSITIVITY;
                character.rotation -= mouse_delta * sensitivity;
                character.rotation.y = character.rotation.y.clamp(-1.54, 1.54);
                // Order is important to prevent unintended roll
                transform.rotation = Quat::from_axis_angle(Vec3::Y, character.rotation.x)
                    * Quat::from_axis_angle(Vec3::X, character.rotation.y);
            }
        }
    }
}

fn update_chunk_state(
    chunk_offset: IVec2,
    mut commands: Commands,
    thread_pool: Res<AsyncComputeTaskPool>,
    world: ResMut<Arc<World>>,
    mut chunk_priority_map: ResMut<ChunkPriorityMap>,
) {
    let view_distance = VIEW_DISTANCE as i32;

    let chunks_to_load = chunk_priority_map.0.get_or_insert_with(|| {
        let mut chunks_to_load = Vec::new();

        for x in -(view_distance - 1)..view_distance {
            for y in -(view_distance - 1)..view_distance {
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

        chunks_to_load
    });

    for local_offset in chunks_to_load {
        let chunk_id = *local_offset + chunk_offset;
        if !world.chunks.contains_key(&chunk_id) && !world.generating_chunks.contains(&chunk_id) {
            world.generating_chunks.insert(chunk_id);

            thread_pool
                .spawn(async_chunk_gen(chunk_id, world.clone()))
                .detach();

            // let task = thread_pool.spawn(async_chunk_mesh(chunk_id, world.clone()));
            // commands.spawn().insert(ChunkTask {
            //     id: chunk_id,
            //     task: task,
            // });
        }
    }
}

async fn async_chunk_gen(chunk_id: IVec2, world: Arc<World>) {
    COUNTER.fetch_add(1, Ordering::Relaxed);

    let mut chunk = Chunk::new(chunk_id);
    chunk.generate(IVec3::new(
        chunk_id.x * CHUNK_SIZE_X as i32,
        0,
        chunk_id.y * CHUNK_SIZE_Z as i32,
    ));

    world.chunks.insert(chunk_id, chunk);
    world.generating_chunks.remove(&chunk_id);

    for dir in neighbors {
        if let Some(mut value) = world.neighbor_count.get_mut(&(chunk_id + dir.into())) {
            *value += 1;
            if *value >= 9 && !world.meshing_chunks.contains(&(chunk_id + dir.into())) {
                world.meshing_chunks.insert(chunk_id + dir.into());
                world.meshing_queue.insert(chunk_id + dir.into());
            }
        } else {
            world.neighbor_count.insert(chunk_id + dir.into(), 1);
        }
    }
}

async fn async_chunk_mesh(chunk_id: IVec2, world: Arc<World>) -> ChunkTaskData {
    COUNTER2.fetch_add(1, Ordering::Relaxed);

    let chunk = world
        .chunks
        .get(&chunk_id)
        .expect("Tried to mesh a chunk that wasn't generated");
    let tmp_mesh = chunk.generate_mesh(&world);

    let mut mesh = Mesh::new(TriangleList);
    mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, tmp_mesh.vertices);
    mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, tmp_mesh.normals);
    mesh.set_attribute("Vertex_UV", tmp_mesh.uvs);
    mesh.set_attribute("Vertex_AO", VertexAttributeValues::from(tmp_mesh.ao));
    mesh.set_indices(Some(Indices::U32(tmp_mesh.indices)));

    ChunkTaskData { mesh: mesh }
}

fn handle_chunk_tasks(
    mut commands: Commands,
    mut completed_chunks: Query<(Entity, &mut ChunkTask)>,
    mut meshes: ResMut<Assets<Mesh>>,
    thread_pool: Res<AsyncComputeTaskPool>,
    material_handle: Res<ChunkMaterialHandle>,
    pipeline_handle: Res<ChunkPipelineHandle>,
    world: Res<Arc<World>>,
) {
    for chunk_id in world.meshing_queue.clone().iter() {
        let task = thread_pool.spawn(async_chunk_mesh(*chunk_id, world.clone()));
        commands.spawn().insert(ChunkTask {
            id: *chunk_id,
            task: task,
        });

        world.meshing_queue.remove(&chunk_id);
    }

    for (entity, mut chunk_task) in completed_chunks.iter_mut() {
        if let Some(chunk_task_data) = future::block_on(future::poll_once(&mut chunk_task.task)) {
            // Add our new PbrBundle of components to our tagged entity
            commands
                .entity(entity)
                .insert_bundle(MeshBundle {
                    mesh: meshes.add(chunk_task_data.mesh),
                    render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
                        pipeline_handle.0.clone(),
                    )]),
                    transform: Transform::from_xyz(
                        chunk_task.id.x as f32 * CHUNK_SIZE_X as f32,
                        0.0,
                        chunk_task.id.y as f32 * CHUNK_SIZE_Z as f32,
                    ),
                    ..Default::default()
                })
                .insert(material_handle.0.clone())
                .insert(ChunkComponent {
                    chunk_id: chunk_task.id,
                });

            // Task is complete, so remove task component from entity
            commands.entity(entity).remove::<ChunkTask>();
        }
    }

    println!("Generated chunks: {:?}", COUNTER);
    println!("Meshed chunks: {:?}", COUNTER2);
}

fn unload_chunks(
    commands: &mut Commands,
    chunk_priority_map: &mut ResMut<ChunkPriorityMap>,
    chunk_entities: Query<(Entity, &ChunkComponent)>,
    chunk_tasks: Query<(Entity, &ChunkTask)>,
    character: &mut Mut<Character>,
    world: &mut ResMut<Arc<World>>,
) {
    if let Some(chunk_priority_map) = &chunk_priority_map.0 {
        println!("Chunks loaded: {:?}", chunk_entities.iter().count());
        println!("Chunks in hashmap: {:?}", world.chunks.len());

        for (entity, chunk) in chunk_entities.iter() {
            if !chunk_priority_map.contains(&(chunk.chunk_id - character.current_chunk)) {
                commands.entity(entity).despawn();
            }
        }

        // let mut oawiehgoaigeh = Vec::new();
        // for chunk in world.chunks.iter() {
        //     if !chunk_priority_map.contains(&(*chunk.key() - character.current_chunk)) {
        //         oawiehgoaigeh.push(*chunk.key());
        //     }
        // }

        // for aweoigh in oawiehgoaigeh {
        //     world.chunks.remove(&aweoigh);
        // }

        for (entity, chunk_task) in chunk_tasks.iter() {
            if !chunk_priority_map.contains(&(chunk_task.id - character.current_chunk)) {
                world.chunks.remove(&chunk_task.id);
                world.generating_chunks.remove(&chunk_task.id);
                commands.entity(entity).despawn();
            }
        }
    }
}
