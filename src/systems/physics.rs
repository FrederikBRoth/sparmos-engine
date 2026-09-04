use cgmath::{Vector3, vec3};
use hecs::Entity;
use wgpu::wgt::instance;

use crate::core::{
    engine::{DefaultSystem, GpuBindableSystem},
    entities::World,
    physics::{
        collision::{Collider, Collision},
        rigidbody::{self, BodyType, RigidBody},
        solver::{ImpulseSolver, PositionSolver, Solver},
    },
    render::Renderable,
};
const PHYSICS_DT: f32 = 1.0 / 60.0;

pub struct PhysicsSystem {
    current_dt: f32,
    gravity: Vector3<f32>,
    solvers: Vec<Box<dyn Solver>>,
}

impl PhysicsSystem {
    pub fn new(gravity: Vector3<f32>) -> Self {
        Self {
            current_dt: 0.0,
            gravity,
            solvers: vec![Box::new(PositionSolver), Box::new(ImpulseSolver)],
        }
    }
}

impl DefaultSystem for PhysicsSystem {
    fn run(
        &mut self,
        world: &mut World,
        resources: &mut crate::core::render::RenderContext,
        dt: std::time::Duration,
    ) {
        self.current_dt += dt.as_secs_f32();

        while self.current_dt >= PHYSICS_DT {
            let mut collisions: Vec<Collision> = vec![];
            world.query::<(Entity, &Renderable, &Collider, &RigidBody)>(|mut query| {
                for (entity_a, renderable_a, collider_a, rigidbody_a) in query.iter() {
                    for (i_a, instance_a) in resources.gpu_objects.instance_controllers
                        [renderable_a.instance_controller_handle]
                        .instances()
                        .iter()
                        .enumerate()
                    {
                        world.query::<(Entity, &Renderable, &Collider, &RigidBody)>(|mut query| {
                            for (entity_b, renderable_b, collider_b, rigidbody_b) in query.iter() {
                                for (i_b, instance_b) in resources.gpu_objects.instance_controllers
                                    [renderable_b.instance_controller_handle]
                                    .instances()
                                    .iter()
                                    .enumerate()
                                {
                                    if entity_a == entity_b {
                                        continue;
                                    }

                                    let points = Collider::collision(
                                        collider_a,
                                        &instance_a.transform,
                                        collider_b,
                                        &instance_b.transform,
                                    );

                                    if (points.has_collision) {
                                        collisions.push(Collision {
                                            object_a: (entity_a, i_a),
                                            object_b: (entity_b, i_b),
                                            collision_points: points,
                                        });
                                        println!("COLLISION!!!!")
                                    }
                                }
                            }
                        });
                    }
                }
            });

            for solver in self.solvers.iter() {
                solver.solve(world, &collisions, dt.as_secs_f32(), resources);
            }

            world.query::<(Entity, &Renderable, &Collider, &mut RigidBody)>(|mut query| {
                for (_, renderable, collider, rigidbody) in query.iter() {
                    if matches!(rigidbody.body_type, BodyType::Static) {
                        continue;
                    }
                    for instance in resources.gpu_objects.instance_controllers
                        [renderable.instance_controller_handle]
                        .instances_mut()
                    {
                        rigidbody.force += rigidbody.mass * self.gravity;
                        rigidbody.velocity += rigidbody.force / rigidbody.mass * PHYSICS_DT;

                        instance.transform.position += rigidbody.velocity * PHYSICS_DT;
                        rigidbody.force = vec3(0.0, 0.0, 0.0);
                    }
                }
            });

            self.current_dt -= PHYSICS_DT
        }
    }
}
