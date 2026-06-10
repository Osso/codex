use codex_hostrun::mcp_server::run_stdio_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_stdio_server().await
}
