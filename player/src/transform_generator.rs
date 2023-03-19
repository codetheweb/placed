use ultraviolet::{Mat4, Vec2, Vec3};

pub struct TransformGenerator {
    window_size: (u32, u32),
    window_scale_factor: f32,
    is_user_panning: bool,
    previous_offset: Vec2,
    pan_velocity: Vec2,
    offset: Vec2,
    scale: f32,
    texture_size: wgpu::Extent3d,
}

impl TransformGenerator {
    pub fn new(window_width: u32, window_height: u32, texture_size: wgpu::Extent3d) -> Self {
        Self {
            window_size: (window_width, window_height),
            window_scale_factor: 1.0,
            is_user_panning: false,
            previous_offset: Vec2::zero(),
            pan_velocity: Vec2::zero(),
            offset: Vec2::zero(),
            scale: 1.0,
            texture_size,
        }
    }

    pub fn set_window_scale_factor(&mut self, window_scale_factor: f32) {
        self.window_scale_factor = window_scale_factor;
    }

    pub fn on_pan_start(&mut self) {
        self.is_user_panning = true;
    }

    pub fn on_pan_end(&mut self) {
        self.is_user_panning = false;
        self.pan_velocity *= 0.5;
    }

    pub fn apply_translate_diff(&mut self, x: f32, y: f32) {
        if x == 0.0 && y == 0.0 {
            return;
        }

        self.offset.x += x;
        self.offset.y += y;

        self.pan_velocity = self.offset - self.previous_offset;
        self.previous_offset = self.offset;
    }

    pub fn apply_scale_diff(&mut self, diff: f32) {
        let new_scale = (self.scale + diff).clamp(0.5, 20.0);
        self.scale = new_scale;
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        let world_to_screen = Mat4::from_nonuniform_scale(Vec3::new(
            self.texture_size.width as f32 / self.window_size.0 as f32,
            self.texture_size.height as f32 / self.window_size.1 as f32,
            1.0,
        ));

        let translate = Mat4::from_translation(Vec3::new(
            self.offset.x / (self.texture_size.width as f32 / 2.0),
            self.offset.y / (self.texture_size.height as f32 / 2.0),
            0.0,
        ));

        let base_scale = Mat4::from_scale(self.window_scale_factor);
        let scale = Mat4::from_scale(self.scale);

        let transform = world_to_screen * translate * base_scale * scale;

        transform
    }

    pub fn on_window_resize(&mut self, new_width: u32, new_height: u32) {
        self.window_size = (new_width, new_height);
    }

    pub fn update(&mut self) {
        if !self.is_user_panning && self.pan_velocity != Vec2::zero() {
            self.pan_velocity *= 0.9;
            self.offset = self.offset + self.pan_velocity;

            if (self.pan_velocity.x.abs() + self.pan_velocity.y.abs()) < 0.1 {
                self.pan_velocity = Vec2::zero();
            }
        }
    }
}
