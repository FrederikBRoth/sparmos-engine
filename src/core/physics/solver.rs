use std::cmp::max;

use cgmath::InnerSpace;

use crate::{
    application::graphics::Graphics,
    core::{
        entities::World,
        physics::{
            collision::{Collider, Collision, CollisionPoints},
            rigidbody::{self, BodyType, RigidBody},
        },
        render::{RenderContext, Renderable},
    },
};

pub trait Solver {
    fn solve(&self, world: &mut World, collision: &[Collision], dt: f32, rc: &mut RenderContext);
}
pub struct PositionSolver;

impl Solver for PositionSolver {
    fn solve(&self, world: &mut World, collisions: &[Collision], dt: f32, rc: &mut RenderContext) {
        for collision in collisions {
            let mut query_a = world
                .entities
                .query_one::<(&Renderable, &RigidBody)>(collision.object_a.0);
            let (renderable_a, rigidbody_a) = query_a.get().unwrap();

            let mut query_b = world
                .entities
                .query_one::<(&Renderable, &RigidBody)>(collision.object_b.0);
            let (renderable_b, rigidbody_b) = query_b.get().unwrap();

            let a_static = matches!(rigidbody_a.body_type, BodyType::Static) as usize;
            let b_static = matches!(rigidbody_b.body_type, BodyType::Static) as usize;

            let resolution = collision.collision_points.normal * collision.collision_points.depth
                / max(1, a_static + b_static) as f32;

            let instance_a = rc.gpu_objects.instance_controllers
                [renderable_a.instance_controller_handle]
                .instances_mut()
                .get_mut(collision.object_a.1)
                .unwrap();
            instance_a.transform.position -= resolution * (1 - a_static) as f32;
            let instance_b = rc.gpu_objects.instance_controllers
                [renderable_b.instance_controller_handle]
                .instances_mut()
                .get_mut(collision.object_b.1)
                .unwrap();
            instance_b.transform.position += resolution * (1 - b_static) as f32;
        }
    }
}

pub struct ImpulseSolver;

impl Solver for ImpulseSolver {
    fn solve(&self, world: &mut World, collision: &[Collision], dt: f32, rc: &mut RenderContext) {
        for collision in collision {
            let [a, b] = world
                .entities
                .query_disjoint_mut::<(&Renderable, &mut RigidBody), 2>([
                    collision.object_a.0,
                    collision.object_b.0,
                ]);

            let (renderable_a, body_a) = a.unwrap();
            let (renderable_b, body_b) = b.unwrap();

            let a_inv_mass = body_a.inv_mass();
            let b_inv_mass = body_b.inv_mass();

            let total_inv_mass = a_inv_mass + b_inv_mass;

            if total_inv_mass == 0.0 {
                return;
            }

            let relative_velocity = body_b.velocity - body_a.velocity;

            let normal_speed = relative_velocity.dot(collision.collision_points.normal);

            // Already moving apart
            if normal_speed >= 0.0 {
                return;
            }

            // 0.0 = completely inelastic
            // 1.0 = perfectly elastic
            let restitution = 0.0;

            let j = -(1.0 + restitution) * normal_speed / total_inv_mass;

            let impulse = collision.collision_points.normal * j;

            if matches!(body_a.body_type, BodyType::Dynamic) {
                body_a.velocity -= impulse * a_inv_mass;
            }

            if matches!(body_b.body_type, BodyType::Dynamic) {
                body_b.velocity += impulse * b_inv_mass;
            }
        }
    }
}
