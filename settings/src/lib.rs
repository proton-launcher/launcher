use std::{collections::HashMap, error::Error, fs::File, io::{Read, Write}};

use serde_json::{Value, Map};

pub enum Setting {
    Boolean(bool),
    StringArray(Vec<String>),
    Null,
}

pub struct SettingManager {
    settings: HashMap<String, Setting>
}

impl SettingManager {
    pub fn get_setting(&self, id: String) -> Option<&Setting> {
        self.settings.get(&id)
    }
    pub fn set_setting(&mut self, id: String, setting: Setting) {
        self.settings.insert(id, setting);
    }
    pub fn save(&self) -> Result<(), Box<dyn Error>>{
        let mut file = File::create("launcher_settings.json")?;

        let mut map = Map::new();
        for (id, value) in &self.settings {
            map.insert(id.clone(), match value {
                Setting::Boolean(value) => Value::Bool(*value),
                Setting::StringArray(strings) => {
                    Value::Array(strings.iter().map(|string| {
                        Value::String(string.clone())
                    }).collect())
                },
                Setting::Null => Value::Null,
            });
        }

        let json = Value::Object(map);

        file.write_all(serde_json::to_string(&json)?.as_bytes())?;

        Ok(())
    }
}

pub fn initialize_settings() -> Result<SettingManager, Box<dyn Error>> {
    let mut settings = HashMap::new();

    let file = File::open("launcher_settings.json").ok();
    if let Some(mut file) = file {
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let json_contents: Value = serde_json::from_str(&contents)?;
        for (id, value) in json_contents.as_object().unwrap() {
            settings.insert(id.clone(), match value {
                Value::Bool(value) => Setting::Boolean(*value),
                Value::Array(array) => {
                    Setting::StringArray(array.iter().map(|value| {
                        value.as_str().unwrap().to_string()
                    }).collect())
                }
                _ => Setting::Null,
            });
        }
    }

    let setting_manager = SettingManager { settings };
    Ok(setting_manager)
}
