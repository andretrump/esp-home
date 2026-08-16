use anyhow::Result;
use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use std::collections::HashMap;

pub struct NvsManager {
    nvs: EspNvs<NvsDefault>,
    namespace: String,
    properties: HashMap<String, usize>,
}

impl NvsManager {
    pub fn new(
        partition: EspNvsPartition<NvsDefault>,
        namespace: String,
        properties: HashMap<String, usize>,
    ) -> Result<Self> {
        let nvs = EspNvs::new(partition, namespace.as_str(), true)?;
        let nvs_manager = Self {
            nvs,
            namespace,
            properties,
        };
        Ok(nvs_manager)
    }

    pub fn store_properties(&self, properties: HashMap<String, String>) -> Result<()> {
        for property in properties {
            self.store_property(property.0.as_str(), property.1.as_str())?;
        }
        Ok(())
    }

    pub fn store_property(&self, key: &str, value: &str) -> Result<()> {
        if !self.properties.contains_key(key) {
            anyhow::bail!("Unknown property key: {}", key);
        }
        self.nvs.set_str(key, value)?;
        Ok(())
    }

    pub fn load_all_properties(&self) -> Result<HashMap<String, String>> {
        self.properties
            .keys()
            .map(|key| {
                let value = self
                    .load_property(key)?
                    .ok_or_else(|| anyhow::anyhow!("Property '{}' not set", key))?;
                Ok((key.clone(), value))
            })
            .collect()
    }

    pub fn load_property(&self, key: &str) -> Result<Option<String>> {
        let size = self
            .properties
            .get(key)
            .ok_or_else(|| anyhow::anyhow!("Unknown property key: {}", key))?;
        let mut buffer = vec![0u8; size + 1];
        Ok(self.nvs.get_str(key, &mut buffer)?.map(|s| s.to_string()))
    }

    pub fn namespace(&self) -> &String {
        &self.namespace
    }
}
