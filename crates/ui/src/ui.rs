use crate::{DAGLayout, GraphStyle, Zoom};
use anyhow::Result;
use egui::{self, collapsing_header::CollapsingState, Modifiers, Pos2, Rect, Vec2};
use egui_extras::{Column, TableBuilder};
use glitch_common::{
    comps::*,
    ser::{load_datastore, save_datastore},
    DataStore, ViewMode,
};
use hecs::Entity;
use log::*;
use std::{collections::HashSet, ops::Deref};

/// Helper function to sort entities by name, then by entity ID for consistent ordering
fn sort_entities_by_name<I>(world: &hecs::World, entities: I) -> Vec<hecs::Entity>
where
    I: Iterator<Item = hecs::Entity>,
{
    let mut entities_with_names: Vec<_> = entities
        .map(|entity| {
            let name = world
                .get::<&Name>(entity)
                .ok()
                .map(|n| n.0.clone())
                .unwrap_or_default();
            (name, entity)
        })
        .collect();

    entities_with_names.sort();

    entities_with_names
        .into_iter()
        .map(|(_, entity)| entity)
        .collect()
}

/// FIXME We can probably put this in ctx.memory()
pub struct UiState {
    show_left_panel: bool,
    show_right_panel: bool,
    show_debug_window: bool,
    size_tracker: hecs::ChangeTracker<Size>,
    tree_change_tracker: hecs::ChangeTracker<Child>,
    graph_change_tracker: hecs::ChangeTracker<Edge>,
    current_selection: Selection,
    scene_rect: Rect,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_left_panel: true,
            show_right_panel: true,
            show_debug_window: false,
            size_tracker: Default::default(),
            tree_change_tracker: Default::default(),
            graph_change_tracker: Default::default(),
            current_selection: Default::default(),
            scene_rect: Rect::ZERO,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    #[default]
    None,
    Entity(hecs::Entity),
}

impl Selection {
    pub fn or(self, other: Self) -> Self {
        match self {
            Selection::None => other,
            _ => self,
        }
    }

    pub fn is_entity(&self) -> bool {
        matches!(self, Selection::Entity(_))
    }

    pub fn map_entity_or<U, F>(self, default: U, f: F) -> U
    where
        F: FnOnce(Entity) -> U,
    {
        match self {
            Selection::Entity(entity) => f(entity),
            _ => default,
        }
    }
}

trait ChangesExt {
    fn any(&mut self) -> bool;
}

impl<T: hecs::Component + Clone + PartialEq> ChangesExt for hecs::Changes<'_, T> {
    fn any(&mut self) -> bool {
        self.changed().count() > 0 || self.added().count() > 0 || self.removed().count() > 0
    }
}

