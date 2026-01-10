use serde::de::DeserializeOwned;
use crate::discord::modules::*;
pub trait ModuleConfig: Serialize + DeserializeOwned + Default {
    /// The key in the JSON "modules" map (e.g., "JoinAnnouncer")
    fn key() -> &'static str;
}

use std::{collections::HashMap, hash::Hash};
use serde::{Deserialize, Serialize};

pub trait ConfigProvider {
    fn key(&self) -> &'static str;
    fn default_value(&self) -> serde_json::Value;
}

// A simple generic struct to hold the provider info
struct Provider<T: ModuleConfig>(std::marker::PhantomData<T>);

impl<T: ModuleConfig> ConfigProvider for Provider<T> {
    fn key(&self) -> &'static str { T::key() }
    fn default_value(&self) -> serde_json::Value {
        serde_json::to_value(T::default()).unwrap()
    }
}

fn get_all_providers() -> Vec<Box<dyn ConfigProvider>> {
    vec![
        // Add new settings classes here. TODO: use inventory?
        Box::new(Provider::<dashboard::DashboardSettings>(std::marker::PhantomData)),
        Box::new(Provider::<herald::HeraldSettings>(std::marker::PhantomData)),
    ]
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DiscordConfig {
    pub bot_token: String,
    pub channel_id: u64,
    pub admin_channel_id: u64,
    pub general_channel_id: u64,
    pub admin_role_id: u64,
    pub disabled_modules: Vec<String>,
    pub blocked_notifications: Vec<String>,
    
    /// New: Stores module-specific JSON objects
    #[serde(default)]
    pub modules: HashMap<String, serde_json::Value>,
}

impl DiscordConfig {
    /// Helper to extract a specific module's config or return its Default
    pub fn get_module_config<T: ModuleConfig>(&self) -> Option<T> {
        self.modules
            .get(T::key())
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub fn load(path: &str, refresh: bool) -> Result<Self, String> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                // create a default one.
                let default_config = Self::default();
                let json = serde_json::to_string_pretty(&default_config).unwrap();
                std::fs::write(path, json).map_err(|e| e.to_string())?;
                return Ok(default_config);
            }
            Err(e) => return Err(format!("Failed to read file: {}", e)),
        };

        let mut config: DiscordConfig = serde_json::from_str(&content)
            .map_err(|e| format!("JSON Syntax Error at {}: {}", path, e))?;

        if refresh {
            let mut changed = false;

            for provider in get_all_providers() {
                let key = provider.key().to_string();
                let default_val = provider.default_value();

                if !config.modules.contains_key(&key) {
                    config.modules.insert(key.clone(), default_val.clone());
                    println!("Module Added: {}", key);
                    changed = true;
                }

                let existing_val = config.modules.get_mut(&key).unwrap();
                if let (Some(existing_obj), Some(default_obj)) = (existing_val.as_object_mut(), default_val.as_object()) {
                    for (field, val) in default_obj {
                        if !existing_obj.contains_key(field) {
                            existing_obj.insert(field.clone(), val.clone());
                            println!("  └─ New Field Added to {}: {}", key, field);
                            changed = true;
                        }
                    }
                }
            }

            // 3. Save only if we actually touched something
            if changed {
                let fresh_json = serde_json::to_string_pretty(&config).unwrap();
                let _ = std::fs::write(path, fresh_json);
                println!("Config file updated with missing defaults.");
            }
        }

        Ok(config)
    }
}

impl Default for DiscordConfig {
    fn default() -> Self {
        let mut modules = HashMap::new();

        for provider in get_all_providers() {
            modules.insert(provider.key().to_string(), provider.default_value());
        }

        Self {
            bot_token: "INSERT_TOKEN_HERE".into(),
            channel_id: 0,
            admin_channel_id: 0,
            general_channel_id: 0,
            admin_role_id: 0,
            disabled_modules: vec![],
            blocked_notifications: vec![],
            modules,
        }
    }
}