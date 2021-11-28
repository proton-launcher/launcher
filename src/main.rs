use std::{env::set_current_dir, error::Error, io::stdin};

use authentication::{Profile, authenticate};
use game::{RunArguments, install_installation, parse_installation, run_installation};

struct State {
    current_profile: Option<Profile>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let _ = set_current_dir("runtime");

    let mut state = State {
        current_profile: None,
    };

    loop {
        let mut input = String::new();
        stdin().read_line(&mut input)?;

        let arguments: Vec<&str> = input.trim_end().split(" ").collect();
        match arguments[0] {
            "account" => {
                match arguments[1] {
                    "login" => {
                        state.current_profile = Some(authenticate()?);
                    },
                    _ => (),
                }
            },
            "game" => {
                match arguments[1] {
                    "install" => {
                        let installation = parse_installation(arguments[2].to_string())?;
                        install_installation(&installation)?;
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
