use crate::{DAGLayout, GraphStyle, PanZoomArea, Zoom};
use egui::{self, Modifiers, Pos2, Rect, Vec2};
use glitch_data::{components::*, ser};
use log::*;
use std::{collections::HashSet, ops::Deref};

/// FIXME We can probably put all this in ctx.memory()
#[derive(Default)]
pub struct DrawState {
    size_tracker: hecs::ChangeTracker<Size>,
    topology_tracker: hecs::ChangeTracker<Child>,
    selected: Option<hecs::Entity>,
}

#[no_mangle]
pub fn draw(
    state: &mut DrawState,
    world: &mut hecs::World,
    ctx: &egui::Context,
    _frame: &mut eframe::Frame,
) {
    let mut buffer = hecs::CommandBuffer::new();
    for (entity, _) in world.query::<&Node>().without::<&Size>().iter() {
        buffer.insert_one(entity, Size(Vec2::ZERO));
    }
    buffer.run_on(world);

    let organise_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::R);
    let reorganise = ctx.input_mut(|i| i.consume_shortcut(&organise_shortcut));

    egui::CentralPanel::default()
        .frame(egui::containers::Frame {
            fill: ctx.style().visuals.window_fill.gamma_multiply(0.95),
            inner_margin: egui::Margin::symmetric(5.0, 20.0),
            ..Default::default()
        })
        .show(ctx, |ui| {
            PanZoomArea.show(ui, |ui, zoom| {
                let roots = world
                    .query::<()>()
                    .with::<&Node>()
                    .without::<&Child>()
                    .iter()
                    .map(|(e, _)| e)
                    .collect::<Vec<_>>();

                let node_margin = ui.style().node_margin().zoomed(zoom);
                let layout = DAGLayout::new(node_margin.sum());

                let parent_nodes = world
                    .query::<&Child>()
                    .with::<&Node>()
                    .iter()
                    .map(|(_, c)| c.parent)
                    .collect::<HashSet<_>>();

                let topology_changed = {
                    // FIXME should also track links
                    let mut changes = state.topology_tracker.track(world);
                    changes.changed().count() > 0
                        || changes.added().count() > 0
                        || changes.removed().count() > 0
                };

                if topology_changed || reorganise {
                    // FIXME Only relayout the trees that have changed
                    // Should be easy to trace back to the roots with ancestors and a set
                    info!("Relayouting");
                    let mut buffer = hecs::CommandBuffer::new();
                    for &entity in parent_nodes.iter() {
                        if let Err(e) = layout.update_topology(world, entity, &mut buffer) {
                            error!("Error during topology update: {e}");
                        }
                    }
                    buffer.run_on(world);
                }

                let size_changed = {
                    let mut changes = state.size_tracker.track(world);
                    changes.changed().count() > 0 || changes.added().count() > 0
                };

                if size_changed {
                    let mut buffer = hecs::CommandBuffer::new();
                    for &entity in parent_nodes.iter() {
                        if let Err(e) = layout.update_positions(world, entity, &mut buffer) {
                            error!("Error during node positioning: {e}");
                        }
                    }

                    // Insert a position for each root in the world, by using the size of the previous one
                    let margin = node_margin.sum();
                    let mut y = 0.0;
                    for root in roots.iter().cloned() {
                        let size = world.get::<&Size>(root).unwrap().0;
                        buffer.insert_one(root, Pos2::new(0.0, y));
                        y += size.y + 2.0 * margin.y;
                    }

                    buffer.run_on(world);
                }

                // Draw the graphs and update the selected entity if needed
                state.selected = roots.into_iter().fold(state.selected, |selected, root| {
                    draw_node(ui, world, root, zoom, state.selected).or(selected)
                });

                if ui.interact_bg(egui::Sense::click()).clicked() {
                    state.selected = None;
                }
            });

            #[cfg(debug_assertions)]
            draw_debug_window(ctx, world);
        });

    egui::SidePanel::right("inspector")
        .resizable(false)
        .min_width(200.0)
        .frame(egui::containers::Frame {
            fill: ctx.style().visuals.window_fill.gamma_multiply(0.95),
            inner_margin: egui::Margin::symmetric(10.0, 20.0),
            ..Default::default()
        })
        .show_animated(ctx, state.selected.is_some(), |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let selected = state.selected.unwrap();
                egui::Grid::new("info")
                    .striped(true)
                    .spacing(egui::vec2(8.0, 8.0))
                    .show(ui, |ui| {
                        #[cfg(debug_assertions)]
                        {
                            ui.label("Entity");
                            ui.label(format!("{:?}", selected));
                            ui.end_row();
                        }

                        if let Ok(name) = world.get::<&Name>(selected) {
                            ui.label("Name");
                            ui.label(name.0.clone());
                            ui.end_row();
                        }

                        if let Ok(typename) = world.get::<&TypeName>(selected) {
                            ui.label("Type");
                            ui.label(typename.0.clone());
                            ui.end_row();
                        }

                        if let Ok(state) = world.get::<&State>(selected) {
                            ui.label("State");
                            ui.label(format!("{state:?}"));
                            ui.end_row();
                        }
                    });

                if let Ok(properties) = world.get::<&Properties>(selected) {
                    ui.add_space(10.0);
                    egui::ScrollArea::horizontal()
                        .max_width(200.0)
                        .show(ui, |ui| {
                            egui::CollapsingHeader::new("Properties")
                                .default_open(true)
                                .show_unindented(ui, |ui| {
                                    egui::Grid::new("properties")
                                        .striped(true)
                                        .spacing(egui::vec2(8.0, 8.0))
                                        .show(ui, |ui| {
                                            for (key, value) in properties.0.iter() {
                                                ui.label(key);
                                                ui.label(value);
                                                ui.end_row();
                                            }
                                        });
                                });
                        });
                }
            });
        });

    if reorganise {
        ctx.memory_mut(|mem| mem.reset_areas());
    }

    // FIXME: request a repaint if we are displaying a feed on a texture
}

