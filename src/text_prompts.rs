use anyhow::Result;
use std::io::{self, Write};

pub fn prompt_yes_no(message: &str) -> Result<bool> {
    eprintln!("\n┌─────────────────────────────────────────┐");
    eprintln!("│ {:<39} │", message);
    eprintln!("└─────────────────────────────────────────┘");
    eprint!("  (y/n): ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}

pub fn prompt_choice(message: &str, choices: &[&str]) -> Result<usize> {
    eprintln!("\n┌─────────────────────────────────────────┐");
    eprintln!("│ {:<39} │", message);
    eprintln!("├─────────────────────────────────────────┤");

    for (i, choice) in choices.iter().enumerate() {
        eprintln!("│ {}. {:<37} │", i + 1, choice);
    }

    eprintln!("└─────────────────────────────────────────┘");
    eprint!("  Choice (1-{}): ", choices.len());
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let choice = input.trim().parse::<usize>()?;
    if choice < 1 || choice > choices.len() {
        anyhow::bail!("Invalid choice");
    }

    Ok(choice - 1)
}

pub fn display_message(message: &str) {
    eprintln!("\n┌─────────────────────────────────────────┐");
    for line in message.lines() {
        eprintln!("│ {:<39} │", line);
    }
    eprintln!("└─────────────────────────────────────────┘\n");
}
