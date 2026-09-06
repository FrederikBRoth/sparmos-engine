use std::collections::{HashMap, HashSet};

use cgmath::{InnerSpace, Vector3};
use hecs::Entity;

use crate::core::{
    entities::World,
    instance::Transform,
    physics::{
        collision::{Collider, Collision},
        rigidbody::{BodyType, RigidBody},
    },
};

pub const POSITION_SLOP: f32 = 0.001;
pub const POSITION_PERCENT: f32 = 0.2;

pub trait Solver {
    fn solve(&mut self, world: &mut World, collision: &[Collision], dt: f32);
}
pub struct PositionSolver;

impl Solver for PositionSolver {
    fn solve(&mut self, world: &mut World, collisions: &[Collision], _dt: f32) {
        for collision in collisions {
            let [a, b] = world
                .entities
                .query_disjoint_mut::<(&mut Transform, &Collider, &RigidBody), 2>([
                    collision.object_a,
                    collision.object_b,
                ]);
            let (transform_a, collider_a, rigidbody_a) =
                a.expect("collision A needs Transform, Collider, and RigidBody");
            let (transform_b, collider_b, rigidbody_b) =
                b.expect("collision B needs Transform, Collider, and RigidBody");

            let points = Collider::collision(collider_a, transform_a, collider_b, transform_b);
            if !points.has_collision {
                continue;
            }

            let Some((movement_a, movement_b)) = position_movements(
                points.normal,
                points.depth,
                rigidbody_a.inv_mass(),
                rigidbody_b.inv_mass(),
            ) else {
                continue;
            };

            transform_a.position += movement_a;
            transform_b.position += movement_b;
        }
    }
}

#[derive(Default)]
pub struct ImpulseSolver {
    accumulated_impulses: HashMap<(Entity, Entity), f32>,
}

impl ImpulseSolver {
    pub fn prepare(&mut self, world: &mut World, collisions: &[Collision]) {
        let active_contacts: HashSet<_> = collisions.iter().map(contact_key).collect();
        self.accumulated_impulses
            .retain(|key, _| active_contacts.contains(key));

        for collision in collisions {
            let magnitude = self
                .accumulated_impulses
                .get(&contact_key(collision))
                .copied()
                .unwrap_or(0.0);
            if magnitude == 0.0 {
                continue;
            }

            let [a, b] = world
                .entities
                .query_disjoint_mut::<&mut RigidBody, 2>([collision.object_a, collision.object_b]);
            let body_a = a.expect("collision A needs RigidBody");
            let body_b = b.expect("collision B needs RigidBody");
            let impulse = collision.collision_points.normal * magnitude;

            if matches!(body_a.body_type, BodyType::Dynamic) {
                body_a.velocity -= impulse * body_a.inv_mass();
            }
            if matches!(body_b.body_type, BodyType::Dynamic) {
                body_b.velocity += impulse * body_b.inv_mass();
            }
        }
    }
}

impl Solver for ImpulseSolver {
    fn solve(&mut self, world: &mut World, collisions: &[Collision], _dt: f32) {
        for collision in collisions {
            let key = contact_key(collision);
            let old_magnitude = self.accumulated_impulses.get(&key).copied().unwrap_or(0.0);

            let Some(new_magnitude) = ({
                let [a, b] = world.entities.query_disjoint_mut::<&mut RigidBody, 2>([
                    collision.object_a,
                    collision.object_b,
                ]);

                let body_a = a.expect("collision A needs RigidBody");
                let body_b = b.expect("collision B needs RigidBody");
                let a_inv_mass = body_a.inv_mass();
                let b_inv_mass = body_b.inv_mass();
                let total_inv_mass = a_inv_mass + b_inv_mass;

                if total_inv_mass == 0.0 {
                    None
                } else {
                    let relative_velocity = body_b.velocity - body_a.velocity;
                    let normal_speed = relative_velocity.dot(collision.collision_points.normal);
                    let new_magnitude = (old_magnitude - normal_speed / total_inv_mass).max(0.0);
                    let impulse =
                        collision.collision_points.normal * (new_magnitude - old_magnitude);

                    if matches!(body_a.body_type, BodyType::Dynamic) {
                        body_a.velocity -= impulse * a_inv_mass;
                    }

                    if matches!(body_b.body_type, BodyType::Dynamic) {
                        body_b.velocity += impulse * b_inv_mass;
                    }

                    Some(new_magnitude)
                }
            }) else {
                continue;
            };

            self.accumulated_impulses.insert(key, new_magnitude);
        }
    }
}

fn contact_key(collision: &Collision) -> (Entity, Entity) {
    if collision.object_a < collision.object_b {
        (collision.object_a, collision.object_b)
    } else {
        (collision.object_b, collision.object_a)
    }
}

fn position_movements(
    normal: Vector3<f32>,
    depth: f32,
    inv_mass_a: f32,
    inv_mass_b: f32,
) -> Option<(Vector3<f32>, Vector3<f32>)> {
    let total_inv_mass = inv_mass_a + inv_mass_b;
    if total_inv_mass == 0.0 {
        return None;
    }

    let correction_depth = (depth - POSITION_SLOP).max(0.0) * POSITION_PERCENT;
    let correction = normal * correction_depth;
    Some((
        -correction * (inv_mass_a / total_inv_mass),
        correction * (inv_mass_b / total_inv_mass),
    ))
}
