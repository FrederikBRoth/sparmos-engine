use cgmath::{Vector3, vec3};

use crate::core::{
    engine::{DefaultSystem, GpuBindableSystem},
    physics::{
        collision::Collider,
        rigidbody::{self, BodyType, RigidBody},
    },
    render::Renderable,
};
const PHYSICS_DT: f32 = 1.0 / 60.0;

pub struct PhysicsSystem {
    current_dt: f32,
    gravity: Vector3<f32>,
}

impl PhysicsSystem {
    pub fn new(gravity: Vector3<f32>) -> Self {
        Self {
            current_dt: 0.0,
            gravity,
        }
    }
}

impl DefaultSystem for PhysicsSystem {
    fn run(
        &mut self,
        world: std::cell::Ref<'_, crate::core::entities::World>,
        resources: &mut crate::core::render::RenderContext,
        dt: std::time::Duration,
    ) {
        self.current_dt += dt.as_secs_f32();

        while self.current_dt >= PHYSICS_DT {
            world.query::<(&Renderable, &Collider, &mut RigidBody)>(|mut query| {
                for (renderable, collider, rigidbody) in query.iter() {
                    if matches!(rigidbody.body_type, BodyType::Static) {
                        continue;
                    }
                    for instance in resources.gpu_objects.instance_controllers
                        [renderable.instance_controller_handle]
                        .instances_mut()
                    {
                        rigidbody.force += rigidbody.mass * self.gravity;
                        rigidbody.velocity += rigidbody.force / rigidbody.mass * PHYSICS_DT;

                        instance.position += rigidbody.velocity * PHYSICS_DT;
                        rigidbody.force = vec3(0.0, 0.0, 0.0);
                    }
                }
            });

            self.current_dt -= PHYSICS_DT
        }
    }
}
