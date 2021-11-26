#[derive(Default, Debug, Clone)]
pub struct Vertex {
    pub position: [f32; 3],
}

#[derive(Default, Copy, Clone)]
pub struct Normal {
    pub normal: [f32; 3],
}