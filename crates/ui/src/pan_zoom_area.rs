use egui::emath::TSTransform;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct PanZoomArea;

#[derive(Clone, Default, Debug)]
struct State {
    transform: TSTransform,
}

impl State {
    pub fn load(ctx: &egui::Context, id: egui::Id) -> Option<Self> {
        ctx.data_mut(|d| d.get_persisted(id))
    }

    pub fn store(self, ctx: &egui::Context, id: egui::Id) {
        ctx.data_mut(|d| d.insert_persisted(id, self));
    }
}

impl PanZoomArea {
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui, f32) -> R,
    ) -> egui::InnerResponse<R> {
        let ctx = ui.ctx().clone();
        let (id, rect) = ui.allocate_space(ui.available_size());
        let mut state = State::load(&ctx, id).unwrap_or_default();
        let response = ui.interact(rect, id, egui::Sense::click_and_drag());

        // Allow dragging the background as well.
        if response.dragged() {
            state.transform.translation += response.drag_delta();
        }

        // Plot-like reset
        if response.double_clicked() {
            state.transform = TSTransform::default();
        }

        let transform =
            TSTransform::from_translation(ui.min_rect().left_top().to_vec2()) * state.transform;

        if let Some(pointer) = ui.ctx().input(|i| i.pointer.hover_pos()) {
            // Note: doesn't catch zooming / panning if a button in this PanZoom container is hovered.
            if response.hovered() {
                let pointer_in_layer = transform.inverse() * pointer;
                let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
                let pan_delta = ui.ctx().input(|i| i.smooth_scroll_delta);

                // Zoom in on pointer:
                state.transform = state.transform
                    * TSTransform::from_translation(pointer_in_layer.to_vec2())
                    * TSTransform::from_scaling(zoom_delta)
                    * TSTransform::from_translation(-pointer_in_layer.to_vec2());

                // Pan:
                state.transform = TSTransform::from_translation(pan_delta) * state.transform;
            }
        }

        let mut inner_rect = ui.max_rect();
        inner_rect.min += transform.translation;
        inner_rect.max = inner_rect.max.max(inner_rect.min);
        let mut content_ui = ui.child_ui_with_id_source(inner_rect, *ui.layout(), "child", None);
        let ret = add_contents(&mut content_ui, state.transform.scaling);
        let response = ui.allocate_rect(content_ui.min_rect(), egui::Sense::hover());
        state.store(&ctx, id);
        egui::InnerResponse::new(ret, response)
    }
}
