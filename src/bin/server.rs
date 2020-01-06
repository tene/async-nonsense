use chat::web::Web;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut web = Web::new().await;
    web.listen_unix("chat.socket").await?;
    loop {}
}
