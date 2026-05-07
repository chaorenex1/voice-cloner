use std::{path::PathBuf, sync::RwLock};

use serde::{de::DeserializeOwned, Serialize};

use crate::app::error::{AppError, AppResult};

#[derive(Debug)]
pub struct JsonStore<T> {
    path: PathBuf,
    value: RwLock<T>,
}

impl<T> JsonStore<T>
where
    T: Clone + DeserializeOwned + Serialize,
{
    pub fn new(path: impl Into<PathBuf>, default: T) -> Self {
        Self {
            path: path.into(),
            value: RwLock::new(default),
        }
    }

    pub fn load_or_create(&self) -> AppResult<T> {
        if self.path.exists() {
            let content =
                std::fs::read_to_string(&self.path).map_err(|source| AppError::io("reading json store", source))?;
            let loaded =
                serde_json::from_str::<T>(&content).map_err(|source| AppError::json("parsing json store", source))?;
            *self.value.write().expect("json store lock poisoned") = loaded.clone();
            Ok(loaded)
        } else {
            self.save_current()
        }
    }

    pub fn get(&self) -> T {
        self.value.read().expect("json store lock poisoned").clone()
    }

    pub fn replace(&self, next: T) -> AppResult<T> {
        *self.value.write().expect("json store lock poisoned") = next.clone();
        self.save_current()?;
        Ok(next)
    }

    fn save_current(&self) -> AppResult<T> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| AppError::io("creating json store directory", source))?;
        }

        let value = self.get();
        let content =
            serde_json::to_string_pretty(&value).map_err(|source| AppError::json("serializing json store", source))?;
        let temp_path = self.path.with_extension("json.tmp");
        std::fs::write(&temp_path, content).map_err(|source| AppError::io("writing json store temp file", source))?;
        std::fs::rename(&temp_path, &self.path).map_err(|source| AppError::io("committing json store", source))?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde::{Deserialize, Serialize};

    use super::JsonStore;

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
    struct TestDoc {
        name: String,
    }

    #[test]
    fn json_store_creates_and_reloads_document() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("voice-cloner-store-{unique}/doc.json"));
        let store = JsonStore::new(&path, TestDoc { name: "default".into() });

        assert_eq!(store.load_or_create().unwrap().name, "default");
        store.replace(TestDoc { name: "updated".into() }).unwrap();

        let reloaded = JsonStore::new(&path, TestDoc { name: "ignored".into() });
        assert_eq!(reloaded.load_or_create().unwrap().name, "updated");
    }
}
