use std::collections::{HashMap, HashSet};

use cgmath::{InnerSpace, Vector3, vec3};
use hecs::Entity;

use crate::core::{
    engine::DefaultSystem,
    entities::World,
    instance::Transform,
    physics::{
        collision::{Collider, Collision},
        rigidbody::{BodyType, RigidBody},
        solver::{ImpulseSolver, PositionSolver, Solver},
    },
};
pub(crate) const PHYSICS_DT: f32 = 1.0 / 60.0;
const VELOCITY_ITERATIONS: usize = 10;
const POSITION_ITERATIONS: usize = 3;
const MAX_FIXED_SUBSTEPS: usize = 8;
const MAX_FRAME_DT: f32 = 0.25;
const BROAD_PHASE_CELL_SIZE: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsDiagnostics {
    pub fixed_substeps: usize,
    pub candidate_pairs: usize,
    pub active_collisions: usize,
    pub max_penetration: f32,
    pub max_abs_normal_speed: f32,
    pub dropped_time: f32,
}

pub struct PhysicsSystem {
    current_dt: f32,
    gravity: Vector3<f32>,
    impulse_solver: ImpulseSolver,
    position_solver: PositionSolver,
    diagnostics: PhysicsDiagnostics,
    debug_diagnostics: bool,
}

impl PhysicsSystem {
    pub fn new(gravity: Vector3<f32>) -> Self {
        Self {
            current_dt: 0.0,
            gravity,
            impulse_solver: ImpulseSolver::default(),
            position_solver: PositionSolver,
            diagnostics: PhysicsDiagnostics::default(),
            debug_diagnostics: false,
        }
    }

    pub fn diagnostics(&self) -> PhysicsDiagnostics {
        self.diagnostics
    }

    pub fn set_debug_diagnostics(&mut self, enabled: bool) {
        self.debug_diagnostics = enabled;
    }

    /// Advance physics without requiring any rendering resources.
    pub fn advance(&mut self, world: &mut World, dt: std::time::Duration) {
        self.diagnostics = PhysicsDiagnostics::default();

        let incoming_dt = dt.as_secs_f32();
        let accepted_dt = incoming_dt.min(MAX_FRAME_DT);
        self.diagnostics.dropped_time += incoming_dt - accepted_dt;
        self.current_dt += accepted_dt;

        while self.current_dt >= PHYSICS_DT && self.diagnostics.fixed_substeps < MAX_FIXED_SUBSTEPS
        {
            world.query::<&mut RigidBody>(|mut query| {
                for rigidbody in query.iter() {
                    if matches!(rigidbody.body_type, BodyType::Dynamic) {
                        let acceleration = self.gravity + rigidbody.force * rigidbody.inv_mass();
                        rigidbody.velocity += acceleration * PHYSICS_DT;
                    }

                    rigidbody.force = vec3(0.0, 0.0, 0.0);
                }
            });

            //Creates candidates from query
            let mut candidates = vec![];
            world.query::<(Entity, &Transform, &Collider, &RigidBody)>(|mut query| {
                for (entity, transform, collider, rigidbody) in query.iter() {
                    candidates.push(PhysicsCandidate {
                        entity,
                        collider: collider.clone(),
                        transform: transform.clone(),
                        is_static: matches!(rigidbody.body_type, BodyType::Static),
                    });
                }
            });
            //sorts them from bottom to top
            candidates.sort_by(|a, b| {
                b.is_static.cmp(&a.is_static).then_with(|| {
                    let a_along_gravity = a.transform.position.dot(self.gravity);
                    let b_along_gravity = b.transform.position.dot(self.gravity);
                    b_along_gravity
                        .total_cmp(&a_along_gravity)
                        .then_with(|| a.entity.cmp(&b.entity))
                })
            });

            let mut collisions: Vec<Collision> = vec![];
            //creates a broad_phase_pair based on a chunk based environment. Gives a list of
            //candidate pairs
            let candidate_pairs = broad_phase_pairs(&candidates);
            self.diagnostics.candidate_pairs += candidate_pairs.len();
            for (i, j) in candidate_pairs {
                let a = &candidates[i];
                let b = &candidates[j];

                let points =
                    Collider::collision(&a.collider, &a.transform, &b.collider, &b.transform);
                if points.has_collision {
                    self.diagnostics.max_penetration =
                        self.diagnostics.max_penetration.max(points.depth);
                    collisions.push(Collision {
                        object_a: a.entity,
                        object_b: b.entity,
                        collision_points: points,
                    });
                }
            }

            self.diagnostics.active_collisions += collisions.len();

            //Solve impulses
            self.impulse_solver.prepare(world, &collisions);
            for _ in 0..VELOCITY_ITERATIONS {
                self.impulse_solver.solve(world, &collisions, PHYSICS_DT);
            }
            for collision in &collisions {
                let [a, b] = world
                    .entities
                    .query_disjoint_mut::<&RigidBody, 2>([collision.object_a, collision.object_b]);
                let body_a = a.expect("collision A needs RigidBody");
                let body_b = b.expect("collision B needs RigidBody");
                let normal_speed = (body_b.velocity - body_a.velocity)
                    .dot(collision.collision_points.normal)
                    .abs();
                self.diagnostics.max_abs_normal_speed =
                    self.diagnostics.max_abs_normal_speed.max(normal_speed);
            }

            world.query::<(&mut Transform, &RigidBody)>(|mut query| {
                for (transform, rigidbody) in query.iter() {
                    if matches!(rigidbody.body_type, BodyType::Dynamic) {
                        transform.position += rigidbody.velocity * PHYSICS_DT;
                    }
                }
            });

            for _ in 0..POSITION_ITERATIONS {
                self.position_solver.solve(world, &collisions, PHYSICS_DT);
            }

            self.current_dt -= PHYSICS_DT;
            self.diagnostics.fixed_substeps += 1;
        }

        if self.current_dt >= PHYSICS_DT {
            let retained_time = self.current_dt % PHYSICS_DT;
            self.diagnostics.dropped_time += self.current_dt - retained_time;
            self.current_dt = retained_time;
        }

        if self.debug_diagnostics && self.diagnostics.fixed_substeps > 0 {
            log::debug!(
                "physics: substeps={}, pairs={}, collisions={}, max_depth={:.6}, max_normal_speed={:.6}, velocity_iterations={}, position_iterations={}, dropped_time={:.6}",
                self.diagnostics.fixed_substeps,
                self.diagnostics.candidate_pairs,
                self.diagnostics.active_collisions,
                self.diagnostics.max_penetration,
                self.diagnostics.max_abs_normal_speed,
                VELOCITY_ITERATIONS,
                POSITION_ITERATIONS,
                self.diagnostics.dropped_time,
            );
        }
    }
}

