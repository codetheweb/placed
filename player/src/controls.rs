use egui::Context;

pub struct Controls {
    pub timescale_factor: f32,
}

impl Controls {
    pub fn new() -> Self {
        Self {
            timescale_factor: 1.0,
        }
    }

    pub fn ui(&mut self, ctx: &Context) {
        egui::Area::new("my_area")
            .fixed_pos(egui::pos2(10.0, 10.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::BLACK)
                    .rounding(5.0)
                    .inner_margin(5.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Slider::new(&mut self.timescale_factor, 0.0..=10000.0)
                                .text("Timescale"),
                        );
                        ui.label("Label with red background");
                    });
            });
    }
}
