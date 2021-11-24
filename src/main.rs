mod renderer;

use std::sync::Arc;

fn main() {
    let renderer = renderer::RenderEngine::init();
    renderer::RenderEngine::game_loop(renderer);
}
