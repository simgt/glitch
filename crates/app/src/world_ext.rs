use glitch_data::components::*;

pub trait WorldExt {
    // FIXME remove find_element and use find_entity instead
    fn update_element(&mut self, element: Element) -> hecs::Entity;
    fn find_entity(&self, id: RemoteId) -> Option<hecs::Entity>;
    fn spawn_pad(&mut self, pad: Pad) -> hecs::Entity;
}

impl WorldExt for hecs::World {
    /// Update an element if it already exists, or spawn a new one if it doesn't
    fn update_element(&mut self, element: Element) -> hecs::Entity {
        if let Some(entity) = self.find_entity(element.id) {
            self.insert(entity, element).unwrap();
            entity
        } else {
            // FIXME instead of putting a dummy size here, we should query the
            // nodes without a size and draw them on an invisible ui to bootstrap
            // the layout
            let mut builder = hecs::EntityBuilder::new();
            builder.add_bundle(element);
            builder.add(Size(egui::Vec2::ZERO));
            self.spawn(builder.build())
        }
    }

    fn find_entity(&self, id: RemoteId) -> Option<hecs::Entity> {
        self.query::<&RemoteId>()
            .iter()
            .find_map(|(entity, &object_id)| if object_id == id { Some(entity) } else { None })
    }

    fn spawn_pad(&mut self, pad: Pad) -> hecs::Entity {
        self.spawn(pad)
    }
}
