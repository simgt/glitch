use crate::components::*;
use hecs::serialize::row::*;

pub struct SerContext;

#[derive(serde::Serialize, serde::Deserialize)]
enum ComponentId {
    Element,
    Pad,
    Child,
    Link,
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
        try_serialize::<Element, _, _>(&entity, &ComponentId::Element, &mut map)?;
        try_serialize::<Pad, _, _>(&entity, &ComponentId::Pad, &mut map)?;
        try_serialize::<Child, _, _>(&entity, &ComponentId::Child, &mut map)?;
        try_serialize::<Edge, _, _>(&entity, &ComponentId::Link, &mut map)?;
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
                ComponentId::Element => {
                    entity.add::<Element>(map.next_value()?);
                }
                ComponentId::Pad => {
                    entity.add::<Pad>(map.next_value()?);
                }
                ComponentId::Child => {
                    entity.add::<Child>(map.next_value()?);
                }
                ComponentId::Link => {
                    entity.add::<Edge>(map.next_value()?);
                }
            }
        }
        Ok(())
    }
}
