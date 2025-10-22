use crate::{comps::*, DataStore};
use anyhow::{Context, Result};
use hecs::serialize::row::*;
use serde::Serialize;
use std::{collections::HashMap, io::Read, path::Path};
use tracing::info;

pub fn load_world(path: impl AsRef<Path>) -> Result<hecs::World> {
    let path = path.as_ref();
    info!("Loading world from {path:?}");
    let mut bytes = Vec::new();
    let mut deserializer = std::fs::File::open(path)
        .context("Failed to open file")
        .and_then(|mut file| file.read_to_end(&mut bytes).context("Failed to read file"))
        .and_then(|_| ron::de::Deserializer::from_bytes(&bytes).context("Failed to deserialize"))
        .unwrap();
    hecs::serialize::row::deserialize(&mut SerContext, &mut deserializer)
        .context("Failed to deserialize world")
}

pub fn save_datastore(datastore: &DataStore, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    info!("Saving datastore to {path:?}");

    let mut file = std::fs::File::create(path).context("Failed to create file")?;

    // Create a serializer with pretty printing
    let mut serializer = ron::Serializer::with_options(
        &mut file,
        Some(ron::ser::PrettyConfig::default()),
        Default::default(),
    )
    .context("Failed to create serializer")?;

    // First serialize the world using hecs serialization
    let mut world_bytes = Vec::new();
    {
        let mut world_serializer = ron::Serializer::with_options(
            &mut world_bytes,
            Some(ron::ser::PrettyConfig::default()),
            Default::default(),
        )
        .context("Failed to create world serializer")?;

        hecs::serialize::row::serialize(
            &datastore.rolling_snapshot.world,
            &mut SerContext,
            &mut world_serializer,
        )
        .context("Failed to serialize world")?;
    }

    // Create a container structure for both world and datastore data
    #[derive(serde::Serialize)]
    struct DataStoreContainer {
        world_data: String,
        command_history: std::collections::BTreeMap<crate::Timestamp, Vec<crate::Command>>,
    }

    let container = DataStoreContainer {
        world_data: String::from_utf8(world_bytes)
            .context("Failed to convert world data to string")?,
        command_history: datastore.command_history.clone(),
    };

    container
        .serialize(&mut serializer)
        .context("Failed to serialize datastore")
}

pub fn load_datastore(path: impl AsRef<Path>) -> Result<DataStore> {
    let path = path.as_ref();
    info!("Loading datastore from {path:?}");

    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .context("Failed to open file")?
        .read_to_end(&mut bytes)
        .context("Failed to read file")?;

    #[derive(serde::Deserialize)]
    struct DataStoreContainer {
        world_data: String,
        command_history: std::collections::BTreeMap<crate::Timestamp, Vec<crate::Command>>,
    }

    let container: DataStoreContainer =
        ron::de::from_bytes(&bytes).context("Failed to deserialize datastore")?;

    // Deserialize the world from the embedded world data
    let world_bytes = container.world_data.as_bytes();
    let mut world_deserializer = ron::de::Deserializer::from_bytes(world_bytes)
        .context("Failed to create world deserializer")?;
    let world = hecs::serialize::row::deserialize(&mut SerContext, &mut world_deserializer)
        .context("Failed to deserialize world")?;

    Ok(DataStore {
        rolling_snapshot: crate::Snapshot {
            world,
            remote_entities: HashMap::new(),
        },
        fixed_snapshot: crate::Snapshot::new(),
        current_view_mode: crate::ViewMode::Rolling,
        command_history: container.command_history,
    })
}

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
        // Size, Position and Layers are not serialized as they are rebuilt
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Node, SpawnOrInsert};
    use tempfile::NamedTempFile;

    #[test]
    fn test_datastore_serialization() {
        // Create a test DataStore with some data
        let mut datastore = DataStore::default();

        // Add some entities to the world
        let entity1 = datastore.rolling_snapshot.world.spawn((Node {},));
        let entity2 = datastore.rolling_snapshot.world.spawn((Node {},));

        // Add some commands to the history
        let cmd1 = Command::SpawnOrInsert(entity1, SpawnOrInsert::Node(Node {}));
        let cmd2 = Command::SpawnOrInsert(entity2, SpawnOrInsert::Node(Node {}));

        datastore.record_command(cmd1);
        datastore.record_command(cmd2);

        // Save to a temporary file
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        save_datastore(&datastore, temp_file.path()).expect("Failed to save datastore");

        // Load from the temporary file
        let loaded_datastore = load_datastore(temp_file.path()).expect("Failed to load datastore");

        // Verify the data
        assert_eq!(
            loaded_datastore.command_history.len(),
            datastore.command_history.len()
        );
        assert_eq!(loaded_datastore.history_len(), datastore.history_len());

        // Verify the world has the same number of entities
        assert_eq!(
            loaded_datastore.rolling_snapshot.world.len(),
            datastore.rolling_snapshot.world.len()
        );
    }

    #[test]
    fn test_empty_datastore_serialization() {
        let datastore = DataStore::default();

        // Save to a temporary file
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        save_datastore(&datastore, temp_file.path()).expect("Failed to save empty datastore");

        // Load from the temporary file
        let loaded_datastore =
            load_datastore(temp_file.path()).expect("Failed to load empty datastore");

        // Verify the data
        assert_eq!(loaded_datastore.command_history.len(), 0);
        assert_eq!(loaded_datastore.rolling_snapshot.world.len(), 0);
    }
}
