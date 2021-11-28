use std::{env::set_current_dir, error::Error};

use installation::{install_installation, parse_installation, run_installation};

fn main() -> Result<(), Box<dyn Error>> {
    set_current_dir("runtime");
//    let profile = authenticate()?;
//    println!("Token: {}", profile.token);
//    println!("UUID: {}", profile.uuid);
//    println!("Username: {}", profile.username);
    let installation = parse_installation("1.8.9".to_string())?;
    println!("{:?}", installation);
    install_installation(&installation)?;
    run_installation(&installation)?;
    Ok(())
}
