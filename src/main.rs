use std::{env::set_current_dir, error::Error, fs::File, io::{BufReader, Write, stdin}, path::Path};

use authentication::{Profile, authenticate};
use game::{RunArguments, install_installation, parse_installation, run_installation, download_installation};
use settings::{SettingManager, initialize_settings, Setting};

struct State {
    current_profile: Option<Profile>,
    setting_manager: SettingManager,
}

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        let _ = set_current_dir("runtime");
    }
    println!("Proton Launcher CLI:");

    let mut state = State {
        current_profile: None,
        setting_manager: initialize_settings()?,
    };

    let account_file = Path::new("account.json");
    if account_file.exists() {
        let file = File::open(account_file)?;
        let profile: Profile = serde_json::from_reader(BufReader::new(file))?;
        state.current_profile = Some(profile);
    }

    loop {
        let mut input = String::new();
        stdin().read_line(&mut input)?;

        let arguments: Vec<&str> = input.trim_end().split(" ").collect();
        match arguments[0] {
            "account" => {
                match arguments[1] {
                    "login" => {
                        let profile = authenticate()?;
                        let save_to_file = if let Some(argument) = arguments.get(2) {
                            argument == &"save"
                        } else {
                            false
                        };

                        if save_to_file {
                            let mut file = File::create("account.json")?;
                            file.write_all(serde_json::to_string(&profile)?.as_bytes())?;
                        }

                        state.current_profile = Some(profile);
                        println!("Logged in as: {}", state.current_profile.as_ref().unwrap().username);
                    },
                    _ => (),
                }
            },
            "game" => {
                match arguments[1] {
                    "install" => {
                        println!("Installing...");
                        download_installation(arguments[2].to_string())?;
                        let installation = parse_installation(arguments[2].to_string())?;
                        install_installation(&installation)?;
                        println!("Finished!");
                    },
                    "launch" => {
                        let installation = parse_installation(arguments[2].to_string())?;
                        let profile = state.current_profile.as_ref().ok_or("Launching without signing in")?;
                        run_installation(&installation, RunArguments {
                            token: profile.token.clone(),
                            uuid: profile.uuid.clone(),
                            username: profile.username.clone(),
                        }, &state.setting_manager)?;
                    },
                    _ => (),
                }
            },
            "settings" => {
                let setting_manager = &mut state.setting_manager;
                match arguments[1] {
                    "set" => {
                        let id = arguments[2].to_string();
                        let wanted = arguments[3];
                        match setting_manager.get_setting_mut(id.clone()).ok_or(format!("Nonexistent setting: {}", id))? {
                            Setting::Boolean(value) => *value = wanted.parse::<bool>()?,
                            Setting::Integer(value) => *value = wanted.parse::<i32>()?,
                            Setting::String(string) => *string = wanted.parse::<String>()?,
                            Setting::StringArray(array) => *array = wanted.split(",").filter(|value| !value.is_empty()).map(|value| value.to_string()).collect(),
                            Setting::Null => return Err("idk what just happened".into()),
                        };
                    },
                    _ => (),
                }

                setting_manager.save()?;
            }
            _ => (),
        }
    }
}
