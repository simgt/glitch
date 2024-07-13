/// A floating content frame.
/// As opposed to a fixed Area, this container can be nested
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FloatingArea {
    pub pos: egui::Pos2,
}

impl FloatingArea {
    pub fn new(pos: impl Into<egui::Pos2>) -> Self {
        Self { pos: pos.into() }
    }

    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        self.show_dyn(ui, Box::new(add_contents))
    }

    pub fn show_dyn<'c, R>(
        self,
        ui: &mut egui::Ui,
        add_contents: Box<dyn FnOnce(&mut egui::Ui) -> R + 'c>,
    ) -> egui::InnerResponse<R> {
        let mut inner_rect = ui.max_rect();
        inner_rect.min += self.pos.to_vec2();
        inner_rect.max = inner_rect.max.max(inner_rect.min);
        let mut content_ui = ui.child_ui_with_id_source(inner_rect, *ui.layout(), "child", None);
        let ret = add_contents(&mut content_ui);
        let response = ui.allocate_rect(content_ui.min_rect(), egui::Sense::hover());
        egui::InnerResponse::new(ret, response)
    }
}
