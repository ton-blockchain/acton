mod debugger;
mod server;

use anyhow::Result;
use server::McpServer;

#[tokio::main]
async fn main() -> Result<()> {
    unsafe {
        std::env::set_var(
            "TONCENTER_API_KEY",
            "49efa980ccdcd018fd09d387e63537afd9db4dbb8509d69e7bc2303ca2b2c860",
        )
    }

    let server = McpServer::new();
    server.run().await?;
    Ok(())
}
