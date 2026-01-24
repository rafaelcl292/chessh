use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use russh::server::{Auth, Handler, Msg, Server as RusshServer, Session as RusshSession};
use russh::{Channel, ChannelId, CryptoVec};
use russh::keys::PrivateKey;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use super::session::{SessionId, TerminalSize};
use crate::server::SessionManager;

pub struct SshServerConfig {
    pub address: SocketAddr,
    pub host_key: PrivateKey,
}

impl SshServerConfig {
    pub fn new(address: SocketAddr, host_key: PrivateKey) -> Self {
        Self { address, host_key }
    }
}

pub struct SshServer {
    session_manager: Arc<RwLock<SessionManager>>,
}

impl SshServer {
    pub fn new(session_manager: Arc<RwLock<SessionManager>>) -> Self {
        Self { session_manager }
    }

    pub async fn run(mut self, config: SshServerConfig) -> Result<(), Box<dyn std::error::Error>> {
        let russh_config = russh::server::Config {
            auth_rejection_time: std::time::Duration::from_secs(1),
            auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
            keys: vec![config.host_key],
            ..Default::default()
        };

        info!("Starting SSH server on {}", config.address);

        self.run_on_address(Arc::new(russh_config), config.address).await?;

        Ok(())
    }
}

impl RusshServer for SshServer {
    type Handler = ConnectionHandler;

    fn new_client(&mut self, peer_addr: Option<SocketAddr>) -> Self::Handler {
        info!("New connection from {:?}", peer_addr);
        ConnectionHandler::new(self.session_manager.clone(), peer_addr)
    }

    fn handle_session_error(&mut self, error: russh::Error) {
        error!("Session error: {}", error);
    }
}

pub struct ConnectionHandler {
    session_manager: Arc<RwLock<SessionManager>>,
    #[allow(dead_code)]
    peer_addr: Option<SocketAddr>,
    session_id: SessionId,
    channel_writers: HashMap<ChannelId, mpsc::Sender<Vec<u8>>>,
    terminal_size: TerminalSize,
}

impl ConnectionHandler {
    fn new(session_manager: Arc<RwLock<SessionManager>>, peer_addr: Option<SocketAddr>) -> Self {
        Self {
            session_manager,
            peer_addr,
            session_id: SessionId::new(),
            channel_writers: HashMap::new(),
            terminal_size: TerminalSize::default(),
        }
    }
}

impl Handler for ConnectionHandler {
    type Error = russh::Error;

    async fn auth_none(&mut self, user: &str) -> Result<Auth, Self::Error> {
        debug!("Auth none for user: {}", user);
        Ok(Auth::Accept)
    }

    async fn auth_password(&mut self, user: &str, _password: &str) -> Result<Auth, Self::Error> {
        debug!("Auth password for user: {}", user);
        Ok(Auth::Accept)
    }

    async fn auth_publickey_offered(
        &mut self,
        _user: &str,
        _public_key: &russh::keys::PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn auth_publickey(
        &mut self,
        _user: &str,
        _public_key: &russh::keys::PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut RusshSession,
    ) -> Result<bool, Self::Error> {
        debug!("Channel open session: {:?}", channel.id());
        Ok(true)
    }

    async fn pty_request(
        &mut self,
        _channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut RusshSession,
    ) -> Result<(), Self::Error> {
        debug!(
            "PTY request: term={}, size={}x{}",
            term, col_width, row_height
        );
        self.terminal_size = TerminalSize {
            width: col_width,
            height: row_height,
        };
        session.request_success();
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut RusshSession,
    ) -> Result<(), Self::Error> {
        debug!("Shell request on channel {:?}", channel);
        session.request_success();

        let session_manager = self.session_manager.clone();
        let session_id = self.session_id;
        let terminal_size = TerminalSize {
            width: self.terminal_size.width,
            height: self.terminal_size.height,
        };

        let handle = session.handle();
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(256);
        self.channel_writers.insert(channel, tx);

        tokio::spawn(async move {
            let mut manager = session_manager.write().await;
            manager.add_session(session_id, terminal_size);
            drop(manager);

            while let Some(data) = rx.recv().await {
                if let Some(app_response) = process_input(&session_manager, session_id, &data).await
                {
                    let _ = handle.data(channel, CryptoVec::from(app_response)).await;
                }
            }

            let mut manager = session_manager.write().await;
            manager.remove_session(session_id);
        });

        let welcome = format!(
            "\x1b[2J\x1b[H\r\n  Welcome to CheSSH!\r\n\r\n  Session: {}\r\n  Terminal: {}x{}\r\n\r\n  Type 'help' for commands.\r\n\r\n> ",
            self.session_id,
            self.terminal_size.width,
            self.terminal_size.height
        );
        let _ = session.data(channel, CryptoVec::from(welcome.as_bytes().to_vec()));

        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut RusshSession,
    ) -> Result<(), Self::Error> {
        debug!("Window change: {}x{}", col_width, row_height);
        self.terminal_size = TerminalSize {
            width: col_width,
            height: row_height,
        };
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut RusshSession,
    ) -> Result<(), Self::Error> {
        if let Some(tx) = self.channel_writers.get(&channel) {
            let _ = tx.send(data.to_vec()).await;
        }

        if data == b"\x03" {
            let _ = session.close(channel);
        } else if data == b"\r" || data == b"\n" {
            let _ = session.data(channel, CryptoVec::from("\r\n> ".as_bytes().to_vec()));
        } else {
            let _ = session.data(channel, CryptoVec::from(data.to_vec()));
        }

        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut RusshSession,
    ) -> Result<(), Self::Error> {
        debug!("Channel close: {:?}", channel);
        self.channel_writers.remove(&channel);
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut RusshSession,
    ) -> Result<(), Self::Error> {
        debug!("Channel EOF: {:?}", channel);
        let _ = session.close(channel);
        Ok(())
    }
}

async fn process_input(
    _session_manager: &Arc<RwLock<SessionManager>>,
    _session_id: SessionId,
    _data: &[u8],
) -> Option<Vec<u8>> {
    None
}
