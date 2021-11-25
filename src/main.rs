mod renderer;

fn main() {
    let renderer = renderer::RenderEngine::init();
    renderer::RenderEngine::game_loop(renderer);
}
