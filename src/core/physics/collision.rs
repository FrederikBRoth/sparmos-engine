use web_time::Instant;

use cgmath::{EuclideanSpace, InnerSpace, Point3, Rotation, Vector3};
use hecs::Entity;

use crate::{
    application::graphics::Graphics,
    core::{entities::World, instance::Transform, render::Renderable},
};

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub entity_handle: Entity,
    pub instance_index: usize,
    pub distance: f32,
}

pub struct Collision {
    pub object_a: (Entity, usize),
    pub object_b: (Entity, usize),
    pub collision_points: CollisionPoints,
}

/// A world-space contact manifold for objects A and B.
///
/// For a valid contact, `a` lies on A, `b` lies on B, `normal` is normalized
/// and points from A toward B, and `depth` is the non-negative penetration.
#[derive(Clone, Copy, Debug)]
pub struct CollisionPoints {
    pub a: Vector3<f32>,
    pub b: Vector3<f32>,
    pub normal: Vector3<f32>,
    pub depth: f32,
    pub has_collision: bool,
}

impl Default for CollisionPoints {
    fn default() -> Self {
        Self {
            a: [0.0, 0.0, 0.0].into(),
            b: [0.0, 0.0, 0.0].into(),
            normal: [0.0, 0.0, 0.0].into(),
            depth: Default::default(),
            has_collision: Default::default(),
        }
    }
}

impl CollisionPoints {
    pub fn new(a: Vector3<f32>, b: Vector3<f32>) -> Self {
        let ba = a - b;
        let depth = ba.magnitude();

        let normal = if depth > 0.00001 {
            ba / depth
        } else {
            Vector3::unit_y()
        };

        Self::new_with_normal_and_depth(a, b, normal, depth)
    }

    pub fn new_with_normal_and_depth(
        a: Vector3<f32>,
        b: Vector3<f32>,
        normal: Vector3<f32>,
        depth: f32,
    ) -> Self {
        debug_assert!(depth >= 0.0);
        debug_assert!((normal.magnitude2() - 1.0).abs() < 0.0001);

        Self {
            a,
            b,
            normal,
            depth,
            has_collision: true,
        }
    }
}

type CollisionFn =
    fn(a: &Collider, at: &Transform, b: &Collider, bt: &Transform) -> Result<CollisionPoints, ()>;

#[rustfmt::skip]
const COLLISION_TABLE: [[Option<CollisionFn>; 2]; 2] = [
    //Sphere                  Plane
    [Some(sphere_sphere_collision), Some(sphere_plane_collision)],
    [None, None],
];
#[derive(Clone, Debug)]
pub enum Collider {
    /// Full local extents from the voxel's minimum corner at the origin.
    Box {
        extents: Vector3<f32>,
    },
    Sphere {
        radius: f32,
    },
    Capsule {
        radius: f32,
        half_height: f32,
    },
    Plane,
}

impl Collider {
    /// Bounds for axis-aligned instances with uniform scale.
    pub(crate) fn aabb(&self, transform: &Transform) -> Aabb {
        let magnitude = transform.scale.abs();
        match self {
            Collider::Box { extents } => {
                let world_extents = *extents * magnitude;
                // Mirroring a corner-relative mesh moves its minimum corner.
                let min = if transform.scale < 0.0 {
                    transform.position - world_extents
                } else {
                    transform.position
                };
                Aabb::from_box(min, world_extents)
            }
            Collider::Sphere { radius } => {
                Aabb::from_sphere(transform.position, *radius * magnitude)
            }
            Collider::Capsule {
                radius,
                half_height,
            } => Aabb::from_capsule(
                transform.position,
                *radius * magnitude,
                *half_height * magnitude,
            ),
            Collider::Plane => Aabb::from_plane(transform.position, transform.scale),
        }
    }

    fn precise_ray_intersection(&self, ray: &Ray, transform: &Transform) -> Option<f32> {
        match self {
            Collider::Box { .. } => ray_aabb(ray, &self.aabb(transform)),
            Collider::Sphere { radius } => ray_sphere(
                ray,
                Point3::from_vec(transform.position),
                *radius * transform.scale.abs(),
            ),
            Collider::Capsule { .. } => todo!(),
            Collider::Plane => ray_plane(ray, transform),
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Collider::Sphere { .. } => 0,
            Collider::Plane => 1,

            Collider::Capsule { .. } => 2,
            Collider::Box { .. } => 3,
        }
    }

    pub(crate) fn collision(
        a: &Collider,
        at: &Transform,
        b: &Collider,
        bt: &Transform,
    ) -> CollisionPoints {
        let do_swap = a.index() > b.index();

        let (a, at, b, bt) = if do_swap {
            (b, bt, a, at)
        } else {
            (a, at, b, bt)
        };

        let Some(collision_fn) = COLLISION_TABLE
            .get(a.index())
            .and_then(|row| row.get(b.index()))
            .and_then(|collision_fn| *collision_fn)
        else {
            return CollisionPoints::default();
        };

        let Ok(mut collision_points) = collision_fn(a, at, b, bt) else {
            return CollisionPoints::default();
        };

        if do_swap {
            std::mem::swap(&mut collision_points.a, &mut collision_points.b);
            collision_points.normal = -collision_points.normal;
        }

        collision_points
    }
}

