/*use std::{
    env::set_current_dir,
    error::Error,
    fs::File,
    io::{stdin, BufReader, Write},
    path::Path,
};

use authentication::{authenticate, Profile};
use game::{
    download_installation, install_installation, parse_installation, run_installation, RunArguments,
};
use settings::{initialize_settings, Setting, SettingManager};

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
            "account" => match arguments[1] {
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
                    println!(
                        "Logged in as: {}",
                        state.current_profile.as_ref().unwrap().username
                    );
                }
                _ => (),
            },
            "game" => match arguments[1] {
                "install" => {
                    println!("Installing...");
                    if let Setting::Boolean(value) = state
                        .setting_manager
                        .get_setting("developer_mode".into())
                        .unwrap()
                    {
                        if !value {
                            download_installation(arguments[2].to_string())?;
                        }
                    }
                    let installation = parse_installation(arguments[2].to_string())?;
                    install_installation(&installation)?;
                    println!("Finished!");
                }
                "launch" => {
                    let installation = parse_installation(arguments[2].to_string())?;
                    let profile = state
                        .current_profile
                        .as_ref()
                        .ok_or("Launching without signing in")?;
                    run_installation(
                        &installation,
                        RunArguments {
                            token: profile.token.clone(),
                            uuid: profile.uuid.clone(),
                            username: profile.username.clone(),
                        },
                        &state.setting_manager,
                    )?;
                }
                _ => (),
            },
            "settings" => {
                let setting_manager = &mut state.setting_manager;
                match arguments[1] {
                    "set" => {
                        let id = arguments[2].to_string();
                        let wanted = arguments[3];
                        match setting_manager
                            .get_setting_mut(id.clone())
                            .ok_or(format!("Nonexistent setting: {}", id))?
                        {
                            Setting::Boolean(value) => *value = wanted.parse::<bool>()?,
                            Setting::Integer(value) => *value = wanted.parse::<i32>()?,
                            Setting::String(string) => *string = wanted.parse::<String>()?,
                            Setting::StringArray(array) => {
                                *array = wanted
                                    .split(",")
                                    .filter(|value| !value.is_empty())
                                    .map(|value| value.to_string())
                                    .collect()
                            }
                            Setting::Null => return Err("idk what just happened".into()),
                        };
                    }
                    _ => (),
                }

                setting_manager.save()?;
            }
            _ => (),
        }
    }
}*/

use std::env::set_current_dir;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

use authentication::{Profile, authenticate, refresh};
use clap::error::{Error, ErrorKind};
use clap::{ArgMatches, Args as _, Command, FromArgMatches, Parser, Subcommand};
use game::{
    download_installation, install_installation, parse_installation, run_installation, RunArguments,
};
use settings::{initialize_settings, SettingManager};

#[derive(Parser, Debug)]
struct LaunchArgs {
    installation: String,
}

#[derive(Parser, Debug)]
struct RemoveArgs {
    #[clap(short, long)]
    force: bool,
    name: Vec<String>,
}

#[derive(Debug)]
enum CliSub {
    Launch(LaunchArgs),
}

impl FromArgMatches for CliSub {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
        match matches.subcommand() {
            Some(("launch", args)) => Ok(Self::Launch(LaunchArgs::from_arg_matches(args)?)),
            //Some(("remove", args)) => Ok(Self::Remove(RemoveArgs::from_arg_matches(args)?)),
            Some((_, _)) => Err(Error::raw(
                ErrorKind::UnrecognizedSubcommand,
                "Valid subcommands are `launch`",
            )),
            None => Err(Error::raw(
                ErrorKind::MissingSubcommand,
                "Valid subcommands are `launch`",
            )),
        }
    }
    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
        match matches.subcommand() {
            Some(("launch", args)) => *self = Self::Launch(LaunchArgs::from_arg_matches(args)?),
            //Some(("remove", args)) => *self = Self::Remove(RemoveArgs::from_arg_matches(args)?),
            Some((_, _)) => {
                return Err(Error::raw(
                    ErrorKind::UnrecognizedSubcommand,
                    "Valid subcommands are `launch`",
                ))
            }
            None => (),
        };
        Ok(())
    }
}

impl Subcommand for CliSub {
    fn augment_subcommands(cmd: Command<'_>) -> Command<'_> {
        cmd.subcommand(LaunchArgs::augment_args(Command::new("launch")))
            //.subcommand(RemoveArgs::augment_args(Command::new("remove")))
            .subcommand_required(true)
    }
    fn augment_subcommands_for_update(cmd: Command<'_>) -> Command<'_> {
        cmd.subcommand(LaunchArgs::augment_args(Command::new("launch")))
            //.subcommand(RemoveArgs::augment_args(Command::new("remove")))
            .subcommand_required(true)
    }
    fn has_subcommand(name: &str) -> bool {
        matches!(name, "launch")
    }
}

#[derive(Parser, Debug)]
struct Cli {
    #[clap(subcommand)]
    subcommand: CliSub,
}

struct State {
    current_profile: Option<Profile>,
    setting_manager: SettingManager,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    if cfg!(debug_assertions) {
        let _ = set_current_dir("runtime");
    }

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

    match args.subcommand {
        CliSub::Launch(launch_args) => {
            if !Path::new(&format!("installation/files/{}", launch_args.installation)).exists() {
                println!("Installing {}...", launch_args.installation);

                download_installation(launch_args.installation.to_string())?;
                let installation = parse_installation(launch_args.installation.to_string())?;
                install_installation(&installation)?;
                println!("Finished!");
            }

            let installation = parse_installation(launch_args.installation)?;
            /*let profile = state
                .current_profile
                .as_mut()
                .ok_or("Launching without signing in")?;*/

            let profile = match state.current_profile.as_ref() {
                Some(profile) => {
                    let new_profile = refresh(profile)?;
                    state.current_profile = Some(new_profile);

                    let mut file = File::create("account.json")?;
                    file.write_all(serde_json::to_string(&state.current_profile)?.as_bytes())?;

                    state.current_profile.unwrap()
                }
                None => {
                    let new_profile = authenticate()?;
                    state.current_profile = Some(new_profile);

                    let mut file = File::create("account.json")?;
                    file.write_all(serde_json::to_string(&state.current_profile)?.as_bytes())?;

                    state.current_profile.unwrap()
                }
            };

            run_installation(
                &installation,
                RunArguments {
                    token: profile.token.clone(),
                    uuid: profile.uuid.clone(),
                    username: profile.username.clone(),
                },
                &state.setting_manager,
            )?;
        }
    }

    Ok(())
}
