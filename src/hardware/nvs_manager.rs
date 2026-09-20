use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use std::{collections::HashMap, marker::PhantomData};

#[macro_export]
macro_rules! nvs_keys {
    ($name:ident { $($variant:ident => $key:literal),* $(,)? }) => {
        #[derive(Copy, Clone, Debug)]
        pub enum $name {
            $($variant,)*
        }

        impl $crate::hardware::NvsKey for $name {
            fn key(&self) -> &'static str {
                match self {
                    $($name::$variant => $key,)*
                }
            }
            fn max_size(&self) -> usize { 512 }
            fn from_str(key: &str) -> Option<Self> {
                match key {
                    $($key => Some(Self::$variant),)*
                    _ => None,
                }
            }
            fn all_variants() -> &'static [Self] {
                &[$($name::$variant,)*]
            }
        }
    };
}

pub trait NvsKey {
    fn key(&self) -> &'static str;
    fn max_size(&self) -> usize;
    fn from_str(key: &str) -> Option<Self>
    where
        Self: Sized;
    fn all_variants() -> &'static [Self]
    where
        Self: Sized;
}

#[derive(Debug, thiserror::Error)]
#[error("Property '{0}' is not set in NVS")]
pub struct PropertyNotSet(pub &'static str);

pub struct LoadedConfig(HashMap<&'static str, String>);

impl LoadedConfig {
    pub fn get<K: NvsKey>(&self, key: K) -> &str {
        self.0[key.key()].as_str()
    }
}

pub struct NvsManager<K: NvsKey> {
    nvs: EspNvs<NvsDefault>,
    namespace: String,
    _key_type: std::marker::PhantomData<K>,
}

impl<K: NvsKey + Copy + 'static> NvsManager<K> {
    pub fn new(partition: EspNvsPartition<NvsDefault>, namespace: String) -> Self {
        let nvs =
            EspNvs::new(partition, namespace.as_str(), true).expect("Failed to initialize NVS.");
        Self {
            nvs,
            namespace,
            _key_type: PhantomData,
        }
    }

    pub fn all_properties_set(&self) -> bool {
        K::all_variants()
            .iter()
            .all(|key| self.load_property(*key).is_ok())
    }

    pub fn store_properties(&self, properties: HashMap<String, String>) {
        for (key_str, value) in properties {
            if let Some(key) = K::from_str(&key_str) {
                self.store_property(key, &value);
            }
        }
    }

    pub fn load_all_properties(&self) -> Result<LoadedConfig, PropertyNotSet> {
        K::all_variants()
            .iter()
            .map(|key| {
                let value = self.load_property(*key)?;
                Ok((key.key(), value))
            })
            .collect::<Result<HashMap<&'static str, String>, PropertyNotSet>>()
            .map(LoadedConfig)
    }

    pub fn load_property(&self, key: K) -> Result<String, PropertyNotSet> {
        let mut buffer = vec![0u8; key.max_size() + 1];
        self.nvs
            .get_str(key.key(), &mut buffer)
            .expect("Failed to read from NVS")
            .map(|s| s.to_string())
            .ok_or_else(|| PropertyNotSet(key.key()))
    }

    pub fn store_property(&self, key: K, value: &str) {
        self.nvs
            .set_str(key.key(), value)
            .expect("Failed to write to NVS");
    }

    pub fn namespace(&self) -> &String {
        &self.namespace
    }
}
