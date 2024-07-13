use crate::{DAGLayout, FloatingArea, PanZoomArea, Zoom};
use anyhow::{Context, Result};
use egui::{self, emath::TSTransform, Modifiers, Pos2};
use glitch_data::{components::*, *};
use hecs_hierarchy::{Hierarchy, HierarchyMut};
use log::*;
use petgraph::graphmap::DiGraphMap;
use std::collections::HashMap;

pub struct AppState {
    world: hecs::World,
    #[allow(dead_code)]
    rt: tokio::runtime::Runtime,
    rx: tokio::sync::mpsc::Receiver<Event>,
    dag_layout: DAGLayout,
    size_tracker: hecs::ChangeTracker<Size>,
}

impl AppState {
    pub fn new(_ctx: &egui::Context) -> Result<Self> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.spawn(serve(tx));

        Ok(Self {
            world: hecs::World::new(),
            rt,
            rx,
            dag_layout: DAGLayout::default(),
            size_tracker: hecs::ChangeTracker::default(),
        })
    }

    pub fn update(&mut self) {
        // Process events while there are any
        // We're receiving events in a way that doesn't seem logical, for instance
        // in the case of decodebin pads are linked before being added, etc.
        // To account for that we always tentatively create related entities
        while let Ok(event) = self.rx.try_recv() {
            debug!("Received event: {event:?}");
            match event {
                Event::NewElement(element) => {
                    let _ = self.world.update_element(element);
                }
                Event::ChangeElementState { element, state } => {
                    let e = self.world.update_element(element);
                    let _ = self.world.insert_one(e, state);
                }
                Event::AddChildElement { child, parent } => {
                    let child = self.world.update_element(child);
                    let parent = self.world.update_element(parent);
                    self.world.attach::<ElementTree>(child, parent).unwrap();
                }
                Event::AddPad { pad, element } => {
                    let node = self.world.update_element(element);
                    let pin = self
                        .world
                        .find_pad(pad.id)
                        .unwrap_or_else(|| self.world.spawn_pad(pad));
                    // Link the pin to its node, we're keeping the hierarchy separate to be
                    // able to efficiently query leaf nodes
                    self.world.attach::<PadTree>(pin, node).unwrap();
                }
                Event::LinkPad {
                    src_pad, sink_pad, ..
                } => {
                    let out_pin = self
                        .world
                        .find_pad(src_pad.id)
                        .unwrap_or_else(|| self.world.spawn_pad(src_pad));
                    let in_pin = self
                        .world
                        .find_pad(sink_pad.id)
                        .unwrap_or_else(|| self.world.spawn_pad(sink_pad));
                    self.world.spawn((Link {
                        from_pad: out_pin,
                        to_pad: in_pin,
                    },));
                }
            }
        }

        {
            let changed = {
                let mut changes = self.size_tracker.track(&mut self.world);
                changes.changed().count() > 0 || changes.added().count() > 0
            };

            if changed {
                // FIXME only relayout the trees that have changed
                // Should be easy to trace back to the roots with ancestors and a set
                self.relayout();
            }
        }
    }

    pub fn relayout(&mut self) {
        info!("Relayout");
        let roots: Vec<hecs::Entity> = self
            .world
            .roots::<ElementTree>()
            .unwrap()
            .iter()
            .map(|(e, _)| e)
            .collect();
        for root in roots.iter().cloned() {
            if let Err(e) = self.layout_tree(root) {
                error!("Error during layout: {e}");
            }
        }

        // Insert a position for each root in the world, by using the size of the previous one
        let mut y = 0.0;
        let margin = 10.0;
        for root in roots.iter().cloned() {
            let size = self.world.get::<&Size>(root).unwrap().0;
            self.world
                .insert_one(root, egui::Pos2::new(0.0, y))
                .unwrap();
            y += size.y + margin;
        }
    }

    fn layout_tree(&mut self, root: hecs::Entity) -> Result<()> {
        // FIXME we should take the queries as params to avoid borrowing the entire world
        debug!("Layout from {root:?}");

        // Create a graph of the children of the given root
        // For now we ignore the actual pads when deciding on the layout
        let mut graph = DiGraphMap::<hecs::Entity, ()>::new();
        let mut node_sizes = HashMap::new();

        for child in self.world.children::<ElementTree>(root).collect::<Vec<_>>() {
            // If the child isn't a leaf, recurse into it
            if self
                .world
                .satisfies::<(&Element, &ParentElement)>(child)
                .unwrap()
            {
                debug!("Recursing into {child:?}");
                if let Err(e) = self.layout_tree(child) {
                    error!("Error during layout of child: {e}");
                }
            }

            // Query its size for use in the layout
            // The size are being set when drawing nodes, leafs need to be given
            // a size at init even if they haven't been positioned yet
            node_sizes.insert(
                child,
                self.world
                    .get::<&Size>(child)
                    .context("Missing size in a child, cannot layout yet")?
                    .0,
            );

            graph.add_node(child);
        }

        // Iterate over all the wires, and add the edges that are to and from
        // nodes of this graph
        for (_, (wire,)) in self.world.query::<(&Link,)>().iter() {
            let Ok(from_node) = self
                .world
                .parent::<PadTree>(wire.from_pad)
                .with_context(|| format!("Source pin {:?} doesn't have a parent", wire.from_pad))
                .inspect_err(|e| error!("{e}"))
            else {
                continue;
            };

            let Ok(to_node) = self
                .world
                .parent::<PadTree>(wire.to_pad)
                .with_context(|| format!("Sink pin {:?} doesn't have a parent", wire.to_pad))
                .inspect_err(|e| error!("{e}"))
            else {
                continue;
            };

            if graph.contains_node(from_node) && graph.contains_node(to_node) {
                graph.add_edge(from_node, to_node, ());
            }
        }

        info!("Graph of {root:?}: {graph:?}");

        for (node, pos) in self.dag_layout.layout(&graph, &node_sizes)?.into_iter() {
            self.world
                .insert_one(node, pos)
                .expect("Failed to insert position");
        }

        self.world
            .insert_one(root, egui::Pos2::ZERO)
            .expect("Failed to insert root position");

        Ok(())
    }
}