impl DefaultSystem for PhysicsSystem {
    fn run(
        &mut self,
        world: &mut World,
        _resources: &mut crate::core::render::RenderContext,
        dt: std::time::Duration,
    ) {
        self.advance(world, dt);
    }
}

struct PhysicsCandidate {
    entity: Entity,
    collider: Collider,
    transform: crate::core::instance::Transform,
    is_static: bool,
}

fn broad_phase_pairs(candidates: &[PhysicsCandidate]) -> Vec<(usize, usize)> {
    let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
    let mut finite_colliders = Vec::new();
    let mut planes = Vec::new();

    for (index, candidate) in candidates.iter().enumerate() {
        if matches!(candidate.collider, Collider::Plane) {
            planes.push(index);
            continue;
        }

        finite_colliders.push(index);
        let aabb = candidate.collider.aabb(&candidate.transform);
        let min = grid_cell(aabb.min);
        let max = grid_cell(aabb.max);
        for x in min.0..=max.0 {
            for y in min.1..=max.1 {
                for z in min.2..=max.2 {
                    grid.entry((x, y, z)).or_default().push(index);
                }
            }
        }
    }

    let mut unique_pairs = HashSet::new();
    for cell in grid.values() {
        for left in 0..cell.len() {
            for right in (left + 1)..cell.len() {
                let pair = ordered_pair(cell[left], cell[right]);
                if !(candidates[pair.0].is_static && candidates[pair.1].is_static) {
                    unique_pairs.insert(pair);
                }
            }
        }
    }

    for plane in planes {
        for &finite in &finite_colliders {
            let pair = ordered_pair(plane, finite);
            if !(candidates[pair.0].is_static && candidates[pair.1].is_static) {
                unique_pairs.insert(pair);
            }
        }
    }

    let mut pairs: Vec<_> = unique_pairs.into_iter().collect();
    pairs.sort_unstable();
    pairs
}

fn grid_cell(position: Vector3<f32>) -> (i32, i32, i32) {
    (
        (position.x / BROAD_PHASE_CELL_SIZE).floor() as i32,
        (position.y / BROAD_PHASE_CELL_SIZE).floor() as i32,
        (position.z / BROAD_PHASE_CELL_SIZE).floor() as i32,
    )
}

fn ordered_pair(a: usize, b: usize) -> (usize, usize) {
    if a < b { (a, b) } else { (b, a) }
}
