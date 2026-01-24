use std::net::SocketAddr;
use std::sync::Arc;

use russh::keys::ssh_key::rand_core::OsRng;
use russh::keys::PrivateKey;
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use chessh::server::SessionManager;
use chessh::ssh::{SshServer, SshServerConfig};

const DEFAULT_PORT: u16 = 2222;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    FmtSubscriber::builder().with_max_level(Level::INFO).init();

    info!("CheSSH - SSH Chess Server");
    info!("Generating host key...");

    let host_key = PrivateKey::random(&mut OsRng, russh::keys::Algorithm::Ed25519)
        .expect("Failed to generate host key");

    let address: SocketAddr = format!("0.0.0.0:{}", DEFAULT_PORT).parse()?;
    let config = SshServerConfig::new(address, host_key);

    let session_manager = Arc::new(RwLock::new(SessionManager::new()));

    let server = SshServer::new(session_manager);

    info!(
        "Connect with: ssh -p {} -o StrictHostKeyChecking=no localhost",
        DEFAULT_PORT
    );

    server.run(config).await?;

    Ok(())
}