#[no_mangle]
pub fn show_ui(
    state: &mut UiState,
    data_store: &mut DataStore,
    ctx: &egui::Context,
    _frame: &mut eframe::Frame,
) {
    // Add a dummy size for all the nodes that don't have one yet
    // to bootstrap the layout: the layout algorithm needs a size for each
    // node to compute a position and display function needs a position for
    // rawing and computing a size.
    let mut buffer = hecs::CommandBuffer::new();
    for (entity, _) in data_store
        .current_world()
        .query::<&Node>()
        .without::<&Size>()
        .iter()
    {
        buffer.insert_one(entity, Size(Vec2::new(20.0, 10.0)));
    }
    buffer.run_on(data_store.current_world_mut());

    let organise_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::R);
    let mut reorganise = ctx.input_mut(|i| i.consume_shortcut(&organise_shortcut));

    egui::TopBottomPanel::top("top_panel")
        .frame(egui::Frame::default().inner_margin(egui::Margin {
            left: if cfg!(target_os = "macos") { 76 } else { 6 },
            top: 6,
            bottom: 6,
            ..Default::default()
        }))
        .show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let now = chrono::Local::now();

                ui.menu_button("File", |ui| {
                    let file_name = format!("glitch {}.ron", now.format("%Y-%m-%d %H.%M"));
                    let dialog = rfd::FileDialog::new()
                        .set_file_name(&file_name)
                        .add_filter("Glitch DataStore Files", &["ron"]);

                    if ui.button("Open...").clicked() {
                        if let Some(path) = dialog.clone().pick_file() {
                            info!("Loading datastore from {path:?}");
                            match load_datastore(path) {
                                Ok(loaded_datastore) => {
                                    *data_store = loaded_datastore;
                                    info!(
                                        "Successfully loaded datastore with {} commands",
                                        data_store.history_len()
                                    );

                                    state.size_tracker = Default::default();
                                    state.tree_change_tracker = Default::default();
                                    state.graph_change_tracker = Default::default();

                                    // FIXME this shouldn't be necessary as the trackers should detect the changes
                                    reorganise = true;
                                }
                                Err(e) => {
                                    error!("Failed to load datastore: {e}");
                                }
                            }
                        }
                    }

                    if ui.button("Save as...").clicked() {
                        if let Some(path) = dialog.save_file() {
                            info!("Saving datastore to {path:?}");
                            match save_datastore(data_store, path) {
                                Ok(()) => {
                                    info!(
                                        "Successfully saved datastore with {} commands",
                                        data_store.history_len()
                                    );
                                }
                                Err(e) => {
                                    error!("Failed to save datastore: {e}");
                                }
                            }
                        }
                    }

                    if ui.button("Clear").clicked() {
                        *data_store = DataStore::default();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui
                        .button(if state.show_left_panel {
                            "Hide Left Panel"
                        } else {
                            "Show Left Panel"
                        })
                        .clicked()
                    {
                        state.show_left_panel = !state.show_left_panel;
                    }
                    if ui
                        .button(if state.show_right_panel {
                            "Hide Right Panel"
                        } else {
                            "Show Right Panel"
                        })
                        .clicked()
                    {
                        state.show_right_panel = !state.show_right_panel;
                    }

                    if ui
                        .button(if state.show_debug_window {
                            "Hide Debug Window"
                        } else {
                            "Show Debug Window"
                        })
                        .clicked()
                    {
                        state.show_debug_window = !state.show_debug_window;
                    }
                });
            });
        });

    egui::TopBottomPanel::bottom("timeline")
        .frame(egui::Frame::default().inner_margin(egui::Margin::symmetric(6, 6)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Timeline:");

                // Step backward button
                let can_step_back = data_store.can_step_backward();
                let back_tooltip = if can_step_back {
                    "Step backward in time"
                } else {
                    "No earlier commands available"
                };
                if ui
                    .add_enabled(can_step_back, egui::Button::new("◀ Back"))
                    .on_hover_text(back_tooltip)
                    .clicked()
                {
                    data_store.step_backward();
                }

                // Step forward button
                let can_step_forward = data_store.can_step_forward();
                let forward_tooltip = if can_step_forward {
                    "Step forward in time"
                } else {
                    "Already at the latest position"
                };
                if ui
                    .add_enabled(can_step_forward, egui::Button::new("Forward ▶"))
                    .on_hover_text(forward_tooltip)
                    .clicked()
                {
                    data_store.step_forward();
                }

                ui.separator();

                // Rolling mode toggle button
                let is_rolling = matches!(data_store.current_view_mode, ViewMode::Rolling);
                let (button_text, button_tooltip) = if is_rolling {
                    ("⏸ Pause", "Stop live updates and freeze at current state")
                } else {
                    ("▶ Live", "Resume live updates")
                };
                if ui
                    .button(button_text)
                    .on_hover_text(button_tooltip)
                    .clicked()
                {
                    if is_rolling {
                        // Stop at current position (latest timestamp)
                        if let Some(&latest) = data_store.command_history.keys().next_back() {
                            data_store.set_view(ViewMode::Specific(latest));
                        }
                    } else {
                        data_store.toggle_rolling_mode();
                    }
                }

                ui.separator();

                // Show current position with better formatting
                match (data_store.current_timeline_position(), is_rolling) {
                    (Some(position), true) => {
                        ui.label(format!("Position: {} (Live)", position));
                    }
                    (Some(position), false) => {
                        ui.label(format!("Position: {} (Paused)", position));
                    }
                    (None, _) => {
                        ui.label("No commands recorded");
                    }
                }
            });

            // Add the timeline slider below the buttons
            if let Some(range) = data_store.timestamp_bounds() {
                let mut current_position = data_store
                    .current_timeline_position()
                    .unwrap_or(*range.start());
                let prev_position = current_position;

                ui.vertical(|ui| {
                    ui.style_mut().spacing.slider_width = ui.available_width();
                    ui.add(
                        egui::Slider::new(&mut current_position, range)
                            .clamping(egui::SliderClamping::Always)
                            .show_value(false)
                            .trailing_fill(true)
                            .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.5 }),
                    );
                });

                if prev_position != current_position {
                    data_store.set_view(ViewMode::Specific(current_position));
                }
            }
        });

    let side_panel_frame = egui::Frame {
        fill: ctx.style().visuals.window_fill.gamma_multiply(0.95),
        inner_margin: egui::Margin::symmetric(10, 10),
        ..Default::default()
    };

    if state.show_left_panel && data_store.current_world().query::<&Node>().iter().count() > 0 {
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .frame(side_panel_frame)
            .show(ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    let nodes = {
                        let world = data_store.current_world();
                        let root_entities: Vec<_> = world
                            .query::<&Node>()
                            .iter()
                            .filter_map(|(entity, _)| {
                                if world.get::<&Child>(entity).is_err() {
                                    Some(entity)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        sort_entities_by_name(world, root_entities.into_iter())
                    };

                    // Collect all the ancestors of the current selection
                    let mut selected_entities = Vec::new();
                    if let Selection::Entity(selected) = state.current_selection {
                        let view = data_store.current_world().view::<&Child>();
                        let mut current = Some(selected);
                        while let Some(entity) = current {
                            selected_entities.push(entity);
                            current = view.get(entity).map(|c| c.parent);
                        }
                    }

                    state.current_selection =
                        nodes
                            .into_iter()
                            .fold(state.current_selection, |selection, node| {
                                show_node_tree(
                                    ui,
                                    data_store.current_world(),
                                    node,
                                    &selected_entities,
                                    0,
                                )
                                .or(selection)
                            });
                });
            });
    }

    if state.show_right_panel {
        egui::SidePanel::right("right_panel")
            .resizable(true)
            .default_width(250.0)
            .min_width(200.0)
            .frame(side_panel_frame)
            .show_animated(ctx, state.current_selection.is_entity(), |ui| {
                let Selection::Entity(selected) = state.current_selection else {
                    error!("Invalid selection");
                    return;
                };
                TableBuilder::new(ui)
                    .column(Column::auto().at_least(100.0))
                    .column(Column::remainder())
                    .body(|mut body| {
                        #[cfg(debug_assertions)]
                        {
                            body.row(18.0, |mut row| {
                                row.col(|ui| {
                                    ui.label("Entity");
                                });
                                row.col(|ui| {
                                    ui.label(format!("{selected:?}"));
                                });
                            });

                            // Display position
                            if let Ok(pos) = data_store.current_world().get::<&Pos2>(selected) {
                                body.row(18.0, |mut row| {
                                    row.col(|ui| {
                                        ui.label("Position");
                                    });
                                    row.col(|ui| {
                                        ui.label(format!("{pos:?}"));
                                    });
                                });
                            }

                            // Display size
                            if let Ok(size) = data_store.current_world().get::<&Size>(selected) {
                                body.row(18.0, |mut row| {
                                    row.col(|ui| {
                                        ui.label("Size");
                                    });
                                    row.col(|ui| {
                                        ui.label(format!("{size}"));
                                    });
                                });
                            }
                        }

                        if let Ok(name) = data_store.current_world().get::<&Name>(selected) {
                            body.row(18.0, |mut row| {
                                row.col(|ui| {
                                    ui.label("Name");
                                });
                                row.col(|ui| {
                                    ui.label(format!("{name}"));
                                });
                            });
                        }

                        if let Ok(typename) = data_store.current_world().get::<&TypeName>(selected)
                        {
                            body.row(18.0, |mut row| {
                                row.col(|ui| {
                                    ui.label("Type");
                                });
                                row.col(|ui| {
                                    ui.label(format!("{typename}"));
                                });
                            });
                        }

                        if let Ok(state) = data_store.current_world().get::<&State>(selected) {
                            body.row(18.0, |mut row| {
                                row.col(|ui| {
                                    ui.label("State");
                                });
                                row.col(|ui| {
                                    ui.label(format!("{state:?}"));
                                });
                            });
                        }
                    });

                if let Ok(properties) = data_store.current_world().get::<&Properties>(selected) {
                    ui.add_space(10.0);
                    egui::ScrollArea::horizontal()
                        .id_salt("properties_table_scroll_area")
                        .show(ui, |ui| {
                            TableBuilder::new(ui)
                                .column(Column::auto().at_least(100.0))
                                .column(Column::remainder())
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong("Property");
                                    });
                                    header.col(|ui| {
                                        ui.strong("Value");
                                    });
                                })
                                .body(|mut body| {
                                    for (key, value) in properties.0.iter() {
                                        body.row(18.0, |mut row| {
                                            row.col(|ui| {
                                                ui.label(key);
                                            });
                                            row.col(|ui| {
                                                ui.label(value);
                                            });
                                        });
                                    }
                                });
                        });
                }
            });
    }

    // Query all child nodes, then follow links to parents to compute the max depth
    let mut max_depth = 1;
    for (entity, _) in data_store
        .current_world()
        .query::<&Child>()
        .with::<&Node>()
        .iter()
    {
        let mut depth = 1;
        let mut current = entity;

        // Follow parent chain to compute depth
        while let Ok(child) = data_store.current_world().get::<&Child>(current) {
            depth += 1;
            current = child.parent;
        }

        max_depth = max_depth.max(depth);
    }
    // A pipeline with only one element will have a depth of 2
    let zoom = (max_depth as f32 - 1.0).max(1.0);

    egui::CentralPanel::default()
        .frame(egui::containers::Frame {
            fill: ctx.style().visuals.window_fill.gamma_multiply(0.95),
            inner_margin: egui::Margin::symmetric(5, 20),
            ..Default::default()
        })
        .show(ctx, |ui| {
            let scene = egui::Scene::new().zoom_range(0.0..=1.0);
            let mut inner_rect = Rect::NAN;

            let response = scene
                .show(ui, &mut state.scene_rect, |ui| {
                    let world = data_store.current_world();
                    let root_entities: Vec<_> = world
                        .query::<()>()
                        .with::<&Node>()
                        .without::<&Child>()
                        .iter()
                        .map(|(e, _)| e)
                        .collect();
                    let roots = sort_entities_by_name(world, root_entities.into_iter());

                    let node_margin = ui.style().node_margin().zoomed(zoom);
                    let layout = DAGLayout::new(node_margin.sum());

                    let parent_nodes = data_store
                        .current_world()
                        .query::<&Child>()
                        .with::<&Node>()
                        .iter()
                        .map(|(_, c)| c.parent)
                        .collect::<HashSet<_>>();

                    let topology_changed = state
                        .tree_change_tracker
                        .track(data_store.current_world_mut())
                        .any()
                        || state
                            .graph_change_tracker
                            .track(data_store.current_world_mut())
                            .any();

                    if topology_changed || reorganise {
                        // FIXME Only relayout the trees that have changed
                        // Should be easy to trace back to the roots with ancestors and a set
                        info!("Relayouting");
                        let mut buffer = hecs::CommandBuffer::new();
                        for &entity in parent_nodes.iter() {
                            if let Err(e) = layout.update_topology(
                                data_store.current_world(),
                                entity,
                                &mut buffer,
                            ) {
                                error!("Error during topology update: {e}");
                            }
                        }
                        buffer.run_on(data_store.current_world_mut());
                    }

                    let size_changed = state
                        .size_tracker
                        .track(data_store.current_world_mut())
                        .any();

                    if size_changed || reorganise {
                        let mut buffer = hecs::CommandBuffer::new();

                        for &entity in parent_nodes.iter() {
                            if let Err(e) = layout.update_positions(
                                data_store.current_world(),
                                entity,
                                &mut buffer,
                            ) {
                                error!("Error during node positioning: {e}");
                            }
                        }

                        // Insert a position for each root in the world, by using the size of the previous one
                        let margin = node_margin.sum();
                        let mut y = 0.0;
                        for root in roots.iter().cloned() {
                            if let Ok(size) = data_store.current_world().get::<&Size>(root) {
                                buffer.insert_one(root, Pos2::new(0.0, y));
                                y += size.0.y + 2.0 * margin.y;
                            }
                        }

                        buffer.run_on(data_store.current_world_mut());
                    }

                    // Draw the graphs and update the selected entity if needed
                    state.current_selection =
                        roots
                            .into_iter()
                            .fold(state.current_selection, |selected, root| {
                                show_node(
                                    ui,
                                    data_store.current_world_mut(),
                                    root,
                                    zoom,
                                    state.current_selection,
                                )
                                .unwrap_or(Selection::None)
                                .or(selected)
                            });

                    inner_rect = ui.min_rect();
                })
                .response;

            if response.clicked() {
                state.current_selection = Selection::None;
            }

            if response.double_clicked() || reorganise {
                state.scene_rect = inner_rect;
            }

            #[cfg(debug_assertions)]
            if state.show_debug_window {
                show_debug_window(ctx, data_store.current_world_mut());
            }
        });

    if reorganise {
        ctx.memory_mut(|mem| mem.reset_areas());
    }

    // FIXME: request a repaint if we are displaying a feed on a texture
}

