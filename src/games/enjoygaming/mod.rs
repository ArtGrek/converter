//src\games\enjoygaming\mod.rs
pub mod grand_lightning;

pub async fn execute(provider_name: &str, game_name: &str, mode: &str, action: &str, ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match game_name {
        "grand_lightning" => {grand_lightning::execute(&provider_name, &game_name, mode, action).await},
        _ => {Err(format!("\r\tGame not implement").into())}
    }
}