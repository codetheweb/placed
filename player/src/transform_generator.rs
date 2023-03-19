use ultraviolet::{Mat4, Vec3};

pub struct TransformGenerator {
    window_size: (u32, u32),
    window_scale_factor: f32,
    offset: (f32, f32),
    scale: f32,
    texture_size: wgpu::Extent3d,
}

impl TransformGenerator {
    pub fn new(window_width: u32, window_height: u32, texture_size: wgpu::Extent3d) -> Self {
        Self {
            window_size: (window_width, window_height),
            window_scale_factor: 1.0,
            offset: (0.0, 0.0),
            scale: 1.0,
            texture_size,
        }
    }

    pub fn set_window_scale_factor(&mut self, window_scale_factor: f32) {
        self.window_scale_factor = window_scale_factor;
    }

    pub fn apply_translate_diff(&mut self, x: f32, y: f32) {
        self.offset.0 += x;
        self.offset.1 += y;
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
            self.offset.0 / (self.texture_size.width as f32 / 2.0),
            self.offset.1 / (self.texture_size.height as f32 / 2.0),
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
}