#[cfg(debug_assertions)]
fn draw_debug_window(ctx: &egui::Context, world: &mut hecs::World) {
    egui::Window::new("Debug")
        .default_width(200.0)
        .default_open(false)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::Vec2::new(10.0, -10.0))
        .show(ctx, |ui| {
            egui::Grid::new("info")
                .striped(true)
                .spacing(egui::vec2(8.0, 8.0))
                .show(ui, |ui| {
                    ui.label("World size:");
                    ui.label(world.len().to_string());
                    ui.end_row();

                    ui.label("Number of nodes:");
                    ui.label(
                        world
                            .query::<()>()
                            .with::<&Node>()
                            .iter()
                            .count()
                            .to_string(),
                    );
                    ui.end_row();

                    ui.label("Number of edges:");
                    ui.label(
                        world
                            .query::<()>()
                            .with::<&Edge>()
                            .iter()
                            .count()
                            .to_string(),
                    );
                    ui.end_row();

                    ui.label("Number of ports:");
                    ui.label(
                        world
                            .query::<()>()
                            .with::<&Port>()
                            .iter()
                            .count()
                            .to_string(),
                    );
                    ui.end_row();

                    ui.label("Debug on hover");
                    let mut debug_on_hover = ctx.debug_on_hover();
                    ui.checkbox(&mut debug_on_hover, "enable");
                    ctx.set_debug_on_hover(debug_on_hover);
                    ui.end_row();
                });

            if ui.button("Reset").clicked() {
                world.clear();
            }

            if ui.button("Save").clicked() {
                let mut file = std::fs::File::create("checkpoint.ron").unwrap();
                let mut serializer = ron::Serializer::with_options(
                    &mut file,
                    Some(ron::ser::PrettyConfig::default()),
                    Default::default(),
                )
                .unwrap();
                hecs::serialize::row::serialize(&world, &mut ser::SerContext, &mut serializer)
                    .unwrap();
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                ctx.settings_ui(ui);
            })
        });
}

