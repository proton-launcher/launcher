use std::error::Error;

use authentication::authenticate;

fn main() -> Result<(), Box<dyn Error>> {
    let profile = authenticate()?;
    println!("Token: {}", profile.token);
    println!("UUID: {}", profile.uuid);
    println!("Username: {}", profile.username);
    Ok(())
}
