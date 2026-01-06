//src\games\pragmaticplay\big_bass_bonanza_1000\mod.rs
use serde_json::Value;
use std::fs;
use crate::storage::{load_transactions, save_content, };
use crate::convert_to_rust::generate_structs;


pub async fn execute(provider_name: &str, game_name: &str, mode: Option<&str>, command: Option<&str>, action: Option<&str>, ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config: Value = serde_json::from_str(&(fs::read_to_string("./configs/config.json").unwrap_or_default())).unwrap_or_default();
    let location = config.get("location").and_then(|v| v.as_str()).unwrap_or("./");
    let game_config: Value = serde_json::from_str(&(fs::read_to_string(format!("./configs/games/{provider_name}/{game_name}.json")).unwrap_or_default())).unwrap_or_default();
    let binding = vec![];
    let skip_comments: Vec<&str> = game_config.get("skip_comments").and_then(|v| v.as_array()).unwrap_or(&binding).iter().filter_map(|v| v.as_str().map(|s| s)).collect();
    let rename: Vec<&str> = game_config.get("rename").and_then(|v| v.as_array()).unwrap_or(&binding).iter().filter_map(|v| v.as_str().map(|s| s)).collect();

    let mode_path = if let Some(mode) = mode {format!("/bet_{mode}")} else {"".to_string()};
    let command_path = if let Some(command) = command {format!("/{command}")} else {"".to_string()};
    let action_name = if let Some(action) = action {format!("{action}_")} else {format!("{game_name}_")};
    let transactions_path = format!("{location}/{provider_name}/{game_name}/transactions{mode_path}");
    let transactions: Vec<Value> = load_transactions(transactions_path);
    {
        let ins: Vec<Value> = transactions.iter()
        .filter(|tx| {
            (tx.get("in")
                .and_then(|o| o.get("command"))
                .and_then(|c| c.as_str())
                == command || command.is_none())
            &&
            (tx.get("in")
                .and_then(|c| c.get("action"))
                .and_then(|a| a.as_str())
                == action || action.is_none())
        })
        .filter_map(|tx| tx.get("in").cloned()).collect();
        let root_name = format!("{action_name}in");
        let rust_struct = generate_structs(&root_name, &ins, &skip_comments, &rename, false, format!("{game_name}_in"), format!("use crate::{game_name}_in::"));
        let structure_path = format!("{location}/{provider_name}/{game_name}/models{mode_path}{command_path}/{root_name}.rs");
        save_content(structure_path, rust_struct);
    }
    {
        let outs: Vec<Value> = transactions.iter()
        .filter(|tx| {
            (tx.get("in")
                .and_then(|o| o.get("command"))
                .and_then(|c| c.as_str())
                == command || command.is_none())
            &&
            (tx.get("in")
                .and_then(|c| c.get("action"))
                .and_then(|a| a.as_str())
                == action || action.is_none())
        })
        .filter_map(|tx| tx.get("out").cloned()).collect();
        let root_name = format!("{action_name}out");
        let rust_struct = generate_structs(&root_name, &outs, &skip_comments, &rename, false, format!("{game_name}_out"), format!("use crate::{game_name}_out::"));
        let structure_path = format!("{location}/{provider_name}/{game_name}/models{mode_path}{command_path}/{root_name}.rs");
        save_content(structure_path, rust_struct);
    }
    Ok(())
}