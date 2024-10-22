// MIT License
//
// Copyright (c) 2023 Zakarum
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use egui::{
    epaint::Shadow,
    style::{Interaction, ScrollStyle, Spacing, TextCursorStyle, WidgetVisuals, Widgets},
    FontId, Frame, Margin, Rounding, Stroke, Style, Vec2, Visuals,
};

pub trait Zoom {
    #[inline(always)]
    fn zoomed(&self, zoom: f32) -> Self
    where
        Self: Clone,
    {
        let mut clone = self.clone();
        clone.zoom(zoom);
        clone
    }

    fn zoom(&mut self, zoom: f32);
}

impl Zoom for f32 {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        *self *= zoom;
    }
}

impl Zoom for Vec2 {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        *self *= zoom;
    }
}

impl Zoom for Rounding {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.nw.zoom(zoom);
        self.ne.zoom(zoom);
        self.se.zoom(zoom);
        self.sw.zoom(zoom);
    }
}

impl Zoom for Margin {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.left.zoom(zoom);
        self.right.zoom(zoom);
        self.top.zoom(zoom);
        self.bottom.zoom(zoom);
    }
}

impl Zoom for Shadow {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.offset.zoom(zoom);
        self.blur.zoom(zoom);
        self.spread.zoom(zoom);
    }
}

impl Zoom for Stroke {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.width *= zoom;
        if self.width < 1.0 {
            self.color.gamma_multiply(self.width);
            self.width = 1.0;
        }
    }
}

impl Zoom for WidgetVisuals {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.bg_stroke.zoom(zoom);
        self.rounding.zoom(zoom);
        self.fg_stroke.zoom(zoom);
        self.expansion.zoom(zoom);
    }
}

impl Zoom for Interaction {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.resize_grab_radius_corner.zoom(zoom);
        self.resize_grab_radius_side.zoom(zoom);
    }
}

impl Zoom for Widgets {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.noninteractive.zoom(zoom);
        self.inactive.zoom(zoom);
        self.hovered.zoom(zoom);
        self.active.zoom(zoom);
        self.open.zoom(zoom);
    }
}

impl Zoom for TextCursorStyle {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.stroke.zoom(zoom);
    }
}

impl Zoom for Visuals {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.clip_rect_margin.zoom(zoom);
        self.menu_rounding.zoom(zoom);
        self.popup_shadow.zoom(zoom);
        self.resize_corner_size.zoom(zoom);
        self.selection.stroke.zoom(zoom);
        self.text_cursor.zoom(zoom);
        self.widgets.zoom(zoom);
        self.window_rounding.zoom(zoom);
        self.window_shadow.zoom(zoom);
        self.window_stroke.zoom(zoom);
    }
}

impl Zoom for ScrollStyle {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.bar_inner_margin.zoom(zoom);
        self.bar_outer_margin.zoom(zoom);
        self.bar_width.zoom(zoom);
        self.floating_allocated_width.zoom(zoom);
        self.floating_width.zoom(zoom);
        self.handle_min_length.zoom(zoom);
    }
}

impl Zoom for Spacing {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.button_padding.zoom(zoom);
        self.combo_height.zoom(zoom);
        self.combo_width.zoom(zoom);
        self.icon_spacing.zoom(zoom);
        self.icon_width.zoom(zoom);
        self.icon_width_inner.zoom(zoom);
        self.indent.zoom(zoom);
        self.interact_size.zoom(zoom);
        self.item_spacing.zoom(zoom);
        self.menu_margin.zoom(zoom);
        self.scroll.zoom(zoom);
        self.slider_width.zoom(zoom);
        self.text_edit_width.zoom(zoom);
        self.tooltip_width.zoom(zoom);
        self.window_margin.zoom(zoom);
    }
}

impl Zoom for FontId {
    fn zoom(&mut self, zoom: f32) {
        self.size.zoom(zoom);
    }
}

impl Zoom for Style {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        if let Some(font_id) = &mut self.override_font_id {
            font_id.zoom(zoom);
        }
        for font_id in self.text_styles.values_mut() {
            font_id.zoom(zoom);
        }
        self.interaction.zoom(zoom);
        self.spacing.zoom(zoom);
        self.visuals.zoom(zoom);
    }
}

impl<T> Zoom for Option<T>
where
    T: Zoom,
{
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        if let Some(value) = self {
            value.zoom(zoom)
        }
    }
}

impl Zoom for Frame {
    #[inline(always)]
    fn zoom(&mut self, zoom: f32) {
        self.inner_margin.zoom(zoom);
        self.outer_margin.zoom(zoom);
        self.rounding.zoom(zoom);
        self.shadow.zoom(zoom);
        self.stroke.zoom(zoom);
    }
}
