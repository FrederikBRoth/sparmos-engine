use cgmath::{InnerSpace, Vector3};

use crate::core::{
    entities::World,
    instance::Transform,
    physics::{
        collision::Collision,
        rigidbody::{BodyType, RigidBody},
    },
};

pub trait Solver {
    fn solve(&self, world: &mut World, collision: &[Collision], dt: f32);
}
pub struct PositionSolver;

impl Solver for PositionSolver {
    fn solve(&self, world: &mut World, collisions: &[Collision], _dt: f32) {
        for collision in collisions {
            let [a, b] = world
                .entities
                .query_disjoint_mut::<(&mut Transform, &RigidBody), 2>([
                    collision.object_a,
                    collision.object_b,
                ]);
            let (transform_a, rigidbody_a) = a.expect("collision A needs Transform and RigidBody");
            let (transform_b, rigidbody_b) = b.expect("collision B needs Transform and RigidBody");

            let Some((movement_a, movement_b)) = position_movements(
                collision.collision_points.normal,
                collision.collision_points.depth,
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

pub struct ImpulseSolver;

impl Solver for ImpulseSolver {
    fn solve(&self, world: &mut World, collisions: &[Collision], _dt: f32) {
        for collision in collisions {
            let [a, b] = world
                .entities
                .query_disjoint_mut::<&mut RigidBody, 2>([collision.object_a, collision.object_b]);

            let body_a = a.expect("collision A needs RigidBody");
            let body_b = b.expect("collision B needs RigidBody");

            let a_inv_mass = body_a.inv_mass();
            let b_inv_mass = body_b.inv_mass();

            let total_inv_mass = a_inv_mass + b_inv_mass;

            if total_inv_mass == 0.0 {
                continue;
            }

            let relative_velocity = body_b.velocity - body_a.velocity;

            // 0.0 = completely inelastic
            // 1.0 = perfectly elastic
            let restitution = 0.0;

            let Some(impulse) = contact_impulse(
                relative_velocity,
                collision.collision_points.normal,
                restitution,
                total_inv_mass,
            ) else {
                continue;
            };

            if matches!(body_a.body_type, BodyType::Dynamic) {
                body_a.velocity -= impulse * a_inv_mass;
            }

            if matches!(body_b.body_type, BodyType::Dynamic) {
                body_b.velocity += impulse * b_inv_mass;
            }
        }
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

    let correction = normal * depth;
    Some((
        -correction * (inv_mass_a / total_inv_mass),
        correction * (inv_mass_b / total_inv_mass),
    ))
}

fn contact_impulse(
    relative_velocity: Vector3<f32>,
    normal: Vector3<f32>,
    restitution: f32,
    total_inv_mass: f32,
) -> Option<Vector3<f32>> {
    if total_inv_mass == 0.0 {
        return None;
    }

    let normal_speed = relative_velocity.dot(normal);
    if normal_speed >= 0.0 {
        return None;
    }

    let magnitude = -(1.0 + restitution) * normal_speed / total_inv_mass;
    Some(normal * magnitude)
}
