//src\main.rs
use serde_json::Value;
use std::io::{self, Write};
use std::fs;
use std::collections::HashMap;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use rustyline::history::History;
pub mod games;
pub mod storage;
pub mod convert_to_rust;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    print!("\x1B[2J\x1B[1;1H"); io::stdout().flush().unwrap();
    let config: Value = serde_json::from_str(&(fs::read_to_string("./configs/config.json").unwrap_or_default())).unwrap_or_default();
    let location = config.get("location").and_then(|v| v.as_str()).unwrap_or("./");
    tokio::fs::create_dir_all(&format!("{location}/temporary")).await?;
    
    let all_providers_games: HashMap<String, Vec<String>> = serde_json::from_str(&(fs::read_to_string("./configs/games.json".to_string()).unwrap_or_default())).unwrap_or_default();

    let mut supported_providers: Vec<String> = all_providers_games.keys().cloned().collect();
    supported_providers.sort();
    let mut rp = DefaultEditor::new()?;
    tokio::fs::create_dir_all(&format!("{location}/temporary/games")).await?;
    let providers_history_path = &format!("{location}/temporary/games/history.txt");
    let _ = rp.load_history(providers_history_path);
    println!("Supported game providers:");
    for p in &supported_providers {println!("\t- {}", p);}
    if rp.history().is_empty() {
        for p in &supported_providers {
            let _ = rp.add_history_entry(p);
            let _ = rp.save_history(providers_history_path);
        }
    }
    let provider_name = loop {
        match rp.readline("Input game provider (required): ") {
            Ok(line) => {
                let trimmed = line.trim().to_string();
                if !trimmed.is_empty() && supported_providers.contains(&trimmed) {
                    let items: Vec<String> = rp.history().iter().filter(|h| h.as_str() != trimmed).cloned().collect();
                    let _ = rp.clear_history();
                    for h in &items {let _ = rp.add_history_entry(h.as_str());}
                    let _ = rp.save_history(providers_history_path);
                    let _ = rp.add_history_entry(trimmed.as_str());
                    if rp.append_history(providers_history_path).is_err() {let _ = rp.save_history(providers_history_path);}
                    break trimmed;
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => { return Ok(()); }
            Err(err) => { eprintln!("Error: {:?}", err); return Ok(()); }
        }
    };

    let mut supported_games = all_providers_games.get(&provider_name).cloned().unwrap_or_default();
    supported_games.sort();
    let mut rg = DefaultEditor::new()?;
    tokio::fs::create_dir_all(&format!("{location}/temporary/games/{provider_name}")).await?;
    let games_history_path = &format!("{location}/temporary/games/{provider_name}/history.txt");
    let _ = rg.load_history(games_history_path);
    println!("Supported games for provider '{}':", provider_name);
    for g in &supported_games {println!("\t- {}", g);}
    if rg.history().is_empty() {
        for g in &supported_games {
            let _ = rg.add_history_entry(g);
            let _ = rg.save_history(games_history_path);
        }
    }
    let game_name = loop {
        match rg.readline("Input game name (required): ") {
            Ok(line) => {
                let trimmed = line.trim().to_string();
                if !trimmed.is_empty() && supported_games.contains(&trimmed) {
                    let items: Vec<String> = rg.history().iter().filter(|h| h.as_str() != trimmed).cloned().collect();
                    let _ = rg.clear_history();
                    for h in &items {let _ = rg.add_history_entry(h.as_str());}
                    let _ = rg.save_history(games_history_path);
                    let _ = rg.add_history_entry(trimmed.as_str());
                    if rg.append_history(games_history_path).is_err() {let _ = rg.save_history(games_history_path);}
                    break trimmed;
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => { return Ok(()); }
            Err(err) => { eprintln!("Error: {:?}", err); return Ok(()); }
        }
    };

    let game_config: Value = serde_json::from_str(&(fs::read_to_string(format!("./configs/games/{provider_name}/{game_name}.json")).unwrap_or_default())).unwrap_or_default();
    
    let mut supported_modes: Vec<String> = game_config.get("modes").and_then(|b| b.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    supported_modes.sort_by_key(|s| s.parse::<u64>().unwrap_or(u64::MAX));
    let mut rg = DefaultEditor::new()?;
    tokio::fs::create_dir_all(&format!("{location}/temporary/games/{provider_name}/{game_name}")).await?;
    let modes_history_path = &format!("{location}/temporary/games/{provider_name}/{game_name}/mode_history.txt");
    let _ = rg.load_history(modes_history_path);
    println!("Supported modes for game '{}':", game_name);
    for m in &supported_modes {println!("\t- {}", m);}
    if rg.history().is_empty() {
        for m in &supported_modes {
            let _ = rg.add_history_entry(m);
            let _ = rg.save_history(modes_history_path);
        }
    }
    let mode = loop {
        match rg.readline("Input mode or press enter to skip: ") {
            Ok(line) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {break None;} 
                if supported_modes.contains(&trimmed) {
                    let items: Vec<String> = rg.history().iter().filter(|h| h.as_str() != trimmed).cloned().collect();
                    let _ = rg.clear_history();
                    for h in &items {let _ = rg.add_history_entry(h.as_str());}
                    let _ = rg.save_history(modes_history_path);
                    let _ = rg.add_history_entry(trimmed.as_str());
                    if rg.append_history(modes_history_path).is_err() {let _ = rg.save_history(modes_history_path);}
                    break Some(trimmed);
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => { return Ok(()); }
            Err(err) => { eprintln!("Error: {:?}", err); return Ok(()); }
        }
    };
    
    let mut supported_commands: Vec<String> = game_config.get("commands").and_then(|b| b.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    supported_commands.sort();
    let mut rg = DefaultEditor::new()?;
    let commands_history_path = &format!("{location}/temporary/games/{provider_name}/{game_name}/command_history.txt");
    let _ = rg.load_history(commands_history_path);
    println!("Supported commands for game '{}':", game_name);
    for m in &supported_commands {println!("\t- {}", m);}
    if rg.history().is_empty() {
        for m in &supported_commands {
            let _ = rg.add_history_entry(m);
            let _ = rg.save_history(commands_history_path);
        }
    }
    let command = loop {
        match rg.readline("Input command or press enter to skip: ") {
            Ok(line) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {break None;} 
                if supported_commands.contains(&trimmed) {
                    let items: Vec<String> = rg.history().iter().filter(|h| h.as_str() != trimmed).cloned().collect();
                    let _ = rg.clear_history();
                    for h in &items {let _ = rg.add_history_entry(h.as_str());}
                    let _ = rg.save_history(commands_history_path);
                    let _ = rg.add_history_entry(trimmed.as_str());
                    if rg.append_history(commands_history_path).is_err() {let _ = rg.save_history(commands_history_path);}
                    break Some(trimmed);
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => { return Ok(()); }
            Err(err) => { eprintln!("Error: {:?}", err); return Ok(()); }
        }
    };
    
    let mut supported_actions: Vec<String> = game_config.get("actions").and_then(|b| b.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    supported_actions.sort();
    let mut rg = DefaultEditor::new()?;
    let actions_history_path = &format!("{location}/temporary/games/{provider_name}/{game_name}/action_history.txt");
    let _ = rg.load_history(actions_history_path);
    println!("Supported actions for game '{}':", game_name);
    for m in &supported_actions {println!("\t- {}", m);}
    if rg.history().is_empty() {
        for m in &supported_actions {
            let _ = rg.add_history_entry(m);
            let _ = rg.save_history(actions_history_path);
        }
    }
    let action = loop {
        match rg.readline("Input action or press enter to skip: ") {
            Ok(line) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {break None;} 
                if supported_actions.contains(&trimmed) {
                    let items: Vec<String> = rg.history().iter().filter(|h| h.as_str() != trimmed).cloned().collect();
                    let _ = rg.clear_history();
                    for h in &items {let _ = rg.add_history_entry(h.as_str());}
                    let _ = rg.save_history(actions_history_path);
                    let _ = rg.add_history_entry(trimmed.as_str());
                    if rg.append_history(actions_history_path).is_err() {let _ = rg.save_history(actions_history_path);}
                    break Some(trimmed);
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => { return Ok(()); }
            Err(err) => { eprintln!("Error: {:?}", err); return Ok(()); }
        }
    };

    match provider_name.as_str() {
        "pragmaticplay" => {if let Err(e) = games::pragmaticplay::execute(&provider_name, &game_name, mode.as_deref(), command.as_deref(), action.as_deref()).await {eprintln!("Error executing {provider_name} game {game_name}: {e}");}},
        "enjoygaming" => {if let Err(e) = games::enjoygaming::execute(&provider_name, &game_name, mode.as_deref(), command.as_deref(), action.as_deref()).await {eprintln!("Error executing {provider_name} game {game_name}: {e}");}},
        _ => {println!("Provider not implement");}
    } 
    Ok(())
}