trait WorldExt {
    fn find_element(&self, id: ObjectId) -> Option<hecs::Entity>;
    fn update_element(&mut self, element: Element) -> hecs::Entity;
    fn find_pad(&self, id: ObjectId) -> Option<hecs::Entity>;
    fn spawn_pad(&mut self, pad: Pad) -> hecs::Entity;
}

impl WorldExt for hecs::World {
    fn find_element(&self, id: ObjectId) -> Option<hecs::Entity> {
        self.query::<(&Element,)>()
            .iter()
            .find_map(
                |(entity, (element,))| {
                    if element.id == id {
                        Some(entity)
                    } else {
                        None
                    }
                },
            )
    }

    /// Update an element if it already exists, or spawn a new one if it doesn't
    fn update_element(&mut self, element: Element) -> hecs::Entity {
        if let Some(e) = self.find_element(element.id) {
            self.insert_one(e, element).unwrap();
            e
        } else {
            // FIXME instead of putting a dummy size here, we should query the
            // nodes without a size and draw them on an invisible ui to bootstrap
            // the layout
            self.spawn((element, Size(egui::Vec2::ZERO)))
        }
    }

    fn find_pad(&self, id: ObjectId) -> Option<hecs::Entity> {
        self.query::<(&Pad,)>()
            .iter()
            .find_map(|(entity, (pad,))| if pad.id == id { Some(entity) } else { None })
    }

    fn spawn_pad(&mut self, pad: Pad) -> hecs::Entity {
        self.spawn((pad,))
    }
}

#[no_mangle]
pub fn render(state: &mut AppState, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    state.update();

    let organise_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::O);
    let refresh_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::R);

    egui::CentralPanel::default()
        .frame(egui::containers::Frame {
            fill: ctx.style().visuals.window_fill.gamma_multiply(0.95),
            ..Default::default()
        })
        .show(ctx, |ui| {
            PanZoomArea.show(ui, |ui, transform| {
                let mut buffer = hecs::CommandBuffer::new();
                let roots = state
                    .world
                    .roots::<ElementTree>()
                    .unwrap()
                    .iter()
                    .map(|(e, _)| e)
                    .collect::<Vec<_>>();
                FloatingArea::new(transform.translation.to_pos2()).show(ui, |ui| {
                    for root in roots {
                        draw_element(ui, &state.world, root, &mut buffer, &transform);
                    }
                });

                buffer.run_on(&mut state.world);
            });
        });

    egui::SidePanel::right("debug")
        .default_width(200.0)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("info")
                .striped(true)
                .spacing(egui::vec2(8.0, 8.0))
                .show(ui, |ui| {
                    ui.label("World size:");
                    ui.label(state.world.len().to_string());
                    ui.end_row();

                    ui.label("Elements:");
                    ui.label(
                        state
                            .world
                            .query::<(&Element,)>()
                            .iter()
                            .count()
                            .to_string(),
                    );
                    ui.end_row();

                    ui.label("Wires:");
                    ui.label(state.world.query::<(&Link,)>().iter().count().to_string());
                    ui.end_row();

                    ui.label("Debug on hover");
                    let mut debug_on_hover = ctx.debug_on_hover();
                    ui.checkbox(&mut debug_on_hover, "enable");
                    ctx.set_debug_on_hover(debug_on_hover);
                    ui.end_row();
                });
        });

    if ctx.input_mut(|i| i.consume_shortcut(&organise_shortcut)) {
        ctx.memory_mut(|mem| mem.reset_areas());
    }

    if ctx.input_mut(|i| i.consume_shortcut(&refresh_shortcut)) {
        state.relayout();
    }

    // FIXME: not necessary unless we're displaying a feed in a texture
    ctx.request_repaint();
}

