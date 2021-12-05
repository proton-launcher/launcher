use std::{process::Command, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=CustomSecurityManager.java");

    Command::new("javac")
        .arg("CustomSecurityManager.java")
        .spawn()?;

    Ok(())
}
