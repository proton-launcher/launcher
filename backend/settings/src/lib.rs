use std::{collections::HashMap, error::Error, fs::File, io::{Read, Write}};

use serde_json::{Value, Map, Number};

pub enum Setting {
    Boolean(bool),
    Integer(i32),
    String(String),
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
    pub fn get_setting_mut(&mut self, id: String) -> Option<&mut Setting> {
        self.settings.get_mut(&id)
    }
    pub fn get_settings(&self) -> &HashMap<String, Setting> {
        &self.settings
    }
    pub fn save(&self) -> Result<(), Box<dyn Error>>{
        let mut file = File::create("launcher_settings.json")?;

        let mut map = Map::new();
        for (id, value) in &self.settings {
            map.insert(id.clone(), match value {
                Setting::Boolean(value) => Value::Bool(*value),
                Setting::Integer(integer) => Value::Number(Number::from(*integer)),
                Setting::String(string) => Value::String(string.clone()),
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

    let default_settings = {
        let mut map = HashMap::new();
        map.insert("memory".to_string(), Setting::Integer(1024));
        map.insert("java_executable".to_string(), Setting::String("java".into()));

        map
    };

    let file = File::open("launcher_settings.json").ok();
    if let Some(mut file) = file {
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let json_contents: Value = serde_json::from_str(&contents)?;
        for (id, value) in json_contents.as_object().unwrap() {
            settings.insert(id.clone(), match value {
                Value::Bool(value) => Setting::Boolean(*value),
                Value::Number(number) => {
                    if number.is_i64() {
                        Setting::Integer(number.as_i64().unwrap() as i32)
                    } else {
                        Setting::Null
                    }
                },
                Value::String(string) => {
                    Setting::String(string.clone())
                },
                Value::Array(array) => {
                    Setting::StringArray(array.iter().map(|value| {
                        value.as_str().unwrap().to_string()
                    }).collect())
                }
                _ => Setting::Null,
            });
        }
    }

    for (id, value) in default_settings {
        if !settings.contains_key(&id) {
            settings.insert(id.clone(), value);
        }
    }

    let setting_manager = SettingManager { settings };
    Ok(setting_manager)
}
