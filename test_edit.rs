//! A sophisticated greeting system with multiple greeting styles and time-based functionality.
//! 
//! This module provides various ways to greet users with different formality levels
//! and time-based greetings.

use std::time::{SystemTime, UNIX_EPOCH};

fn greet(name: &str) -> String {
    let trimmed_name = name.trim();
    let greeting = format!("Hello, {}! Welcome to our sophisticated greeting system.", trimmed_name);
    println!("{}", greeting);
    greeting
}

fn greet_formal(name: &str) -> String {
    let trimmed_name = name.trim();
    let formal_greeting = format!("Good day, {}. It's a pleasure to make your acquaintance.", trimmed_name);
    println!("{}", formal_greeting);
    formal_greeting
}

fn greet_with_time(name: &str) -> String {
    let trimmed_name = name.trim();
    let time_of_day = get_time_of_day();
    let time_greeting = format!("Good {}, {}! I hope you're having a wonderful {}.", 
                               time_of_day, trimmed_name, time_of_day);
    println!("{}", time_greeting);
    time_greeting
}

fn get_time_of_day() -> &'static str {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
    
    let seconds_since_midnight = now % 86400;
    
    match seconds_since_midnight {
        0..=10800 => "night",        // 12 AM - 3 AM
        10801..=43200 => "morning",  // 3 AM - 12 PM
        43201..=64800 => "afternoon", // 12 PM - 6 PM
        64801..=86400 => "evening",  // 6 PM - 12 AM
        _ => "night", // Fallback
    }
}