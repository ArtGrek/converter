//src\games\pragmaticplay\mod.rs
pub mod big_bass_bonanza_1000;

pub async fn execute(provider_name: &str, game_name: &str, mode: Option<&str>, command: Option<&str>, action: Option<&str>, ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match game_name {
        "big_bass_bonanza_1000" => {big_bass_bonanza_1000::execute(&provider_name, &game_name, mode, command, action).await},
        _ => {Err(format!("\r\tGame not implement").into())}
    }
}