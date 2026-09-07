use std::collections::HashSet;

use cgmath::{EuclideanSpace, InnerSpace, Point3, Rotation, Vector3};
use hecs::Entity;

use crate::{
    application::graphics::Graphics,
    core::{
        entities::World,
        instance::Transform,
        render::{GpuObjects, RenderInstanceRef, RenderableHandle},
    },
};

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub entity_handle: Entity,
    pub instance_index: usize,
    pub distance: f32,
}

pub struct Collision {
    pub object_a: Entity,
    pub object_b: Entity,
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
    /// Bounds for axis-aligned instances.
    pub(crate) fn aabb(&self, transform: &Transform) -> Aabb {
        let scale = transform.scale;
        let magnitude = abs_scale(scale);
        match self {
            Collider::Box { extents } => {
                let signed_extents = component_mul(*extents, scale);
                let local_min = component_min(signed_extents, Vector3::new(0.0, 0.0, 0.0));
                let local_max = component_max(signed_extents, Vector3::new(0.0, 0.0, 0.0));
                Aabb::from_box(transform.position + local_min, local_max - local_min)
            }
            Collider::Sphere { radius } => {
                Aabb::from_sphere(transform.position, *radius * max_component(magnitude))
            }
            Collider::Capsule {
                radius,
                half_height,
            } => Aabb::from_capsule(
                transform.position,
                *radius * magnitude.x.max(magnitude.z),
                *half_height * magnitude.y,
            ),
            Collider::Plane => Aabb::from_plane(transform),
        }
    }