fn sphere_sphere_collision(
    a: &Collider,
    at: &Transform,
    b: &Collider,
    bt: &Transform,
) -> Result<CollisionPoints, ()> {
    if let Collider::Sphere { radius: radius_a } = a
        && let Collider::Sphere { radius: radius_b } = b
    {
        let ab = bt.position - at.position;

        let a_radius = (radius_a * at.scale).abs();
        let b_radius = (radius_b * bt.scale).abs();

        let distance = ab.magnitude();

        if distance < 0.00001 || distance > a_radius + b_radius {
            return Ok(CollisionPoints::default());
        }

        let normal = ab.normalize();

        let point_a = at.position + normal * a_radius;
        let point_b = bt.position - normal * b_radius;
        let depth = a_radius + b_radius - distance;

        Ok(CollisionPoints::new_with_normal_and_depth(
            point_a, point_b, normal, depth,
        ))
    } else {
        println!("Collider A is not a sphere and/or collider B is not a sphere");
        Err(())
    }
}

fn sphere_plane_collision(
    a: &Collider,
    at: &Transform,
    b: &Collider,
    bt: &Transform,
) -> Result<CollisionPoints, ()> {
    if let Collider::Sphere { radius: radius_a } = a
        && let Collider::Plane = b
    {
        let a_radius = (radius_a * at.scale).abs();

        let plane_normal = bt.rotation.rotate_vector(Vector3::unit_y()).normalize();

        let signed_distance = (at.position - bt.position).dot(plane_normal);

        if signed_distance > a_radius {
            return Ok(CollisionPoints::default());
        }

        let point_a = at.position - plane_normal * a_radius;
        let point_b = at.position - plane_normal * signed_distance;
        let contact_normal = -plane_normal;
        let penetration_depth = a_radius - signed_distance;

        Ok(CollisionPoints::new_with_normal_and_depth(
            point_a,
            point_b,
            contact_normal,
            penetration_depth,
        ))
    } else {
        println!("Collider A is not a sphere and/or collider B is not a sphere");
        Err(())
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

    /// Build a box from its minimum corner and full extents.
    pub fn from_box(position: Vector3<f32>, extents: Vector3<f32>) -> Self {
        Self {
            min: position,
            max: position + extents,
        }
    }

    pub fn from_plane(position: Vector3<f32>, size: f32) -> Aabb {
        let half = size * 0.5;
        let thickness = 0.001;

        Self {
            min: cgmath::Vector3::new(position.x - half, position.y - thickness, position.z - half),
            max: cgmath::Vector3::new(position.x + half, position.y + thickness, position.z + half),
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
    pub fn broad_intersects<'a>(&self, world: &World, gfx: &mut Graphics) -> Option<RayHit> {
        let mut closest: Option<RayHit> = None;

        world.query::<(Entity, (&Renderable, &Collider))>(|mut query| {
            for (entity, (r, c)) in query.iter() {
                let ic = gfx
                    .engine
                    .get_instance_controller(&r.instance_controller_handle);
                for (i, instance) in ic.instances().iter().enumerate() {
                    if !instance.should_render {
                        continue;
                    }
                    let aabb = c.aabb(&instance.transform);
                    if let Some(distance) = ray_aabb(self, &aabb) {
                        if closest
                            .map(|collision| distance < collision.distance)
                            .unwrap_or(true)
                        {
                            closest = Some(RayHit {
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

    pub fn precise_intersects<'a>(&self, world: &World, gfx: &mut Graphics) -> Option<RayHit> {
        let now = Instant::now();
        let mut broad_hits: Vec<RayHit> = vec![];

        world.query::<(Entity, (&Renderable, &Collider))>(|mut query| {
            for (entity, (r, c)) in query.iter() {
                let ic = gfx
                    .engine
                    .get_instance_controller(&r.instance_controller_handle);
                for (i, instance) in ic.instances().iter().enumerate() {
                    if !instance.should_render {
                        continue;
                    }

                    let aabb = c.aabb(&instance.transform);
                    if let Some(distance) = ray_aabb(self, &aabb) {
                        broad_hits.push(RayHit {
                            entity_handle: entity,
                            instance_index: i,
                            distance,
                        });
                    }
                }
            }
        });

        broad_hits.sort_by(|a, b| a.distance.total_cmp(&b.distance));

        let mut closest: Option<RayHit> = None;
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

            if let Some(distance) = collider.precise_ray_intersection(self, &instance.transform) {
                if closest
                    .map(|collision| distance < collision.distance)
                    .unwrap_or(true)
                {
                    closest = Some(RayHit {
                        entity_handle: entity.entity_handle,
                        instance_index: entity.instance_index,
                        distance,
                    });
                }
            }
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

fn ray_plane(ray: &Ray, transform: &Transform) -> Option<f32> {
    let plane_normal = transform
        .rotation
        .rotate_vector(Vector3::unit_y())
        .normalize();
    println!("{:?}", plane_normal);
    None
}
