pub mod client;
pub mod comps;
pub mod ser;

pub use client::RecordingStream;
pub use comps::*;

use enum_dispatch::enum_dispatch;
use hecs::Entity;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub const DEFAULT_PORT: u16 = 9870;

pub type Timestamp = u64;

#[derive(Default)]
pub struct Snapshot {
    pub world: hecs::World,
    pub remote_entities: HashMap<Entity, Entity>,
}

impl Snapshot {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Serialize, Deserialize)]
pub struct DataStore {
    #[serde(skip)]
    pub rolling_snapshot: Snapshot,
    #[serde(skip)]
    pub fixed_snapshot: Snapshot,
    #[serde(skip)]
    pub current_view_mode: ViewMode,
    pub command_history: BTreeMap<Timestamp, Vec<Command>>,
}

impl Default for DataStore {
    fn default() -> Self {
        Self {
            rolling_snapshot: Snapshot::default(),
            fixed_snapshot: Snapshot::default(),
            current_view_mode: ViewMode::Rolling,
            command_history: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Default)]
pub enum ViewMode {
    #[default]
    Rolling,
    Specific(Timestamp),
}

impl DataStore {
    pub fn record_command(&mut self, mut command: Command) {
        command.translate_entities(
            &mut self.rolling_snapshot.remote_entities,
            &mut self.rolling_snapshot.world,
        );

        let timestamp = if self.command_history.is_empty() {
            0
        } else {
            self.command_history.keys().next_back().unwrap() + 1
        };

        self.command_history
            .entry(timestamp)
            .or_insert_with(Vec::new)
            .push(command.clone());

        // Always update rolling snapshot
        command.run_on(&mut self.rolling_snapshot.world);
    }

    pub fn current_timeline_position(&self) -> Option<Timestamp> {
        match self.current_view_mode {
            ViewMode::Rolling => self.command_history.keys().next_back().copied(),
            ViewMode::Specific(timestamp) => Some(timestamp),
        }
    }

    pub fn can_step_backward(&self) -> bool {
        match self.current_view_mode {
            ViewMode::Rolling => !self.command_history.is_empty(),
            ViewMode::Specific(timestamp) => {
                self.command_history.range(..timestamp).next().is_some()
            }
        }
    }

    pub fn can_step_forward(&self) -> bool {
        match self.current_view_mode {
            ViewMode::Rolling => false,
            ViewMode::Specific(timestamp) => self
                .command_history
                .range((timestamp + 1)..)
                .next()
                .is_some(),
        }
    }

    pub fn step_backward(&mut self) {
        match self.current_view_mode {
            ViewMode::Rolling => {
                // Switch to the latest timestamp
                if let Some(&latest) = self.command_history.keys().next_back() {
                    self.set_view(ViewMode::Specific(latest));
                }
            }
            ViewMode::Specific(timestamp) => {
                // Find the previous timestamp
                if let Some((&prev_timestamp, _)) =
                    self.command_history.range(..timestamp).next_back()
                {
                    self.set_view(ViewMode::Specific(prev_timestamp));
                }
            }
        }
    }

    pub fn step_forward(&mut self) {
        if let ViewMode::Specific(timestamp) = self.current_view_mode {
            // Find the next timestamp
            if let Some((&next_timestamp, _)) = self.command_history.range((timestamp + 1)..).next()
            {
                self.set_view(ViewMode::Specific(next_timestamp));
            }
        }
    }

    pub fn toggle_rolling_mode(&mut self) {
        match self.current_view_mode {
            ViewMode::Rolling => {
                // Already in rolling mode, do nothing or maybe switch to latest specific timestamp
            }
            ViewMode::Specific(_) => {
                self.set_view(ViewMode::Rolling);
            }
        }
    }

    pub fn current_world(&self) -> &hecs::World {
        match self.current_view_mode {
            ViewMode::Rolling => &self.rolling_snapshot.world,
            ViewMode::Specific(_) => &self.fixed_snapshot.world,
        }
    }

