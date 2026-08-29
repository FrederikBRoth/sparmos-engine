use cgmath::{EuclideanSpace, Point3, Vector3, num_traits::pow, vec3};

use crate::core::instance::Instance;

pub enum TransitionType {
    Overwrite,
    Additive,
}
pub fn get_height_color(height: f32) -> Vector3<f32> {
    // high color rgb(255, 153, 230)
    //#f472b6
    //low color rgb(204, 0, 153)
    //#db2777

    let low_color = Vector3::new(0.852, 0.067, 0.319);
    let high_color = Vector3::new(0.953, 0.406, 0.674);
    low_color + (high_color - low_color) * height
}
pub fn castaljau_point(points: &[egui::Pos2], t: f32) -> egui::Pos2 {
    if points.len() == 1 {
        points[0]
    } else {
        let mut new_points: Vec<egui::Pos2> = Vec::with_capacity(points.len() - 1);
        for i in 0..points.len() - 1 {
            let p0 = points[i];
            let p1 = points[i + 1];

            new_points.push(egui::Pos2 {
                x: (1.0 - t) * p0.x + t * p1.x,
                y: (1.0 - t) * p0.y + t * p1.y,
            });
        }
        castaljau_point(&new_points, t)
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub enum Interpolation {
    EaseOut,
    #[default]
    EaseInEaseOut,
    EaseInEaseOutLoop,
    Linear,
    Custom(Vec<egui::Pos2>),
}

impl Interpolation {
    pub fn lerp_delay(&self, t: f32, delay: f32) -> f32 {
        match self {
            Interpolation::EaseInEaseOut => {
                let number = t.clamp(0.0, 1.0);
                if number < 0.5 {
                    4.0 * number * number * number
                } else {
                    1.0 - pow(-2.0 * number + 2.0, 3) / 2.0
                }
            }
            Interpolation::EaseInEaseOutLoop => {
                let freq = 1.0;
                if t < delay {
                    return 0.0;
                }
                let elapsed = (t - delay) % (freq * 2.0);
                let time = if elapsed >= freq {
                    (2.0 * freq - elapsed) / freq
                } else {
                    elapsed / freq
                };
                let sqr = time * time;
                (sqr / (2.0 * (sqr - time) + 1.0)) - 0.5
            }
            Interpolation::EaseOut => {
                let t = t.clamp(0.0, 1.0);
                1.0 - (1.0 - t).powi(3)
            }
            Interpolation::Linear => t.clamp(0.0, 1.0),
            Interpolation::Custom(point2s) => castaljau_point(point2s, t).x,
        }
    }
    pub fn lerp(&self, t: f32, reversed: bool) -> (f32, f32) {
        match self {
            Interpolation::EaseInEaseOut => {
                let number = t.clamp(0.0, 1.0);
                let x = if number < 0.5 {
                    4.0 * number * number * number
                } else {
                    1.0 - pow(-2.0 * number + 2.0, 3) / 2.0
                };

                if reversed { (1.0 - x, 0.0) } else { (x, 0.0) }
            }
            Interpolation::EaseInEaseOutLoop => {
                let freq = 1.0;
                let elapsed = t % (freq * 2.0);
                let time = if elapsed >= freq {
                    (2.0 * freq - elapsed) / freq
                } else {
                    elapsed / freq
                };
                let sqr = time * time;
                let x = (sqr / (2.0 * (sqr - time) + 1.0)) - 0.5;

                if reversed { (1.0 - x, 0.0) } else { (x, 0.0) }
            }
            Interpolation::EaseOut => {
                let t = t.clamp(0.0, 1.0);
                let x = 1.0 - (1.0 - t).powi(3);
                if reversed { (1.0 - x, 0.0) } else { (x, 0.0) }
            }
            Interpolation::Linear => {
                let x = t.clamp(0.0, 1.0);
                if reversed { (1.0 - x, 0.0) } else { (x, 0.0) }
            }
            Interpolation::Custom(point2s) => {
                let point = castaljau_point(point2s, t);
                (point.x, point.y)
            }
        }
    }
}

//Send og Sync, trådhelvede
pub enum AnimationType {
    Persistent(AnimationPersistent),
    Step(AnimationStep),
}

#[derive(Clone)]
pub struct AnimationPersistent {
    pub amplitude: f32,
    pub speed: f32,
    pub time: f32,
    movement_vector: Vector3<f32>,
    _animation_transition: Interpolation,
}
impl AnimationPersistent {
    pub fn new(movement_vector: Vector3<f32>, animation_transition: Interpolation) -> Self {
        Self {
            time: 0.0,
            speed: 1.0,
            amplitude: 1.0,
            movement_vector,
            _animation_transition: animation_transition,
        }
    }

    pub fn update(&mut self, dt: f32) -> Vector3<f32> {
        self.time += dt;

        // Simple smooth ping-pong
        let t = (self.time * self.speed).sin() * 0.5 + 0.5;

        self.movement_vector * self.amplitude * t
    }
}
#[derive(Copy, Clone, PartialEq)]
pub enum StepState {
    Idle,
    Forward,
    Backward,
    Finished,
}
#[derive(Clone)]
pub struct AnimationStep {
    pub from: Vector3<f32>,
    pub to: Vector3<f32>,
    pub t: f32,
    pub speed: f32,
    pub state: StepState,
    pub animation_transition: Interpolation,
}

impl AnimationStep {
    /// Constructs a new AnimationStep
    pub fn new(
        from: Vector3<f32>,
        to: Vector3<f32>,
        t: f32,
        speed: f32,
        animation_transition: Interpolation,
        state: StepState,
    ) -> Self {
        Self {
            from,
            to,
            t,
            speed,
            animation_transition,
            state,
        }
    }

    pub fn update(&mut self, dt: f32) -> Vector3<f32> {
        match self.state {
            StepState::Idle | StepState::Finished => return Vector3::new(0.0, 0.0, 0.0),

            StepState::Forward => {
                self.t += dt * self.speed;
                if self.t >= 1.0 {
                    self.t = 1.0;
                    self.state = StepState::Finished;
                }
            }

            StepState::Backward => {
                self.t -= dt * self.speed;
                if self.t <= 0.0 {
                    self.t = 0.0;
                    self.state = StepState::Finished;
                }
            }
        }

        let k = self.animation_transition.lerp_delay(self.t, 0.0);
        let pos = self.from + (self.to - self.from) * k;
        pos - self.from
    }
    pub fn is_finished(&self) -> bool {
        self.state == StepState::Finished
    }
}
#[derive(Clone)]

pub struct Animation {
    pub base_position: Vector3<f32>,
    pub offset: Vector3<f32>,
    pub persistents: Vec<AnimationPersistent>,
    pub steps: Vec<AnimationStep>,
    pub color: Vector3<f32>,
}

impl Animation {
    pub fn update(&mut self, dt: f32) {
        let mut offset = Vector3::new(0.0, 0.0, 0.0);

        // Persistent contributions
        for p in &mut self.persistents {
            offset += p.update(dt);
        }

        // Step contributions
        for s in &mut self.steps {
            offset += s.update(dt);

            if s.is_finished() {
                self.base_position += offset;
                offset = Vector3::new(0.0, 0.0, 0.0);
            }
        }
        // Remove finished steps safely
        self.steps.retain(|s| !s.is_finished());

        self.offset = offset;

        // let height = (offset.y * 0.5 + 0.5).clamp(0.0, 1.0);
        // self.color = Vector3::new(1.0, 0.2, 0.6) * height;
    }

    pub fn final_position(&self) -> Vector3<f32> {
        self.base_position + self.offset
    }
}

pub struct AnimationHandler {
    pub movement_list: Vec<Animation>,
    pub disabled: bool,
}

impl AnimationHandler {
    pub fn new_from_instances(
        instances: &[Instance],
        animations: Vec<AnimationType>,
    ) -> AnimationHandler {
        let mut steps = Vec::new();
        let mut persistents = Vec::new();

        for anim in animations {
            match anim {
                AnimationType::Step(step) => steps.push(step),
                AnimationType::Persistent(persistent) => persistents.push(persistent),
                // add other variants here as needed
            }
        }
        AnimationHandler {
            disabled: false,

            movement_list: {
                instances
                    .iter()
                    .map(|instance| Animation {
                        base_position: instance.position,
                        offset: vec3(0.0, 0.0, 0.0),
                        persistents: persistents.clone(),
                        steps: steps.clone(),
                        color: instance.color,
                    })
                    .collect()
            },
        }
    }

    pub fn new_from_point(
        base_pos: Point3<f32>,
        animations: Vec<AnimationType>,
    ) -> AnimationHandler {
        let mut steps = Vec::new();
        let mut persistents = Vec::new();

        for anim in animations {
            match anim {
                AnimationType::Step(step) => steps.push(step),
                AnimationType::Persistent(persistent) => persistents.push(persistent),
                // add other variants here as needed
            }
        }
        AnimationHandler {
            disabled: false,
            movement_list: {
                [Animation {
                    base_position: base_pos.to_vec(),
                    offset: vec3(0.0, 0.0, 0.0),
                    persistents: persistents.clone(),
                    steps: steps.clone(),
                    color: Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                }]
                .to_vec()
            },
        }
    }

    pub fn add_animation(&mut self, anim: AnimationType, animation_index: usize) {
        match anim {
            AnimationType::Step(step) => self
                .movement_list
                .get_mut(animation_index)
                .unwrap()
                .steps
                .push(step),
            AnimationType::Persistent(persistent) => self
                .movement_list
                .get_mut(animation_index)
                .unwrap()
                .persistents
                .push(persistent),
            // add other variants here as needed
        }
    }

    pub fn disable(&mut self) {
        self.disabled = true;
    }
    pub fn enable(&mut self) {
        self.disabled = false;
    }

    // pub fn set_manual_animation_color(&mut self, index: usize, color: Vector3<f32>) {
    //     if let Some(animation) = self.movement_list.get_mut(index) {
    //         animation.animate_color = false;
    //         animation.color = color;
    //     }
    // }
    //
    // pub fn set_animated_color(&mut self, index: usize) {
    //     if let Some(animation) = self.movement_list.get_mut(index) {
    //         animation.animate_color = true;
    //     }
    // }

    pub fn set_animation(&mut self, index: usize, animation_type: AnimationType) {
        if self.disabled {
            return;
        }
        if let Some(animation) = self.movement_list.get_mut(index) {
            match animation_type {
                AnimationType::Persistent(animation_persistent) => {
                    animation.persistents.push(animation_persistent);
                }
                AnimationType::Step(animation_step) => {
                    animation.steps.push(animation_step);
                }
            }
        }
    }

    pub fn reset_instance_position_to_current_position(&mut self, instances: &mut [Instance]) {
        for (anim, instance) in self.movement_list.iter_mut().zip(instances.iter_mut()) {
            let final_pos = anim.final_position();

            instance.position = final_pos;
            instance.bounding = instance.size + final_pos;
            instance.color = anim.color;

            anim.base_position = final_pos;

            anim.offset = Vector3::new(0.0, 0.0, 0.0);
            anim.steps.clear();
            anim.persistents.clear();
        }
    }

    pub fn update_instance(&mut self, dt: f32, instances: &mut [Instance]) {
        for (anim, instance) in self.movement_list.iter_mut().zip(instances.iter_mut()) {
            anim.update(dt);

            instance.position = anim.final_position();
            instance.bounding = instance.size + instance.position;
            instance.color = anim.color;
        }
    }
    pub fn reset_point_position_to_current_position(&mut self, singular: &mut Point3<f32>) {
        let anim = self.movement_list.get_mut(0).unwrap();
        let final_pos = anim.final_position();

        singular.x = final_pos.x;
        singular.y = final_pos.y;
        singular.z = final_pos.z;
        anim.base_position = final_pos;
        anim.offset = Vector3::new(0.0, 0.0, 0.0);
        anim.steps.clear();
        anim.persistents.clear();
    }

    pub fn update_point(&mut self, dt: f32, singular: &mut Point3<f32>) {
        let anim = self.movement_list.get_mut(0).unwrap();
        anim.update(dt);

        singular.x = anim.final_position().x;
        singular.y = anim.final_position().y;
        singular.z = anim.final_position().z;
    }
}
