use serde_json::Value;
use std::path::Path;
use std::fs;
//use std::io;
use walkdir::WalkDir;
use indicatif::{ProgressBar, ProgressStyle, };

pub fn load_transactions (a_location: String, ) -> Vec<Value>{
    let transactions_file_path = a_location;
    let pb_main = ProgressBar::new((2) as u64);
    pb_main.set_prefix("Load transactions from ".to_owned() + &transactions_file_path + ": ");
    pb_main.set_style(ProgressStyle::default_bar().template("{prefix} [{bar:100.cyan/blue}] {pos}/{len} {msg}").expect("ProgressBar template error"),);
    let mut l_transactions: Vec<Value> = Vec::new();
    if Path::new(&transactions_file_path).is_dir() {
        let total = WalkDir::new(&transactions_file_path).into_iter().filter_map(Result::ok).filter(|e| {e.path().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("json")}).count() as u64;
        pb_main.set_length(total);
        for entry in WalkDir::new(&transactions_file_path).into_iter().filter_map(Result::ok) {
            let path = entry.path().to_path_buf();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let mut content = match std::fs::read_to_string(&path) {
                    Ok(v) => v,
                    Err(e) => {println!("Read error in file {}: {}", path.display(), e); pb_main.inc(1); continue;}
                };
                if let Some(pos) = content.rfind('}') {content.truncate(pos + 1);}
                let data: Vec<Value> = match serde_json::from_str(&("[".to_owned() + &content.clone() + "]")) {
                    Ok(v) => v,
                    Err(e) => {println!("JSON parse error in file {}: {}", path.display(), e); pb_main.inc(1); continue;}
                };
                let filtered_data: Vec<Value> = data.iter().map(|item| item.clone()).collect();
                l_transactions.extend(filtered_data);
            }
            pb_main.inc(1);
        }
    } else if Path::new(&transactions_file_path).is_file() {
        let file_content = fs::read_to_string(transactions_file_path).unwrap_or_default();
        pb_main.set_position(1);
        l_transactions = serde_json::from_str(&file_content).unwrap_or_default();
        pb_main.set_position(2);
    } else {
        println!("Does not exist or is not defined: {}", transactions_file_path);
    }
    pb_main.finish_with_message(" -> loaded ".to_owned() + &l_transactions.len().to_string() + " transactions");
    l_transactions
}

pub fn save_content (a_location: String, a_content: String, ) {
    let path = a_location;
    if let Some(parent) = Path::new(&path).parent() {let _ = fs::create_dir_all(parent);}
    fs::write(path, a_content).unwrap();
}

