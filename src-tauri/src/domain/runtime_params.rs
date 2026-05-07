use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeParams {
    #[serde(default)]
    pub values: BTreeMap<String, serde_json::Value>,
}

impl Default for RuntimeParams {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
}