    fn precise_ray_intersection(&self, ray: &Ray, transform: &Transform) -> Option<f32> {
        match self {
            Collider::Box { .. } => ray_aabb(ray, &self.aabb(transform)),
            Collider::Sphere { radius } => ray_sphere(
                ray,
                Point3::from_vec(transform.position),
                *radius * max_component(abs_scale(transform.scale)),
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

        let a_radius = radius_a * max_component(abs_scale(at.scale));
        let b_radius = radius_b * max_component(abs_scale(bt.scale));

        let distance = ab.magnitude();

        if distance > a_radius + b_radius {
            return Ok(CollisionPoints::default());
        }

        let normal = if distance > 0.00001 {
            ab / distance
        } else {
            Vector3::unit_y()
        };

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
        let a_radius = radius_a * max_component(abs_scale(at.scale));

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

    pub fn from_plane(transform: &Transform) -> Aabb {
        let half_x = transform.scale.x * 0.5;
        let half_z = transform.scale.z * 0.5;
        let corners = [
            Vector3::new(-half_x, 0.0, -half_z),
            Vector3::new(half_x, 0.0, -half_z),
            Vector3::new(-half_x, 0.0, half_z),
            Vector3::new(half_x, 0.0, half_z),
        ];

        let first = transform.position + transform.rotation.rotate_vector(corners[0]);
        let mut min = first;
        let mut max = first;
        for corner in corners.into_iter().skip(1) {
            let world_corner = transform.position + transform.rotation.rotate_vector(corner);
            min = component_min(min, world_corner);
            max = component_max(max, world_corner);
        }

        let thickness = abs_scale(
            transform
                .rotation
                .rotate_vector(Vector3::unit_y())
                .normalize(),
        ) * 0.001;

        Self {
            min: min - thickness,
            max: max + thickness,
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

fn abs_scale(scale: Vector3<f32>) -> Vector3<f32> {
    Vector3::new(scale.x.abs(), scale.y.abs(), scale.z.abs())
}

fn max_component(value: Vector3<f32>) -> f32 {
    value.x.max(value.y).max(value.z)
}

fn component_mul(a: Vector3<f32>, b: Vector3<f32>) -> Vector3<f32> {
    Vector3::new(a.x * b.x, a.y * b.y, a.z * b.z)
}

fn component_min(a: Vector3<f32>, b: Vector3<f32>) -> Vector3<f32> {
    Vector3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z))
}

fn component_max(a: Vector3<f32>, b: Vector3<f32>) -> Vector3<f32> {
    Vector3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z))
}

pub struct Ray {
    pub origin: Point3<f32>,
    pub direction: Vector3<f32>,
}

impl Ray {
    // Only uses AABB collision for checks.
    pub fn broad_intersects(&self, world: &World, gfx: &mut Graphics) -> Option<RayHit> {
        self.intersects_objects(world, &gfx.engine.render_context.gpu_objects, false)
    }

    pub fn precise_intersects(&self, world: &World, gfx: &mut Graphics) -> Option<RayHit> {
        self.intersects_objects(world, &gfx.engine.render_context.gpu_objects, true)
    }

    fn intersects_objects(
        &self,
        world: &World,
        objects: &GpuObjects,
        precise: bool,
    ) -> Option<RayHit> {
        let mut closest: Option<RayHit> = None;
        let mut test = |entity, instance_index, collider: &Collider, transform: &Transform| {
            let Some(broad_distance) = ray_aabb(self, &collider.aabb(transform)) else {
                return;
            };
            let distance = if precise {
                collider.precise_ray_intersection(self, transform)
            } else {
                Some(broad_distance)
            };
            if let Some(distance) = distance {
                if closest.map(|hit| distance < hit.distance).unwrap_or(true) {
                    closest = Some(RayHit {
                        entity_handle: entity,
                        instance_index,
                        distance,
                    });
                }
            }
        };

        // Linked slots belong to individual entities even if a batch also has a collider.
        // Stream batch instances so picking a large voxel batch needs no candidate copies.
        let mut linked_slots = HashSet::new();
        world.query::<&RenderInstanceRef>(|mut query| {
            for render_ref in query.iter() {
                objects.render_instance(*render_ref); // Validate even when there is no collider.
                let batch = objects
                    .renderable(render_ref.batch)
                    .expect("invalid RenderBatchHandle");
                linked_slots.insert((batch.instance_controller_handle, render_ref.instance_index));
            }
        });
        world.query::<(Entity, &Transform, &Collider, Option<&RenderInstanceRef>)>(|mut query| {
            for (entity, transform, collider, render_ref) in query.iter() {
                let instance_index = if let Some(render_ref) = render_ref {
                    if !objects.render_instance(*render_ref).should_render {
                        continue;
                    }
                    render_ref.instance_index
                } else {
                    0
                };
                test(entity, instance_index, collider, transform);
            }
        });
        let mut tested_slots = HashSet::new();
        world.query::<(Entity, &RenderableHandle, &Collider)>(|query| {
            for (entity, renderable, collider) in query.without::<&Transform>().iter() {
                let batch = objects
                    .renderable(*renderable)
                    .expect("invalid RenderBatchHandle");
                let controller = objects
                    .instance_controllers
                    .get(batch.instance_controller_handle)
                    .expect("invalid InstanceControllerHandle");
                // Multiple group entities may refer to one controller; visit it once.
                if !tested_slots.insert(batch.instance_controller_handle) {
                    continue;
                }
                for (index, instance) in controller.instances().iter().enumerate() {
                    if instance.should_render
                        && !linked_slots.contains(&(batch.instance_controller_handle, index))
                    {
                        test(entity, index, collider, &instance.transform);
                    }
                }
            }
        });
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

#[cfg(test)]
mod tests {
    use cgmath::{Deg, Quaternion, Rotation3, Vector3};

    use super::Collider;
    use crate::core::instance::Transform;

    #[test]
    fn box_aabb_handles_nonuniform_and_negative_scale_per_axis() {
        let collider = Collider::Box {
            extents: Vector3::new(2.0, 3.0, 4.0),
        };
        let transform = Transform {
            position: Vector3::new(10.0, 20.0, 30.0),
            scale: Vector3::new(-2.0, 0.5, -0.25),
            ..Default::default()
        };

        let aabb = collider.aabb(&transform);
        assert_eq!(aabb.min, Vector3::new(6.0, 20.0, 29.0));
        assert_eq!(aabb.max, Vector3::new(10.0, 21.5, 30.0));
    }

    #[test]
    fn sphere_and_capsule_aabbs_use_conservative_axis_scales() {
        let transform = Transform {
            position: Vector3::new(1.0, 2.0, 3.0),
            scale: Vector3::new(2.0, 3.0, 4.0),
            ..Default::default()
        };

        let sphere = Collider::Sphere { radius: 2.0 }.aabb(&transform);
        assert_eq!(sphere.min, Vector3::new(-7.0, -6.0, -5.0));
        assert_eq!(sphere.max, Vector3::new(9.0, 10.0, 11.0));

        let capsule = Collider::Capsule {
            radius: 2.0,
            half_height: 5.0,
        }
        .aabb(&transform);
        assert_eq!(capsule.min, Vector3::new(-7.0, -21.0, -5.0));
        assert_eq!(capsule.max, Vector3::new(9.0, 25.0, 11.0));
    }

    #[test]
    fn plane_aabb_rotates_scaled_xz_corners() {
        let transform = Transform {
            position: Vector3::new(1.0, 2.0, 3.0),
            rotation: Quaternion::from_angle_x(Deg(90.0)),
            scale: Vector3::new(4.0, 1.0, 2.0),
        };

        let aabb = Collider::Plane.aabb(&transform);
        assert!((aabb.min.x - -1.0).abs() < 0.00001);
        assert!((aabb.max.x - 3.0).abs() < 0.00001);
        assert!((aabb.min.y - 1.0).abs() < 0.00001);
        assert!((aabb.max.y - 3.0).abs() < 0.00001);
        assert!((aabb.min.z - 2.999).abs() < 0.00001);
        assert!((aabb.max.z - 3.001).abs() < 0.00001);
    }
}