#[cfg(debug_assertions)]
fn show_debug_window(ctx: &egui::Context, world: &mut hecs::World) {
    egui::Window::new("Debug")
        .default_width(200.0)
        .default_open(false)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::Vec2::new(10.0, -10.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new("World infos")
                    .default_open(true)
                    .show(ui, |ui| {
                        TableBuilder::new(ui)
                            .column(Column::auto())
                            .column(Column::remainder())
                            .body(|mut body| {
                                body.row(18.0, |mut row| {
                                    row.col(|ui| {
                                        ui.label("World size:");
                                    });
                                    row.col(|ui| {
                                        ui.label(world.len().to_string());
                                    });
                                });

                                body.row(18.0, |mut row| {
                                    row.col(|ui| {
                                        ui.label("Number of nodes:");
                                    });
                                    row.col(|ui| {
                                        ui.label(
                                            world
                                                .query::<()>()
                                                .with::<&Node>()
                                                .iter()
                                                .count()
                                                .to_string(),
                                        );
                                    });
                                });

                                body.row(18.0, |mut row| {
                                    row.col(|ui| {
                                        ui.label("Number of edges:");
                                    });
                                    row.col(|ui| {
                                        ui.label(
                                            world
                                                .query::<()>()
                                                .with::<&Edge>()
                                                .iter()
                                                .count()
                                                .to_string(),
                                        );
                                    });
                                });

                                body.row(18.0, |mut row| {
                                    row.col(|ui| {
                                        ui.label("Number of ports:");
                                    });
                                    row.col(|ui| {
                                        ui.label(
                                            world
                                                .query::<()>()
                                                .with::<&Port>()
                                                .iter()
                                                .count()
                                                .to_string(),
                                        );
                                    });
                                });
                            });
                    });

                ctx.settings_ui(ui);
            })
        });
}

