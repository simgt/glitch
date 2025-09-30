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

#[derive(Serialize, Deserialize)]
pub struct DataStore {
    #[serde(skip)]
    snapshot: Snapshot,
    command_history: BTreeMap<Timestamp, Vec<Command>>,
}

impl Default for DataStore {
    fn default() -> Self {
        Self {
            snapshot: Snapshot::default(),
            command_history: BTreeMap::new(),
        }
    }
}

pub enum ViewMode {
    Latest,
    Specific(Timestamp),
}

impl DataStore {
    pub fn record_command(&mut self, mut command: Command) {
        command.translate_entities(&mut self.snapshot.remote_entities, &mut self.snapshot.world);

        let timestamp = if self.command_history.is_empty() {
            0
        } else {
            self.command_history.keys().next_back().unwrap() + 1
        };

        self.command_history
            .entry(timestamp)
            .or_insert_with(Vec::new)
            .push(command.clone());

        // TODO if self.current_view_mode == ViewMode::Latest {
        command.run_on(&mut self.snapshot.world);
    }

    pub fn current_world(&self) -> &hecs::World {
        &self.snapshot.world
    }

    pub fn current_world_mut(&mut self) -> &mut hecs::World {
        &mut self.snapshot.world
    }

    pub fn set_view(&mut self, view_mode: ViewMode) {
        // TODO set the current world based on the requested timestamp (latest snapshot + all commands up to)
        // TODO add self.current_view_mode
        // TODO rename self.world to self.current_world
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
