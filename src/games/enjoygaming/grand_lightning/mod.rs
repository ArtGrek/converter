//src\games\enjoygaming\grand_lightning\mod.rs
use serde_json::Value;
use std::fs;
use crate::storage::{load_transactions, save_content, };
use crate::convert_to_rust::generate_structs;


pub async fn execute(provider_name: &str, game_name: &str, mode: &str, action: &str, ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config: Value = serde_json::from_str(&(fs::read_to_string("./configs/config.json").unwrap_or_default())).unwrap_or_default();
    let location = config.get("location").and_then(|v| v.as_str()).unwrap_or("./");
    let game_config: Value = serde_json::from_str(&(fs::read_to_string(format!("./configs/games/{provider_name}/{game_name}.json")).unwrap_or_default())).unwrap_or_default();
    let binding = vec![];
    let skip_comments: Vec<&str> = game_config.get("skip_comments").and_then(|v| v.as_array()).unwrap_or(&binding).iter().filter_map(|v| v.as_str().map(|s| s)).collect();
    let rename: Vec<&str> = game_config.get("rename").and_then(|v| v.as_array()).unwrap_or(&binding).iter().filter_map(|v| v.as_str().map(|s| s)).collect();
    let transactions_path = format!("{location}/{provider_name}/{game_name}/transactions");
    let transactions: Vec<Value> = load_transactions(transactions_path);
    {
        let ins: Vec<Value> = transactions.iter().filter_map(|tx| tx.get("in").cloned()).collect();
        let root_name = "grand_lightning_in";
        let rust_struct = generate_structs(root_name, &ins, &skip_comments, &rename, false, "".to_string(), "".to_string());
        let structure_path = format!("{location}/{provider_name}/{game_name}/models/{root_name}.rs");
        save_content(structure_path, rust_struct);
    }
    {
        let outs: Vec<Value> = transactions.iter().filter_map(|tx| tx.get("out").cloned()).collect();
        let root_name = "grand_lightning_out";
        let rust_struct = generate_structs(root_name, &outs, &skip_comments, &rename, false, "".to_string(), "".to_string());
        let structure_path = format!("{location}/{provider_name}/{game_name}/models/{root_name}.rs");
        save_content(structure_path, rust_struct);
    }

    let transactions_path = format!("{location}/{provider_name}/{game_name}/transactions/bet_{mode}");
    let transactions: Vec<Value> = load_transactions(transactions_path);
    let root_name = "grand_lightning";
    let rust_struct = generate_structs(root_name, &transactions, &skip_comments, &rename, false, "".to_string(), "".to_string());
    let structure_path = format!("{location}/{provider_name}/{game_name}/models/bet_{mode}/{root_name}.rs");
    save_content(structure_path, rust_struct);
    
    match action {
        "doInit" => {
        },
        "doSpin" => {
        },
        "doCollect" => {
        },
        _ => {
        }
    }; 
    Ok(())
}