fn show_node(
    ui: &mut egui::Ui,
    world: &mut hecs::World,
    entity: hecs::Entity,
    zoom: f32,
    current_selection: Selection,
) -> Result<Selection> {
    let style = ui.ctx().style();
    let mut proposed_selection = Selection::None;

    let is_root = world.get::<&Child>(entity).is_err();
    let selected = current_selection == Selection::Entity(entity);

    debug!("Showing node {entity:?} (is_root = {is_root}, selected = {selected})");

    let name = world.get::<&Name>(entity)?.deref().clone();
    let pos = *world.get::<&Pos2>(entity)?.deref();

    // FIXME It'd apparently be better to use Areas for this, and they'll handle the click better
    // as long as they are drawn back to front
    let mut inner_rect = ui.max_rect();
    inner_rect.min += pos.to_vec2();
    inner_rect.max = inner_rect.max.max(inner_rect.min);

    let builder = egui::UiBuilder::new()
        .max_rect(inner_rect)
        .layout(*ui.layout())
        .id_salt("node_child");
    let mut child_ui = ui.new_child(builder);

    let children = world
        .query::<&Child>()
        .with::<&Node>()
        .iter()
        .filter_map(|(e, c)| if c.parent == entity { Some(e) } else { None })
        .collect::<Vec<_>>();

    let mut collapsing_state = if children.is_empty() {
        None
    } else {
        Some(CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.id().with(("node", entity)),
            true,
        ))
    };

    // We need to do the click interaction before recurring in the children, otherwise the root
    // node is always getting the click. The drawback is that we use the previous position and size
    if let Ok(size) = world.get::<&Size>(entity) {
        let r = child_ui.interact(
            Rect::from_min_size(inner_rect.min, size.0),
            child_ui.id().with(("clicks", entity)),
            egui::Sense::click(),
        );
        if r.clicked() {
            proposed_selection = Selection::Entity(entity);
        }
        if r.double_clicked() {
            if let Some(ref mut state) = collapsing_state {
                state.toggle(&child_ui);
            }
        }
    }

    let mut prepared_frame = egui::Frame::default()
        .corner_radius(style.node_corner_radius())
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

        ui.horizontal(|ui| {
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

        if let Some(mut state) = collapsing_state {
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
                            show_node(ui, world, child, zoom, current_selection)
                                .inspect_err(|e| error!("{e:?}"))
                                .unwrap_or(Selection::None)
                                .or(selected)
                        });

                // Draw the links
                let mut shapes = Vec::new();
                for link in edges.iter() {
                    debug!("Drawing link {link:?}");
                    let from = match world.get::<&Pos2>(link.output_port) {
                        Ok(pos) => pos,
                        Err(_) => {
                            error!(
                                "Output port {port:?} doesn't have a position",
                                port = link.output_port
                            );
                            continue;
                        }
                    };
                    let to = match world.get::<&Pos2>(link.input_port) {
                        Ok(pos) => pos,
                        Err(_) => {
                            error!(
                                "Input port {port:?} doesn't have a position",
                                port = link.output_port
                            );
                            continue;
                        }
                    };

                    let selected = current_selection == Selection::Entity(link.output_port)
                        || current_selection == Selection::Entity(link.input_port);
                    let stroke = style.link_stroke(selected).zoomed(zoom);

                    shapes.push(epaint::Shape::CubicBezier(
                        epaint::CubicBezierShape::from_points_stroke(
                            compute_bezier_points(*from, *to, 0.5),
                            false,
                            egui::Color32::TRANSPARENT,
                            stroke,
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
    proposed_selection = show_ports(
        &mut child_ui,
        world,
        entity,
        Port::Output,
        r.rect,
        zoom,
        current_selection,
    )
    .or(proposed_selection);

    proposed_selection = show_ports(
        &mut child_ui,
        world,
        entity,
        Port::Input,
        r.rect,
        zoom,
        current_selection,
    )
    .or(proposed_selection);

    Ok(proposed_selection)
}

fn show_ports(
    ui: &mut egui::Ui,
    world: &mut hecs::World,
    parent: hecs::Entity,
    direction: Port,
    rect: Rect,
    zoom: f32,
    current_selection: Selection,
) -> Selection {
    let painter = ui.painter();
    let s = ui.style();

    let mut proposed_selection = Selection::None;

    let port_entities: Vec<_> = world
        .query::<(&Child, &Port)>()
        .iter()
        .filter_map(|(entity, (child, &port))| {
            if child.parent == parent && port == direction {
                Some(entity)
            } else {
                None
            }
        })
        .collect();
    let entities = sort_entities_by_name(world, port_entities.into_iter());

    let (top, bottom) = match direction {
        Port::Input => (rect.left_top(), rect.left_bottom()),
        Port::Output => (rect.right_top(), rect.right_bottom()),
    };

    for (index, entity) in entities.iter().cloned().enumerate() {
        let selected = current_selection.map_entity_or(false, |s| s == parent || s == entity);

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
            proposed_selection = Selection::Entity(entity);
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

fn show_node_tree(
    ui: &mut egui::Ui,
    world: &hecs::World,
    entity: hecs::Entity,
    selected_entities: &[hecs::Entity],
    depth: usize,
) -> Selection {
    let mut proposed_selection = Selection::None;

    let name = world
        .get::<&Name>(entity)
        .map(|n| n.0.clone())
        .unwrap_or_default();

    let child_entities: Vec<_> = world
        .query::<&Child>()
        .iter()
        .filter_map(|(e, c)| if c.parent == entity { Some(e) } else { None })
        .collect();
    let children = sort_entities_by_name(world, child_entities.into_iter());

    let selected = selected_entities.first() == Some(&entity);
    let open = selected_entities.len() > 1 && selected_entities[1..].contains(&entity);
    let text_color = if selected {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };

    let where_to_put_background = ui.painter().add(egui::Shape::Noop);

    let rect = if children.is_empty() {
        ui.horizontal(|ui| {
            if world.satisfies::<&Port>(entity).is_ok() {
                let s = ui.style();
                let circle_size = s.port_radius() * 2.0;
                ui.painter().circle(
                    ui.min_rect().left_center(),
                    s.port_radius(),
                    s.port_bg_fill(),
                    s.port_stroke(false),
                );
                ui.add_space(circle_size);
            }
            let label_response = ui.colored_label(text_color, name);
            if label_response.clicked() {
                proposed_selection = Selection::Entity(entity);
            }
        })
        .response
        .rect
    } else {
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.id().with(entity),
            depth < 3,
        );
        if open {
            state.set_open(true);
        }
        let response = state
            .show_header(ui, |ui| {
                let label_response = ui.colored_label(text_color, name);
                if label_response.clicked() {
                    proposed_selection = Selection::Entity(entity);
                }
            })
            .body(|ui| {
                proposed_selection =
                    children
                        .into_iter()
                        .fold(proposed_selection, |selection, child| {
                            show_node_tree(ui, world, child, selected_entities, depth + 1)
                                .or(selection)
                        });
            });

        response.1.response.rect
    };

    if selected {
        let rect = rect.with_min_x(0.0).with_max_x(ui.clip_rect().max.x);
        ui.painter().set(
            where_to_put_background,
            egui::Shape::Rect(epaint::RectShape::filled(
                rect,
                0.0,
                ui.visuals().selection.bg_fill,
            )),
        );
    }

    proposed_selection
}
