use cgmath::{Vector3, vec3};
use hecs::Entity;

use crate::core::{
    engine::DefaultSystem,
    entities::World,
    physics::{
        collision::{Collider, Collision},
        rigidbody::{BodyType, RigidBody},
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
            solvers: vec![Box::new(ImpulseSolver), Box::new(PositionSolver)],
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
            world.query::<(&Renderable, &mut RigidBody)>(|mut query| {
                for (renderable, rigidbody) in query.iter() {
                    let instances = resources.gpu_objects.instance_controllers
                        [renderable.instance_controller_handle]
                        .instances_mut();
                    debug_assert_eq!(
                        instances.len(),
                        1,
                        "physics entities currently require exactly one instance"
                    );

                    if matches!(rigidbody.body_type, BodyType::Dynamic) {
                        let acceleration = self.gravity + rigidbody.force * rigidbody.inv_mass();
                        rigidbody.velocity += acceleration * PHYSICS_DT;

                        if let Some(instance) = instances.first_mut() {
                            instance.transform.position += rigidbody.velocity * PHYSICS_DT;
                        }
                    }

                    rigidbody.force = vec3(0.0, 0.0, 0.0);
                }
            });

            let mut candidates = vec![];
            world.query::<(Entity, &Renderable, &Collider, &RigidBody)>(|mut query| {
                for (entity, renderable, collider, rigidbody) in query.iter() {
                    let instances = resources.gpu_objects.instance_controllers
                        [renderable.instance_controller_handle]
                        .instances();
                    debug_assert_eq!(
                        instances.len(),
                        1,
                        "physics entities currently require exactly one instance"
                    );

                    if let Some(instance) = instances.first() {
                        candidates.push(PhysicsCandidate {
                            entity,
                            collider: collider.clone(),
                            transform: instance.transform.clone(),
                            is_static: matches!(rigidbody.body_type, BodyType::Static),
                        });
                    }
                }
            });

            let mut collisions: Vec<Collision> = vec![];
            for (i, j) in unordered_pair_indices(candidates.len()) {
                let a = &candidates[i];
                let b = &candidates[j];
                if a.is_static && b.is_static {
                    continue;
                }

                let points =
                    Collider::collision(&a.collider, &a.transform, &b.collider, &b.transform);
                if points.has_collision {
                    collisions.push(Collision {
                        object_a: (a.entity, 0),
                        object_b: (b.entity, 0),
                        collision_points: points,
                    });
                }
            }

            for solver in self.solvers.iter() {
                solver.solve(world, &collisions, PHYSICS_DT, resources);
            }

            self.current_dt -= PHYSICS_DT
        }
    }
}

struct PhysicsCandidate {
    entity: Entity,
    collider: Collider,
    transform: crate::core::instance::Transform,
    is_static: bool,
}

fn unordered_pair_indices(len: usize) -> impl Iterator<Item = (usize, usize)> {
    (0..len).flat_map(move |i| ((i + 1)..len).map(move |j| (i, j)))
}
