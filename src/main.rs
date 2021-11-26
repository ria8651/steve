use legion::*;

fn main() {
    let mut world = World::default();

    world.push((Health(100), Name("Bob")));
    world.push((Health(80), Name("Larry")));
    world.push((Name("Rock"),));

    // construct a query from a "view tuple"
    let mut query = <(&Health, &Name)>::query();

    // this time we have &Velocity and &mut Position
    for (health, name) in query.iter_mut(&mut world) {
        println!("{} has health {}", name.0, health.0);
    }

    // construct a query from a "view tuple"
    let mut query = <(&Name,)>::query();

    // this time we have &Velocity and &mut Position
    for (name,) in query.iter_mut(&mut world) {
        println!("{}", name.0);
    }

    // let renderer = renderer::RenderEngine::init();
    // renderer::RenderEngine::game_loop(renderer);
}

struct Health(i32);
struct Name(&'static str);