fn draw_node(
    ui: &mut egui::Ui,
    world: &mut hecs::World,
    entity: hecs::Entity,
    zoom: f32,
    current_selection: Option<hecs::Entity>,
) -> Option<hecs::Entity> {
    let style = ui.ctx().style();
    let mut proposed_selection = None;

    let is_root = world.get::<&Child>(entity).is_err();
    let selected = current_selection == Some(entity);

    let name = world.get::<&Name>(entity).ok()?.deref().clone();
    let pos = world.get::<&Pos2>(entity).ok()?.deref().clone();

    // FIXME It'd apparently be better to use Areas for this, and they'll handle the click better
    // as long as they are drawn back to front
    let mut inner_rect = ui.max_rect();
    inner_rect.min += pos.to_vec2();
    inner_rect.max = inner_rect.max.max(inner_rect.min);
    let mut child_ui = ui.child_ui_with_id_source(inner_rect, *ui.layout(), "child", None);

    // We need to do the click interaction before recurring in the children, otherwise the root
    // node is always getting the click. The drawback is that we use the previous position and size
    if let Ok(size) = world.get::<&Size>(entity) {
        let r = child_ui.interact(
            Rect::from_min_size(inner_rect.min, size.0),
            child_ui.id().with(("click", entity)),
            egui::Sense::click(),
        );
        if r.clicked() {
            proposed_selection = Some(entity);
        }
    }

    let mut prepared_frame = egui::Frame::default()
        .rounding(style.node_rounding())
        .inner_margin(style.node_padding())
        .stroke(style.node_stroke(selected))
        .fill(style.node_bg_color())
        .shadow(if is_root {
            style.node_shadow()
        } else {
            egui::Shadow::NONE
        })
        .zoomed(zoom)
        .begin(&mut child_ui);

    {
        let ui = &mut prepared_frame.content_ui;

        let children = world
            .query::<&Child>()
            .with::<&Node>()
            .iter()
            .filter_map(|(e, c)| if c.parent == entity { Some(e) } else { None })
            .collect::<Vec<_>>();

        let r = ui.horizontal(|ui| {
            let font = ui
                .style()
                .text_styles
                .get(&egui::TextStyle::Heading)
                .unwrap()
                .clone()
                .zoomed(zoom);
            ui.add(
                egui::Label::new(egui::RichText::new(&name.0).font(font.clone())).selectable(false),
            );
            if let Ok(state) = world.get::<&State>(entity) {
                let state_label = match *state {
                    State::Playing => "▶",
                    State::Paused => "⏸",
                    State::Null => "⏺",
                    State::Ready => "⏹",
                    _ => "??",
                };
                ui.add(
                    egui::Label::new(egui::RichText::new(state_label).font(font)).selectable(false),
                );
            }
        });

        if !children.is_empty() {
            // FIXME put a button in the top left corner instead
            let r = ui.interact(
                r.response.rect,
                ui.id().with(("interact", entity)),
                egui::Sense::click(),
            );

            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.id().with(("node", entity)),
                true,
            );

            if r.clicked() {
                state.toggle(ui);
            }

            state.show_body_unindented(ui, |ui| {
                let edges: Vec<_> = world
                    .query::<&Edge>()
                    .iter()
                    .filter_map(|(_, edge)| {
                        // If the link is connected to a port that belongs to
                        // one of the children of this node, we will draw it
                        if children.iter().cloned().any(|c| {
                            world
                                .parent(edge.output_port)
                                .map(|n| c == n)
                                .unwrap_or(false)
                                || world
                                    .parent(edge.input_port)
                                    .map(|n| c == n)
                                    .unwrap_or(false)
                        }) {
                            Some(edge.to_owned())
                        } else {
                            None
                        }
                    })
                    .collect();

                let where_to_put_links = ui.painter().add(egui::Shape::Noop);
                let zoom = zoom * 0.75;

                // Recursively draw the children and take their selection first if there is any
                proposed_selection =
                    children
                        .iter()
                        .cloned()
                        .fold(proposed_selection, |selected, child| {
                            draw_node(ui, world, child, zoom, current_selection).or(selected)
                        });

                // Draw the links
                // FIXME allow selecting the links
                let mut shapes = Vec::new();
                for link in edges.iter() {
                    let from = match world.get::<&Pos2>(link.output_port) {
                        Ok(pos) => pos,
                        Err(_) => {
                            error!("Link output port not found");
                            continue;
                        }
                    };
                    let to = match world.get::<&Pos2>(link.input_port) {
                        Ok(pos) => pos,
                        Err(_) => {
                            error!("Link input port not found");
                            continue;
                        }
                    };

                    shapes.push(epaint::Shape::CubicBezier(
                        epaint::CubicBezierShape::from_points_stroke(
                            compute_bezier_points(*from, *to, 0.5),
                            false,
                            egui::Color32::TRANSPARENT,
                            style.link_stroke().zoomed(zoom),
                        ),
                    ));
                }

                ui.painter()
                    .set(where_to_put_links, egui::Shape::Vec(shapes));
            });
        }
    }

    let r = prepared_frame.allocate_space(&mut child_ui);

    prepared_frame.frame.fill = if r.hovered() {
        style.node_bg_hover_color()
    } else {
        style.node_bg_color()
    };

    let r = ui.allocate_rect(child_ui.min_rect(), egui::Sense::hover());
    world.insert_one(entity, Size(r.rect.size())).unwrap();

    prepared_frame.paint(&child_ui);

    // Draw the ports
    proposed_selection = draw_ports(
        &mut child_ui,
        world,
        entity,
        Port::Output,
        r.rect,
        zoom,
        current_selection,
    )
    .or(proposed_selection);

    proposed_selection = draw_ports(
        &mut child_ui,
        world,
        entity,
        Port::Input,
        r.rect,
        zoom,
        current_selection,
    )
    .or(proposed_selection);

    proposed_selection
}

