use cgmath::{Vector3, num_traits::pow};

use crate::entity::core::instance::Instance;

#[derive(Copy, Clone)]
pub struct EaseInEaseOutLoop;
impl EaseInEaseOutLoop {
    pub fn ease_in_ease_out_loop(dt: f32, delay: f32, freq: f32) -> f32 {
        if dt < delay {
            return 0.0;
        }
        let elapsed = (dt - delay) % (freq * 2.0);
        let time = if elapsed >= freq {
            (2.0 * freq - elapsed) / freq
        } else {
            elapsed / freq
        };
        let sqr = time * time;
        sqr / (2.0 * (sqr - time) + 1.0)
    }
}
pub fn ease_in_ease_out_loop(dt: f32, delay: f32, freq: f32) -> f32 {
    if dt < delay {
        return 0.0;
    }
    let elapsed = (dt - delay) % (freq * 2.0);
    let time = if elapsed >= freq {
        (2.0 * freq - elapsed) / freq
    } else {
        elapsed / freq
    };
    let sqr = time * time;
    sqr / (2.0 * (sqr - time) + 1.0)
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
#[derive(Copy, Clone)]
pub struct Linear;
impl Linear {
    pub fn ease_linear(number: f32) -> f32 {
        number.clamp(0.0, 1.0)
    }
}
#[derive(Copy, Clone)]
pub struct EaseOut;
impl EaseOut {
    pub fn ease_out_cubic(number: f32) -> f32 {
        let t = number.clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }
}
#[derive(Copy, Clone)]

pub struct EaseInEaseOut;
impl EaseInEaseOut {
    pub fn ease_in_ease_out_cubic(number: f32) -> f32 {
        let number = number.clamp(0.0, 1.0);
        if number < 0.5 {
            4.0 * number * number * number
        } else {
            1.0 - pow(-2.0 * number + 2.0, 3) / 2.0
        }
    }
}
#[derive(Copy, Clone)]
pub enum AnimationTransition {
    EaseOut(EaseOut),
    EaseInEaseOut(EaseInEaseOut),
    EaseInEaseOutLoop(EaseInEaseOutLoop),
}

impl AnimationTransition {
    pub fn lerp(
        &self,
        start: Vector3<f32>,
        end: Vector3<f32>,
        number: f32,
        delay: f32,
    ) -> Vector3<f32> {
        match self {
            AnimationTransition::EaseInEaseOut(_) => {
                let lerp_value = EaseInEaseOut::ease_in_ease_out_cubic(number);
                start + (end - start) * lerp_value
            }
            AnimationTransition::EaseInEaseOutLoop(_) => {
                let lerp_value = EaseInEaseOutLoop::ease_in_ease_out_loop(number, delay, 1.0) - 0.5;
                start + (end - start) * lerp_value
            }
            AnimationTransition::EaseOut(_) => {
                let lerp_value = EaseOut::ease_out_cubic(number);
                start + (end - start) * lerp_value
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
    time: f32,
    movement_vector: Vector3<f32>,
    animation_transition: AnimationTransition,
}
impl AnimationPersistent {
    pub fn new(movement_vector: Vector3<f32>, animation_transition: AnimationTransition) -> Self {
        Self {
            time: 0.0,
            movement_vector,
            animation_transition,
        }
    }
}

#[derive(Clone)]
pub struct AnimationStep {
    movement_vector: Vector3<f32>,
    time: f32,
    reversed: bool,
    activated: bool,
    animating: bool,
    speed: f32,
    animation_transition: AnimationTransition,
    is_static: bool,
    instant: bool,
    id: i32,
}

impl AnimationStep {
    /// Constructs a new AnimationStep
    pub fn new(
        movement_vector: Vector3<f32>,
        speed: f32,
        reversed: bool,
        activated: bool,
        is_static: bool,
        instant: bool,
        animation_transition: AnimationTransition,
        id: i32,
    ) -> Self {
        Self {
            movement_vector,
            time: 0.0,
            reversed,
            activated,
            speed,
            is_static,
            instant,
            animating: false,
            animation_transition,
            id,
        }
    }
}
#[derive(Clone)]

pub struct Animation {
    activated: bool,
    time: f32,
    pub start: Vector3<f32>,
    pub current_pos: Vector3<f32>,
    pub grid_pos: Vector3<f32>,
    persistent_animation: Vec<AnimationPersistent>,

    pub animations: Vec<AnimationStep>,
    color: Vector3<f32>,
    animate_color: bool,
}

pub struct AnimationHandler {
    pub movement_list: Vec<Animation>,
    pub disabled: bool,
}

impl AnimationHandler {
    pub fn new(animations: Vec<AnimationType>) -> AnimationHandler {
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

            movement_list: vec![],
        }
    }

    pub fn disable(&mut self) {
        self.disabled = true;
    }
    pub fn enable(&mut self) {
        self.disabled = false;
    }

    pub fn set_manual_animation_color(&mut self, index: usize, color: Vector3<f32>) {
        if let Some(animation) = self.movement_list.get_mut(index) {
            animation.animate_color = false;
            animation.color = color;
        }
    }

    pub fn set_animated_color(&mut self, index: usize) {
        if let Some(animation) = self.movement_list.get_mut(index) {
            animation.animate_color = true;
        }
    }

    pub fn set_animation(&mut self, index: usize, animation_type: AnimationType) {
        if self.disabled {
            return;
        }
        if let Some(animation) = self.movement_list.get_mut(index) {
            match animation_type {
                AnimationType::Persistent(animation_persistent) => {
                    animation.persistent_animation.push(animation_persistent);
                }
                AnimationType::Step(animation_step) => {
                    animation.animations.push(animation_step);
                }
            }
        }
    }

    pub fn is_locked(&mut self) -> bool {
        let mut locked = false;
        for animation in self.movement_list.iter_mut() {
            for step in animation.animations.iter_mut() {
                if step.is_static {
                    locked = step.is_static
                }
            }
        }
        locked
    }

    pub fn set_animation_state(&mut self, index: usize, state: bool) {
        if self.disabled {
            return;
        }
        if let Some(animation) = self.movement_list.get_mut(index) {
            for step in animation.animations.iter_mut() {
                step.activated = state;
                step.animating = state;
            }
        }
    }

    pub fn reverse(&mut self) {
        for animation in self.movement_list.iter_mut() {
            for step in animation.animations.iter_mut() {
                step.reversed = true;
            }
        }
    }

    pub fn reset_instance_position_to_current_position(&mut self, instances: &mut [Instance]) {
        for (i, animation) in self.movement_list.iter_mut().enumerate() {
            if let Some(instance) = instances.get_mut(i) {
                if animation.animations.is_empty() {
                    continue;
                }

                let delay = (animation.current_pos.x + animation.current_pos.z) * 0.05;
                let mut total_movement = Vector3::new(0.0, 0.0, 0.0);
                for persistent in animation.persistent_animation.iter_mut() {
                    total_movement += persistent.animation_transition.lerp(
                        animation.start,
                        animation.start + persistent.movement_vector,
                        persistent.time,
                        delay,
                    ) - animation.start;
                }
                instance.position = animation.current_pos - total_movement;
                instance.bounding = animation.current_pos + animation.current_pos - total_movement;
                animation.start = instance.position;
                animation.grid_pos = instance.position;
            } else {
                continue;
            };
            animation.animations.clear();
        }
    }

    pub fn animate(&mut self, dt: f32) {
        if self.disabled {
            return;
        }
        for animation in self.movement_list.iter_mut().filter(|ani| ani.activated) {
            let delta = dt;

            animation.time += dt;

            let delay = (animation.current_pos.x + animation.current_pos.z) * 0.05;
            let mut total_movement = animation.start;
            for persistent in animation.persistent_animation.iter_mut() {
                persistent.time += delta;
                total_movement += persistent.animation_transition.lerp(
                    animation.start,
                    animation.start + persistent.movement_vector,
                    persistent.time,
                    delay,
                ) - animation.start;
            }
            let lerp = 1.0 * ease_in_ease_out_loop(animation.time, delay, 1.0);
            if animation.animate_color {
                animation.color = get_height_color(lerp);
            }

            let mut step_delta = Vector3::new(0.0, 0.0, 0.0);
            let mut instant_movement = Vector3::new(0.0, 0.0, 0.0);
            let mut instant_animation = false;
            for step in animation.animations.iter_mut() {
                let mut step_movement = Vector3::new(0.0, 0.0, 0.0);
                if !step.activated {
                    continue;
                }

                if step.instant {
                    step.time = 1.0;
                    instant_movement = (animation.start + step.movement_vector) - animation.start;
                    instant_animation = true;
                } else if step.reversed {
                    step.time -= delta * step.speed;
                    step.time = step.time.clamp(0.0, 1.0);
                    step_movement -= step.animation_transition.lerp(
                        animation.start + step.movement_vector,
                        animation.start,
                        step.time,
                        0.0,
                    ) - (animation.start + step.movement_vector);
                } else {
                    step.time += delta * step.speed;
                    step.time = step.time.clamp(0.0, 1.0);
                    step_movement += step.animation_transition.lerp(
                        animation.start,
                        animation.start + step.movement_vector,
                        step.time,
                        0.0,
                    ) - animation.start;
                }

                if step.time == 0.0 || step.time == 1.0 {
                    step.animating = false
                }

                if step.is_static && step.time == 1.0 && !step.instant {
                    animation.start += step_movement;
                    animation.grid_pos = animation.start + step_movement;
                }
                step_delta += step_movement;
            }
            if instant_animation {
                animation.grid_pos = animation.start + instant_movement;
                animation.start = animation.grid_pos;
                total_movement += instant_movement;
                animation.current_pos = total_movement;
            } else {
                animation.grid_pos = animation.start + step_delta;
                total_movement += step_delta;
                animation.current_pos = total_movement;
            }

            animation.animations.retain(|step| {
                !((step.reversed && step.time == 0.0) || (step.is_static && step.time == 1.0))
            });
        }
    }

    pub fn reorder_instance_list(&mut self, instance_list: Vec<Instance>) {
        for instance in instance_list {}
    }

    pub fn update_instance(&mut self, index: usize, instance: &mut Instance) {
        if let Some(animation) = self.movement_list.get_mut(index) {
            if !animation.activated {
                return;
            }
            instance.position = animation.current_pos;
            instance.bounding = instance.size + animation.current_pos;
            instance.color = animation.color;
        }
    }
}
