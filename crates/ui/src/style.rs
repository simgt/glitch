use eframe::egui;
use egui::{Color32, Margin, Rounding, Shadow, Stroke};

pub trait GraphStyle {
    fn node_bg_color(&self) -> Color32;
    fn node_bg_hover_color(&self) -> Color32;
    fn node_bg_selected_color(&self) -> Color32;
    fn node_stroke(&self, selected: bool) -> Stroke;
    fn node_padding(&self) -> Margin;
    fn node_rounding(&self) -> Rounding;
    fn node_margin(&self) -> Margin; // FIXME use `Margin` instead
    fn node_shadow(&self) -> Shadow;
    fn port_bg_fill(&self) -> Color32;
    fn port_stroke(&self, selected: bool) -> Stroke;
    fn port_radius(&self) -> f32;
    fn link_stroke(&self, selected: bool) -> Stroke;
}

impl GraphStyle for egui::Style {
    fn node_bg_color(&self) -> Color32 {
        self.visuals
            .extreme_bg_color
            .lerp_to_gamma(self.visuals.window_fill, 0.5)
    }

    fn node_bg_hover_color(&self) -> Color32 {
        self.node_bg_color()
            .lerp_to_gamma(self.visuals.window_fill, 0.8)
    }

    fn node_bg_selected_color(&self) -> Color32 {
        self.node_bg_color()
    }

    fn node_stroke(&self, selected: bool) -> Stroke {
        if selected {
            self.visuals.selection.stroke
        } else {
            self.visuals.window_stroke
        }
    }

    fn node_padding(&self) -> Margin {
        Margin::symmetric(20.0, 15.0)
    }

    fn node_rounding(&self) -> Rounding {
        Rounding::same(5.0)
    }

    fn node_margin(&self) -> Margin {
        Margin::same(10.0)
    }

    fn node_shadow(&self) -> Shadow {
        self.visuals.window_shadow
    }

    fn port_bg_fill(&self) -> Color32 {
        self.node_bg_color()
    }

    fn port_stroke(&self, selected: bool) -> Stroke {
        self.link_stroke(selected)
    }

    fn port_radius(&self) -> f32 {
        5.0
    }

    fn link_stroke(&self, selected: bool) -> Stroke {
        if selected {
            self.visuals.selection.stroke
        } else {
            let color = self
                .visuals
                .strong_text_color()
                .gamma_multiply(0.3)
                .to_opaque();
            Stroke::new(1.0, color)
        }
    }
}