fn draw_element(
    ui: &mut egui::Ui,
    world: &hecs::World,
    entity: hecs::Entity,
    buffer: &mut hecs::CommandBuffer,
    transform: &TSTransform,
) {
    debug!("Drawing node {:?}", entity);
    let element = world.get::<&Element>(entity).unwrap();
    let pos = *world.get::<&egui::Pos2>(entity).unwrap();
    let r = FloatingArea::new(pos).show(ui, |ui| {
        egui::Frame::default()
            .rounding(egui::Rounding::same(4.0))
            .inner_margin(egui::Margin::same(8.0))
            .outer_margin(egui::Margin::same(4.0))
            .stroke(ui.ctx().style().visuals.window_stroke)
            .fill(ui.style().visuals.panel_fill)
            .zoomed(transform.scaling)
            .show(ui, |ui| {
                let draw_header = |ui: &mut egui::Ui| {
                    ui.horizontal(|ui| {
                        let mut font = ui
                            .style()
                            .text_styles
                            .get(&egui::TextStyle::Body)
                            .unwrap()
                            .clone();
                        font.zoom(transform.scaling);
                        ui.label(egui::RichText::new(&element.name).font(font.clone()));
                        if let Ok(state) = world.get::<&ElementState>(entity) {
                            let state_label = match *state {
                                ElementState::Playing => "▶",
                                ElementState::Paused => "⏸",
                                ElementState::Null => "⏺",
                                ElementState::Ready => "⏹",
                            };
                            ui.label(egui::RichText::new(state_label).font(font));
                        }
                    });
                };
                let children = world.children::<ElementTree>(entity).collect::<Vec<_>>();

                if children.is_empty() {
                    draw_header(ui);
                } else {
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        ui.id().with(("node", &element.id)),
                        true,
                    )
                    .show_header(ui, draw_header)
                    .body_unindented(|ui| {
                        // Make a new transform without translation but with decreased scaling
                        let transform = TSTransform::from_scaling(transform.scaling * 0.75);
                        // Recurse in the children
                        for child in children.iter().cloned() {
                            draw_element(ui, world, child, buffer, &transform);
                        }

                        // Draw the wires of this level
                        for (_, (wire,)) in world.query::<(&Link,)>().iter() {
                            // If the link is connected to a pad that belongs to
                            // one of the children of this node, draw it
                            if children.iter().cloned().any(|c| {
                                world
                                    .parent::<PadTree>(wire.from_pad)
                                    .map(|n| c == n)
                                    .unwrap_or(false)
                                    || world
                                        .parent::<PadTree>(wire.to_pad)
                                        .map(|n| c == n)
                                        .unwrap_or(false)
                            }) {
                                let _ = draw_link(ui, world, wire, &transform);
                            }
                        }
                    });
                }
            })
    });
    buffer.insert_one(entity, Size(r.response.rect.size()));
}

fn draw_link(
    ui: &mut egui::Ui,
    world: &hecs::World,
    wire: &Link,
    transform: &TSTransform,
) -> Result<()> {
    fn compute_bezier_points(from: Pos2, to: Pos2, curvature: f32) -> [Pos2; 4] {
        let dx = to.x - from.x;
        let control_x_offset = dx * curvature;
        let control1 = Pos2::new(from.x + control_x_offset, from.y);
        let control2 = Pos2::new(to.x - control_x_offset, to.y);
        [from, control1, control2, to]
    }

    // FIXME: we should have direct access to the pins' positions
    // by recomputing them when needed and storing them as component
    let orig = ui.max_rect().min.to_vec2();
    let from = world
        .parent::<PadTree>(wire.from_pad)
        .context("Source node not found")
        .and_then(|node| {
            let mut q = world.query_one::<(&egui::Pos2, &Size)>(node)?;
            let (p, size) = q.get().ok_or(hecs::ComponentError::NoSuchEntity)?;
            Ok(egui::Rect::from_min_size(*p + orig, size.0).right_center())
        })
        .map(|p| p + transform.translation)?;
    let to = world
        .parent::<PadTree>(wire.to_pad)
        .context("Sink node not found")
        .and_then(|node| {
            let mut q = world.query_one::<(&egui::Pos2, &Size)>(node)?;
            let (p, size) = q.get().ok_or(hecs::ComponentError::NoSuchEntity)?;
            Ok(egui::Rect::from_min_size(*p + orig, size.0).left_center())
        })
        .map(|p| p + transform.translation)?;
    let color = egui::Color32::from_rgb(0x88, 0x88, 0x88);
    let stroke = egui::Stroke::new(2.0, color).zoomed(transform.scaling);
    // Draw a quadratic bezier curve
    ui.painter()
        .add(epaint::CubicBezierShape::from_points_stroke(
            compute_bezier_points(from, to, 0.5),
            false,
            egui::Color32::TRANSPARENT,
            stroke,
        ));
    Ok(())
}