    pub fn current_world_mut(&mut self) -> &mut hecs::World {
        match self.current_view_mode {
            ViewMode::Rolling => &mut self.rolling_snapshot.world,
            ViewMode::Specific(_) => &mut self.fixed_snapshot.world,
        }
    }

    pub fn set_view(&mut self, view_mode: ViewMode) {
        self.current_view_mode = view_mode.clone();

        match view_mode {
            ViewMode::Rolling => {
                // Nothing to do - just switch to rolling view
            }
            ViewMode::Specific(timestamp) => {
                // Rebuild fixed snapshot up to the specified timestamp
                self.fixed_snapshot = Snapshot::new();

                // Apply all commands up to and including the specified timestamp
                for (_, commands) in self.command_history.range(..=timestamp) {
                    for mut command in commands.iter().cloned() {
                        command.translate_entities(
                            &mut self.fixed_snapshot.remote_entities,
                            &mut self.fixed_snapshot.world,
                        );
                        command.run_on(&mut self.fixed_snapshot.world);
                    }
                }
            }
        }
    }

    pub fn commands_in<R>(&self, range: R) -> Vec<Command>
    where
        R: std::ops::RangeBounds<Timestamp>,
    {
        self.command_history
            .range(range)
            .flat_map(|(_, commands)| commands.iter().cloned())
            .collect()
    }

    pub fn timestamp_bounds(&self) -> Option<std::ops::RangeInclusive<Timestamp>> {
        if self.command_history.is_empty() {
            None
        } else {
            let min = *self.command_history.keys().next().unwrap();
            let max = *self.command_history.keys().next_back().unwrap();
            Some(min..=max)
        }
    }

    pub fn history_len(&self) -> usize {
        self.command_history
            .values()
            .map(|commands| commands.len())
            .sum()
    }
}

// FIXME the extra enums are fairly verbose and brainfuck, find a cleaner
// way of doing all this

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    SpawnOrInsert(Entity, SpawnOrInsert),
    Remove(Entity, Remove),
    Despawn(Entity),
}

impl Command {
    pub fn translate_entities(
        &mut self,
        mapping: &mut HashMap<Entity, Entity>,
        world: &mut hecs::World,
    ) {
        let entity = match self {
            Command::SpawnOrInsert(entity, component) => {
                component.translate_entities(mapping, world);
                entity
            }
            Command::Remove(entity, _) => entity,
            Command::Despawn(entity) => entity,
        };

        *entity = *mapping
            .entry(*entity)
            .or_insert_with(|| world.reserve_entity());
    }

    pub fn run_on(self, world: &mut hecs::World) {
        match self {
            Command::SpawnOrInsert(entity, component) => {
                component.append_to(world, entity);
            }
            Command::Remove(entity, component) => component.append_to(world, entity),
            Command::Despawn(entity) => {
                world.despawn(entity).unwrap();
            }
        }
    }
}

#[enum_dispatch]
pub trait AppendTo: Sized + hecs::Component {
    fn translate_entities(
        &mut self,
        _mapping: &mut HashMap<Entity, Entity>,
        _world: &mut hecs::World,
    ) {
    }

    fn append_to(self, world: &mut hecs::World, entity: Entity) {
        world.insert_one(entity, self).unwrap();
    }
}

impl AppendTo for Child {
    fn translate_entities(
        &mut self,
        mapping: &mut HashMap<Entity, Entity>,
        world: &mut hecs::World,
    ) {
        self.parent = *mapping
            .entry(self.parent)
            .or_insert_with(|| world.reserve_entity())
    }
}

impl AppendTo for Edge {
    fn translate_entities(
        &mut self,
        mapping: &mut HashMap<Entity, Entity>,
        world: &mut hecs::World,
    ) {
        self.output_port = *mapping
            .entry(self.output_port)
            .or_insert_with(|| world.reserve_entity());

        self.input_port = *mapping
            .entry(self.input_port)
            .or_insert_with(|| world.reserve_entity());
    }
}

