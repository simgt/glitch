pub mod client;
pub mod comps;
pub mod ser;

pub use client::RecordingStream;
pub use comps::*;

use enum_dispatch::enum_dispatch;
use hecs::Entity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DEFAULT_PORT: u16 = 9870;

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