fn draw_ports(
    ui: &mut egui::Ui,
    world: &mut hecs::World,
    parent: hecs::Entity,
    direction: Port,
    rect: Rect,
    zoom: f32,
    current_selection: Option<hecs::Entity>,
) -> Option<hecs::Entity> {
    let painter = ui.painter();
    let s = ui.style();

    let mut proposed_selection = None;

    let entities = world
        .query::<(&Child, &Port)>()
        .iter()
        .filter_map(|(entity, (child, &port))| {
            if child.parent == parent && port == direction {
                Some(entity)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let (top, bottom) = match direction {
        Port::Input => (rect.left_top(), rect.left_bottom()),
        Port::Output => (rect.right_top(), rect.right_bottom()),
    };

    for (index, entity) in entities.iter().cloned().enumerate() {
        let selected = current_selection.map_or(false, |s| s == parent || s == entity);

        let pos = top.lerp(bottom, (index as f32 + 1.0) / (entities.len() as f32 + 1.0));
        painter.circle(
            pos,
            s.port_radius() * zoom,
            s.port_bg_fill(),
            s.port_stroke(selected).zoomed(zoom),
        );

        let response = ui.interact(
            Rect::from_center_size(pos, Vec2::splat(s.port_radius() * 2.0 * zoom)),
            ui.id().with(("port", entity)),
            egui::Sense::click(),
        );
        if response.clicked() {
            proposed_selection = Some(entity);
        }

        world.insert_one(entity, pos).unwrap();
    }

    proposed_selection
}

fn compute_bezier_points(from: Pos2, to: Pos2, curvature: f32) -> [Pos2; 4] {
    let dx = to.x - from.x;
    let control_x_offset = dx * curvature;
    let control1 = Pos2::new(from.x + control_x_offset, from.y);
    let control2 = Pos2::new(to.x - control_x_offset, to.y);
    [from, control1, control2, to]
}
