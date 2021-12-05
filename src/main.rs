use std::{env::set_current_dir, error::Error, fs::File, io::{BufReader, Write, stdin}, path::Path};

use authentication::{Profile, authenticate};
use game::{RunArguments, install_installation, parse_installation, run_installation, download_installation};

struct State {
    current_profile: Option<Profile>,
}

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        let _ = set_current_dir("runtime");
    }
    println!("Proton Launcher CLI:");

    let mut state = State {
        current_profile: None,
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
                        })?;
                    },
                    _ => (),
                }
            },
            _ => (),
        }
    }
}