impl AppendTo for Name {}
impl AppendTo for Node {}
impl AppendTo for Port {}
impl AppendTo for Properties {}
impl AppendTo for State {}
impl AppendTo for TypeName {}

#[enum_dispatch(AppendTo)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpawnOrInsert {
    Node(Node),
    Edge(Edge),
    State(State),
    Name(Name),
    TypeName(TypeName),
    Properties(Properties),
    Port(Port),
    Child(Child),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Remove {
    Node,
    Edge,
    State,
    Name,
    TypeName,
    Properties,
    Port,
    Child,
}

impl AppendTo for Remove {
    fn append_to(self, world: &mut hecs::World, entity: Entity) {
        match self {
            Remove::Node => {
                world.remove_one::<Node>(entity).unwrap();
            }
            Remove::Edge => {
                world.remove_one::<Edge>(entity).unwrap();
            }
            Remove::State => {
                world.remove_one::<State>(entity).unwrap();
            }
            Remove::Name => {
                world.remove_one::<Name>(entity).unwrap();
            }
            Remove::TypeName => {
                world.remove_one::<TypeName>(entity).unwrap();
            }
            Remove::Properties => {
                world.remove_one::<Properties>(entity).unwrap();
            }
            Remove::Port => {
                world.remove_one::<Port>(entity).unwrap();
            }
            Remove::Child => {
                world.remove_one::<Child>(entity).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_snapshot() {
        let mut datastore = DataStore::default();

        // Create valid entity IDs using a temporary world
        let mut temp_world = hecs::World::new();
        let entity1 = temp_world.spawn(());
        let entity2 = temp_world.spawn(());

        let cmd1 = Command::SpawnOrInsert(entity1, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd1);

        let cmd2 = Command::SpawnOrInsert(entity2, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd2);

        // Should start in rolling mode
        assert!(matches!(datastore.current_view_mode, ViewMode::Rolling));
        assert_eq!(datastore.rolling_snapshot.world.len(), 2);
    }

    #[test]
    fn test_specific_timestamp_view() {
        let mut datastore = DataStore::default();

        // Create valid entity IDs using a temporary world
        let mut temp_world = hecs::World::new();
        let entity1 = temp_world.spawn(());
        let entity2 = temp_world.spawn(());
        let entity3 = temp_world.spawn(());

        let cmd1 = Command::SpawnOrInsert(entity1, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd1);

        let cmd2 = Command::SpawnOrInsert(entity2, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd2);

        let cmd3 = Command::SpawnOrInsert(entity3, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd3);

        // Rolling snapshot should have 3 entities
        assert_eq!(datastore.rolling_snapshot.world.len(), 3);

        // Set view to timestamp 0 (first command only)
        datastore.set_view(ViewMode::Specific(0));
        assert!(matches!(datastore.current_view_mode, ViewMode::Specific(0)));
        assert_eq!(datastore.current_world().len(), 1);

        // Set view to timestamp 1 (first two commands)
        datastore.set_view(ViewMode::Specific(1));
        assert!(matches!(datastore.current_view_mode, ViewMode::Specific(1)));
        assert_eq!(datastore.current_world().len(), 2);

        // Switch back to rolling view
        datastore.set_view(ViewMode::Rolling);
        assert!(matches!(datastore.current_view_mode, ViewMode::Rolling));
        assert_eq!(datastore.current_world().len(), 3);
    }

    #[test]
    fn test_view_mode_switching() {
        let mut datastore = DataStore::default();

        // Create valid entity IDs using a temporary world
        let mut temp_world = hecs::World::new();
        let entity1 = temp_world.spawn(());
        let entity2 = temp_world.spawn(());

        let cmd1 = Command::SpawnOrInsert(entity1, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd1);

        // Default should be rolling
        assert!(matches!(datastore.current_view_mode, ViewMode::Rolling));
        let rolling_world = datastore.current_world();
        assert_eq!(rolling_world.len(), 1);

        // Switch to specific view
        datastore.set_view(ViewMode::Specific(0));
        let fixed_world = datastore.current_world();
        assert_eq!(fixed_world.len(), 1);

        // Add another command (should only affect rolling)
        let cmd2 = Command::SpawnOrInsert(entity2, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd2);

        // Fixed view should still have 1 entity
        assert_eq!(datastore.current_world().len(), 1);

        // Rolling view should have 2 entities
        datastore.set_view(ViewMode::Rolling);
        assert_eq!(datastore.rolling_snapshot.world.len(), 2);
    }

    #[test]
    fn test_timeline_navigation() {
        let mut datastore = DataStore::default();

        // Create valid entity IDs using a temporary world
        let mut temp_world = hecs::World::new();
        let entity1 = temp_world.spawn(());
        let entity2 = temp_world.spawn(());
        let entity3 = temp_world.spawn(());

        // Add three commands
        let cmd1 = Command::SpawnOrInsert(entity1, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd1);

        let cmd2 = Command::SpawnOrInsert(entity2, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd2);

        let cmd3 = Command::SpawnOrInsert(entity3, SpawnOrInsert::Node(Node {}));
        datastore.record_command(cmd3);

        // Should start in rolling mode at latest timestamp
        assert!(matches!(datastore.current_view_mode, ViewMode::Rolling));
        assert_eq!(datastore.current_timeline_position(), Some(2));

        // Can step backward from rolling, cannot step forward
        assert!(datastore.can_step_backward());
        assert!(!datastore.can_step_forward());

        // Step backward should move to latest timestamp in specific mode
        datastore.step_backward();
        assert!(matches!(datastore.current_view_mode, ViewMode::Specific(2)));
        assert_eq!(datastore.current_timeline_position(), Some(2));

        // Now we can step both ways
        assert!(datastore.can_step_backward());
        assert!(!datastore.can_step_forward()); // At latest, can't go forward

        // Step backward to timestamp 1
        datastore.step_backward();
        assert!(matches!(datastore.current_view_mode, ViewMode::Specific(1)));
        assert_eq!(datastore.current_timeline_position(), Some(1));

        // Now we can step forward
        assert!(datastore.can_step_backward());
        assert!(datastore.can_step_forward());

        // Step backward to timestamp 0
        datastore.step_backward();
        assert!(matches!(datastore.current_view_mode, ViewMode::Specific(0)));
        assert_eq!(datastore.current_timeline_position(), Some(0));

        // At earliest timestamp, can't step back further
        assert!(!datastore.can_step_backward());
        assert!(datastore.can_step_forward());

        // Step forward to timestamp 1
        datastore.step_forward();
        assert!(matches!(datastore.current_view_mode, ViewMode::Specific(1)));

        // Step forward to timestamp 2
        datastore.step_forward();
        assert!(matches!(datastore.current_view_mode, ViewMode::Specific(2)));

        // Toggle back to rolling mode
        datastore.toggle_rolling_mode();
        assert!(matches!(datastore.current_view_mode, ViewMode::Rolling));
        assert_eq!(datastore.current_timeline_position(), Some(2));
    }

    #[test]
    fn test_empty_datastore_timeline() {
        let mut datastore = DataStore::default();

        // Empty datastore should have no timeline position
        assert_eq!(datastore.current_timeline_position(), None);
        assert!(!datastore.can_step_backward());
        assert!(!datastore.can_step_forward());

        // Step operations should not crash on empty datastore
        datastore.step_backward(); // Should do nothing
        datastore.step_forward(); // Should do nothing
        datastore.toggle_rolling_mode(); // Should do nothing

        // Should still be in rolling mode
        assert!(matches!(datastore.current_view_mode, ViewMode::Rolling));
    }
}
