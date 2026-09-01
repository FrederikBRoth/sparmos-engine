use cgmath::{EuclideanSpace, InnerSpace, Point3, Vector3};

#[derive(Clone, Debug)]
pub enum Collider {
    Box { half_extents: Vector3<f32> },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
}

pub struct Aabb {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
}

impl Aabb {
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

