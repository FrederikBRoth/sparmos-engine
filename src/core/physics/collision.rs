use std::time::Instant;

use cgmath::{EuclideanSpace, InnerSpace, Point3, Vector3};
use hecs::Entity;

use crate::{
    application::graphics::Graphics,
    core::{
        entities::World,
        render::{MaterialHandle, Renderable},
    },
};

#[derive(Clone, Copy, Debug)]
pub struct Collision {
    pub entity_handle: Entity,
    pub instance_index: usize,
    pub distance: f32,
}

#[derive(Clone, Debug)]
pub enum Collider {
    Box { half_extents: Vector3<f32> },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
}

impl Collider {
    pub(crate) fn aabb(&self, position: Vector3<f32>) -> Aabb {
        match self {
            Collider::Box { half_extents } => Aabb::from_box(position, half_extents.clone()),
            Collider::Sphere { radius } => Aabb::from_sphere(position, radius.clone()),
            Collider::Capsule {
                radius,
                half_height,
            } => Aabb::from_capsule(position, radius.clone(), half_height.clone()),
        }
    }
}

pub struct Aabb {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
}

impl Aabb {
    pub fn from_capsule(position: Vector3<f32>, radius: f32, half_height: f32) -> Self {
        let extents = Vector3::new(radius, half_height + radius, radius);

        Self {
            min: position - extents,
            max: position + extents,
        }
    }
    pub fn from_sphere(position: Vector3<f32>, radius: f32) -> Self {
        let r = Vector3::new(radius, radius, radius);

        Self {
            min: position - r,
            max: position + r,
        }
    }

    //position is the cornor, not middle
    pub fn from_box(center: Vector3<f32>, extents: Vector3<f32>) -> Self {
        Self {
            min: center,
            max: center + extents,
        }
    }

    pub fn intersects_aabb(a: &Aabb, b: &Aabb) -> bool {
        a.min.x <= b.max.x
            && a.max.x >= b.min.x
            && a.min.y <= b.max.y
            && a.max.y >= b.min.y
            && a.min.z <= b.max.z
            && a.max.z >= b.min.z
    }
}

pub struct Ray {
    pub origin: Point3<f32>,
    pub direction: Vector3<f32>,
}

impl Ray {
    //Only uses AABB collision for checks. Will not be completely precise for more complex colliders
    pub fn broad_intersects<'a>(&self, world: &World, gfx: &mut Graphics) -> Option<Collision> {
        let mut closest: Option<Collision> = None;

        world.query::<(Entity, (&Renderable, &Collider))>(|mut query| {
            for (entity, (r, c)) in query.iter() {
                let ic = gfx
                    .engine
                    .get_instance_controller(&r.instance_controller_handle);
                for (i, instance) in ic.instances().iter().enumerate() {
                    if !instance.should_render {
                        continue;
                    }
                    let aabb = c.aabb(instance.position);
                    if let Some(distance) = ray_aabb(self, &aabb) {
                        if closest
                            .map(|collision| distance < collision.distance)
                            .unwrap_or(true)
                        {
                            closest = Some(Collision {
                                entity_handle: entity,
                                instance_index: i,
                                distance,
                            });
                        }
                    }
                }
            }
        });

        closest
    }

    pub fn precise_intersects<'a>(&self, world: &World, gfx: &mut Graphics) -> Option<Collision> {
        let now = Instant::now();
        let mut broad_hits: Vec<Collision> = vec![];

        world.query::<(Entity, (&Renderable, &Collider))>(|mut query| {
            for (entity, (r, c)) in query.iter() {
                let ic = gfx
                    .engine
                    .get_instance_controller(&r.instance_controller_handle);
                for (i, instance) in ic.instances().iter().enumerate() {
                    if !instance.should_render {
                        continue;
                    }

                    let aabb = c.aabb(instance.position);
                    if let Some(distance) = ray_aabb(self, &aabb) {
                        broad_hits.push(Collision {
                            entity_handle: entity,
                            instance_index: i,
                            distance,
                        });
                    }
                }
            }
        });

        broad_hits.sort_by(|a, b| a.distance.total_cmp(&b.distance));

        let mut closest: Option<Collision> = None;
        for entity in broad_hits {
            let mut query = world
                .entities
                .query_one::<(&Renderable, &Collider)>(entity.entity_handle);

            let Ok((renderable, collider)) = query.get() else {
                continue;
            };

            let instance = gfx
                .engine
                .get_instance_controller(&renderable.instance_controller_handle)
                .instances()[entity.instance_index]
                .clone();

            if let Some(distance) = match collider {
                Collider::Box { half_extents } => ray_aabb(self, &collider.aabb(instance.position)),
                Collider::Sphere { radius } => {
                    ray_sphere(self, Point3::from_vec(instance.position), *radius)
                }
                Collider::Capsule {
                    radius,
                    half_height,
                } => todo!(),
            } {
                if closest
                    .map(|collision| distance < collision.distance)
                    .unwrap_or(true)
                {
                    closest = Some(Collision {
                        entity_handle: entity.entity_handle,
                        instance_index: entity.instance_index,
                        distance,
                    });
                }
            }

            // precise test
        }
        let elapsed = now.elapsed().as_micros();
        println!("Raytrace: Finish {:?}", elapsed);
        closest
    }
}

pub fn ray_aabb(ray: &Ray, aabb: &Aabb) -> Option<f32> {
    let inv_dir = Vector3::new(
        1.0 / ray.direction.x,
        1.0 / ray.direction.y,
        1.0 / ray.direction.z,
    );

    let t1 = (aabb.min.x - ray.origin.x) * inv_dir.x;
    let t2 = (aabb.max.x - ray.origin.x) * inv_dir.x;

    let t3 = (aabb.min.y - ray.origin.y) * inv_dir.y;
    let t4 = (aabb.max.y - ray.origin.y) * inv_dir.y;

    let t5 = (aabb.min.z - ray.origin.z) * inv_dir.z;
    let t6 = (aabb.max.z - ray.origin.z) * inv_dir.z;

    let t_min = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));

    let t_max = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

    if t_max < 0.0 || t_min > t_max {
        None
    } else {
        Some(t_min.max(0.0))
    }
}

pub fn ray_sphere(ray: &Ray, position: Point3<f32>, radius: f32) -> Option<f32> {
    let oc = ray.origin - position;

    let a = ray.direction.dot(ray.direction);
    let b = 2.0 * oc.dot(ray.direction);
    let c = oc.dot(oc) - radius * radius;

    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_discriminant = discriminant.sqrt();

    let t0 = (-b - sqrt_discriminant) / (2.0 * a);
    let t1 = (-b + sqrt_discriminant) / (2.0 * a);

    if t0 >= 0.0 {
        Some(t0)
    } else if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

pub fn ray_triangle(
    ray: &Ray,
    v0: Vector3<f32>,
    v1: Vector3<f32>,
    v2: Vector3<f32>,
) -> Option<f32> {
    let epsilon = 0.000001;

    let edge1 = v1 - v0;
    let edge2 = v2 - v0;

    let h = ray.direction.cross(edge2);
    let a = edge1.dot(h);

    if a.abs() < epsilon {
        return None;
    }

    let f = 1.0 / a;
    let s = ray.origin.to_vec() - v0;

    let u = f * s.dot(h);

    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(edge1);
    let v = f * ray.direction.dot(q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * edge2.dot(q);

    if t > epsilon { Some(t) } else { None }
}
