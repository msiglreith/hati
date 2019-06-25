//! Camera handling

use cgmath::*;
use specs::prelude::*;
use std::ops::Range;
use winit::*;

#[derive(Debug)]
pub struct Camera {
    pub position: Point3<f32>,
    pub rotation: [Rad<f32>; 3],
    pub up: Vector3<f32>,

    // control
    pub view_move: (bool, bool),
    pub view_rotate: (bool, bool, bool, bool),

    pub depth_range: Range<f32>,
    pub focal_length: f32,
}

impl Component for Camera {
    type Storage = HashMapStorage<Self>;
}

impl Camera {
    pub fn on_event(&mut self, input: KeyboardInput) {
        let KeyboardInput {
            virtual_keycode,
            state,
            ..
        } = input;
        match (state, virtual_keycode) {
            (ElementState::Pressed, Some(VirtualKeyCode::W)) => self.view_move.0 = true,
            (ElementState::Pressed, Some(VirtualKeyCode::S)) => self.view_move.1 = true,

            (ElementState::Released, Some(VirtualKeyCode::W)) => self.view_move.0 = false,
            (ElementState::Released, Some(VirtualKeyCode::S)) => self.view_move.1 = false,

            (ElementState::Pressed, Some(VirtualKeyCode::Left)) => self.view_rotate.0 = true,
            (ElementState::Pressed, Some(VirtualKeyCode::Up)) => self.view_rotate.1 = true,
            (ElementState::Pressed, Some(VirtualKeyCode::Right)) => self.view_rotate.2 = true,
            (ElementState::Pressed, Some(VirtualKeyCode::Down)) => self.view_rotate.3 = true,

            (ElementState::Released, Some(VirtualKeyCode::Left)) => self.view_rotate.0 = false,
            (ElementState::Released, Some(VirtualKeyCode::Up)) => self.view_rotate.1 = false,
            (ElementState::Released, Some(VirtualKeyCode::Right)) => self.view_rotate.2 = false,
            (ElementState::Released, Some(VirtualKeyCode::Down)) => self.view_rotate.3 = false,
            _ => (),
        }
    }

    pub fn update(&mut self, dt: f32) {
        let view_move_speed = 300.0f32;
        let view_rot_speed = Rad(1.0f32);
        let view_dir = self.get_view_dir();

        // move forward/backward
        if self.view_move.0 {
            self.position += view_dir * view_move_speed * dt;
        }
        if self.view_move.1 {
            self.position -= view_dir * view_move_speed * dt;
        }

        if self.view_rotate.0 {
            self.rotation[0] = self.rotation[0] + view_rot_speed * dt;
        }
        if self.view_rotate.2 {
            self.rotation[0] = self.rotation[0] - view_rot_speed * dt;
        }
        if self.view_rotate.1 {
            self.rotation[1] = self.rotation[1] + view_rot_speed * dt;
        } // todo: clamp
        if self.view_rotate.3 {
            self.rotation[1] = self.rotation[1] - view_rot_speed * dt;
        } // todo: clamp
    }

    fn get_view_dir(&self) -> Vector3<f32> {
        let rot_z = Quaternion::from(Euler::new(self.rotation[1], Rad(0.0), Rad(0.0)));
        let rot_y = Quaternion::from(Euler::new(Rad(0.0), self.rotation[0], Rad(0.0)));
        let rotation = rot_y * rot_z;
        rotation.rotate_vector(Vector3::new(0.0, 0.0, -1.0))
    }

    pub fn view(&self) -> [[f32; 4]; 4] {
        let view_dir = self.get_view_dir();
        Matrix4::look_at(self.position, self.position + view_dir, self.up).into()
    }
}
