use crate::comps::*;
use hecs::serialize::row::*;

pub struct SerContext;

#[derive(serde::Serialize, serde::Deserialize)]
enum ComponentId {
    Node,
    State,
    Name,
    TypeName,
    Properties,
    Port,
    Edge,
    Child,
}

impl SerializeContext for SerContext {
    fn serialize_entity<S>(
        &mut self,
        entity: hecs::EntityRef<'_>,
        mut map: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::SerializeMap,
    {
        // Size, Position and TopologyLayout are not serialized as they are rebuilt
        // by the application
        try_serialize::<Node, _, _>(&entity, &ComponentId::Node, &mut map)?;
        try_serialize::<State, _, _>(&entity, &ComponentId::State, &mut map)?;
        try_serialize::<Name, _, _>(&entity, &ComponentId::Name, &mut map)?;
        try_serialize::<TypeName, _, _>(&entity, &ComponentId::TypeName, &mut map)?;
        try_serialize::<Properties, _, _>(&entity, &ComponentId::Properties, &mut map)?;
        try_serialize::<Port, _, _>(&entity, &ComponentId::Port, &mut map)?;
        try_serialize::<Edge, _, _>(&entity, &ComponentId::Edge, &mut map)?;
        try_serialize::<Child, _, _>(&entity, &ComponentId::Child, &mut map)?;
        map.end()
    }
}

impl DeserializeContext for SerContext {
    fn deserialize_entity<'de, M>(
        &mut self,
        mut map: M,
        entity: &mut hecs::EntityBuilder,
    ) -> Result<(), M::Error>
    where
        M: serde::de::MapAccess<'de>,
    {
        while let Some(key) = map.next_key()? {
            match key {
                ComponentId::Node => {
                    entity.add::<Node>(map.next_value()?);
                }
                ComponentId::State => {
                    entity.add::<State>(map.next_value()?);
                }
                ComponentId::Name => {
                    entity.add::<Name>(map.next_value()?);
                }
                ComponentId::TypeName => {
                    entity.add::<TypeName>(map.next_value()?);
                }
                ComponentId::Properties => {
                    entity.add::<Properties>(map.next_value()?);
                }
                ComponentId::Port => {
                    entity.add::<Port>(map.next_value()?);
                }
                ComponentId::Edge => {
                    entity.add::<Edge>(map.next_value()?);
                }
                ComponentId::Child => {
                    entity.add::<Child>(map.next_value()?);
                }
            }
        }
        Ok(())
    }
}
