use ultraviolet::{Mat4, Vec2, Vec3};

pub struct TransformGenerator {
    window_size: (u32, u32),
    window_scale_factor: f32,
    is_user_panning: bool,
    previous_offset: Vec2,
    pan_velocity: Vec2,
    offset: Vec2,
    scale_transform: Mat4,
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
            scale_transform: Mat4::identity(),
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

        self.previous_offset = self.offset;

        self.offset.x += (x / self.window_size.0 as f32) * 2.0;
        self.offset.y += (y / self.window_size.1 as f32) * 2.0;

        self.pan_velocity = self.offset - self.previous_offset;
    }

    pub fn apply_scale_diff(&mut self, diff: f32, origin: Option<(f32, f32)>) {
        let origin = Vec2::from(origin.unwrap_or((0.0, 0.0)));

        let scale_around = Vec2::new(
            ((origin.x / self.window_size.0 as f32) - 0.5) * 2.0,
            -((origin.y / self.window_size.1 as f32) - 0.5) * 2.0,
        ) - self.offset;

        let translate = Mat4::from_translation(Vec3::new(-scale_around.x, -scale_around.y, 0.0));
        let translate_back = Mat4::from_translation(Vec3::new(scale_around.x, scale_around.y, 0.0));

        let scale_amount = if diff > 0.0 {
            1.0 + diff * 0.1
        } else {
            1.0 / (1.0 - diff * 0.1)
        };

        let next_scale_transform =
            translate_back * Mat4::from_scale(scale_amount) * translate * self.scale_transform;

        let scale_factor = next_scale_transform.cols[0].mag();

        if scale_factor < 1.0 || scale_factor > 50.0 {
            return;
        }

        self.scale_transform = next_scale_transform;
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        let base_scale = Mat4::from_scale(self.window_scale_factor);
        let scale_ratio = Mat4::from_nonuniform_scale(Vec3::new(
            self.texture_size.width as f32 / self.window_size.0 as f32,
            self.texture_size.height as f32 / self.window_size.1 as f32,
            1.0,
        ));

        let base_model_transform = Mat4::from_translation(Vec3::new(-0.5, -0.5, 0.0));
        let translate = Mat4::from_translation(Vec3::new(self.offset.x, self.offset.y, 0.0));

        let transform =
            translate * self.scale_transform * scale_ratio * base_scale * base_model_transform;

        transform
    }

    pub fn on_window_resize(&mut self, new_width: u32, new_height: u32) {
        self.window_size = (new_width, new_height);
    }

    pub fn update(&mut self) {
        if !self.is_user_panning && self.pan_velocity != Vec2::zero() {
            self.pan_velocity *= 0.93;
            self.offset = self.offset + self.pan_velocity;

            if (self.pan_velocity.x.abs() + self.pan_velocity.y.abs()) < f32::EPSILON {
                self.pan_velocity = Vec2::zero();
            }
        }
    }
}
