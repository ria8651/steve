mod ecs;

use ecs::Ecs;

fn main() {
    let mut ecs = Ecs::new();

    let entity_id = ecs.new_entity();
    ecs.add_component_to_entity(entity_id, Health(100));
    ecs.add_component_to_entity(entity_id, Name("Bob"));

    let entity_id = ecs.new_entity();
    ecs.add_component_to_entity(entity_id, Health(80));
    ecs.add_component_to_entity(entity_id, Name("John"));

    let entity_id = ecs.new_entity();
    ecs.add_component_to_entity(entity_id, Name("Stone"));
    {
        let mut vec = ecs.borrow_component_vec::<Health>().unwrap();
        for health in vec.iter_mut() {
            if let Some(health) = health {
                println!("{}", health.0);
                health.0 = 10;
            } else {
                println!("None");
            }
        }
    }

    {
        let mut vec = ecs.borrow_component_vec::<Health>().unwrap();
        for health in vec.iter_mut() {
            if let Some(health) = health {
                println!("{}", health.0);
            } else {
                println!("None");
            }
        }
    }

    // let renderer = renderer::RenderEngine::init();
    // renderer::RenderEngine::game_loop(renderer);
}

struct Health(i32);
struct Name(&'static str);
