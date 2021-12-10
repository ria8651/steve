use bevy::{
    prelude::*,
    render::{
        mesh::{Indices, VertexAttributeValues},
        pipeline::PrimitiveTopology::TriangleList,
    },
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[path = "../src/chunk.rs"]
mod chunk;
use chunk::Chunk;

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("Full Chunk Generation", |b| b.iter(|| chunk_stuff(black_box(0))));
}

fn chunk_stuff(a: i32) {
    let mut chunk = Chunk::new();
    chunk.generate(IVec3::new(a, 0, 0));

    // let tmp_mesh = chunk.generate_mesh();

    // let mut mesh = Mesh::new(TriangleList);
    // mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, tmp_mesh.vertices);
    // mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, tmp_mesh.normals);
    // mesh.set_attribute("Vertex_UV", tmp_mesh.uvs);
    // mesh.set_attribute("Vertex_AO", VertexAttributeValues::from(tmp_mesh.ao));
    // mesh.set_indices(Some(Indices::U32(tmp_mesh.indices)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
