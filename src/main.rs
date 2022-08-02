use std::env::set_current_dir;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use authentication::{Profile, authenticate, refresh};
use clap::error::{Error, ErrorKind};
use clap::{ArgMatches, Args as _, Command, FromArgMatches, Parser, Subcommand};
use dirs::config_dir;
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

    let _ = set_current_dir(if cfg!(debug_assertions) {
        PathBuf::from("runtime")
    } else {
        let mut path = config_dir().unwrap();
        path.push("proton-launcher");
        path
    });

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
