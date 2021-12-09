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

mod chunk;
use chunk::*;

const CHUNK_SIZE_X: usize = 32;
const CHUNK_SIZE_Y: usize = 96;
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
            speed: 50.0,           // default: 12.0
        })
        .add_startup_system(setup.system())
        .add_startup_system(spawn_chunk_tasks.system())
        .add_system(fps_system.system())
        .add_system(handle_chunk_tasks.system())
        .run();
}

#[derive(RenderResources, Default, TypeUuid)]
#[uuid = "0320b9b8-b3a3-4baa-8bfa-c94008177b17"]
struct ChunkMaterial {
    texture_atlas: Handle<Texture>,
}
struct ChunkMaterialHandle(Handle<ChunkMaterial>);
struct ChunkPipelineHandle(Handle<PipelineDescriptor>);

#[derive(RenderResources, Default, TypeUuid)]
#[uuid = "93fb26fc-6c05-489b-9029-601edf703b6b"]
struct TextureAtlas {
    texture: Handle<Texture>,
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
    // Start loading the texture.
    let texture_atlas_handle = asset_server.load("textures/terrain.png");

    // Create a new shader pipeline
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
        .insert(FlyCam);
    // origin
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 0.5 })),
        material: pbr_materials.add(bevy::prelude::Color::rgb(0.1, 0.1, 0.1).into()),
        transform: Transform::from_xyz(0.0, 0.0, 0.0),
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
            mesh.set_attribute("Vertex_UV", tmp_mesh.uvs);
            mesh.set_attribute("Vertex_AO", VertexAttributeValues::from(tmp_mesh.ao));
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

struct ChunkTaskData {
    chunk_id: IVec2,
    mesh: Mesh,
}
