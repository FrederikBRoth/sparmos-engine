use cgmath::Vector3;

pub enum BodyType {
    Static,
    Dynamic,
    Kinematic,
}
pub struct RigidBody {
    pub body_type: BodyType,

    pub velocity: Vector3<f32>,
    pub force: Vector3<f32>,

    pub mass: f32,
}

impl RigidBody {
    pub fn new(mass: f32, body_type: BodyType) -> Self {
        Self {
            body_type,
            velocity: [0.0, 0.0, 0.0].into(),
            force: [0.0, 0.0, 0.0].into(),
            mass,
        }
    }
}
