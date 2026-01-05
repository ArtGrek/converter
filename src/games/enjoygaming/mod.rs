//src\games\enjoygaming\mod.rs
pub mod grand_lightning;

pub async fn execute(provider_name: &str, game_name: &str, mode: Option<&str>, command: Option<&str>, action: Option<&str>, ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match game_name {
        "grand_lightning" => {grand_lightning::execute(&provider_name, &game_name, mode, command, action).await},
        _ => {Err(format!("\r\tGame not implement").into())}
    }
}