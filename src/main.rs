#[tokio::main]
async fn main() {
    mantis::server::run_server().await;
}
