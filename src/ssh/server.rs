use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use russh::keys::PrivateKey;
use russh::server::{Auth, Handler, Msg, Server as RusshServer, Session as RusshSession};
use russh::{Channel, ChannelId};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use super::session::{SessionId, TerminalSize};
use super::session_runner::SessionRunner;
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
            nodelay: true,
            ..Default::default()
        };

        info!("Starting SSH server on {}", config.address);

        self.run_on_address(Arc::new(russh_config), config.address)
            .await?;

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
    session_id: SessionId,
    username: Option<String>,
    shell_started: bool,
    channel_writers: HashMap<ChannelId, mpsc::Sender<Vec<u8>>>,
    terminal_size: TerminalSize,
}

impl ConnectionHandler {
    fn new(session_manager: Arc<RwLock<SessionManager>>, _peer_addr: Option<SocketAddr>) -> Self {
        Self {
            session_manager,
            session_id: SessionId::new(),
            username: None,
            shell_started: false,
            channel_writers: HashMap::new(),
            terminal_size: TerminalSize::default(),
        }
    }
}

impl Handler for ConnectionHandler {
    type Error = russh::Error;

    async fn auth_none(&mut self, user: &str) -> Result<Auth, Self::Error> {
        debug!("Auth none for user: {}", user);
        self.username = Some(user.to_string());
        Ok(Auth::Accept)
    }

    async fn auth_password(&mut self, user: &str, _password: &str) -> Result<Auth, Self::Error> {
        debug!("Auth password for user: {}", user);
        self.username = Some(user.to_string());
        Ok(Auth::Accept)
    }

    async fn auth_publickey_offered(
        &mut self,
        user: &str,
        _public_key: &russh::keys::PublicKey,
    ) -> Result<Auth, Self::Error> {
        self.username = Some(user.to_string());
        Ok(Auth::Accept)
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        _public_key: &russh::keys::PublicKey,
    ) -> Result<Auth, Self::Error> {
        self.username = Some(user.to_string());
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut RusshSession,
    ) -> Result<(), Self::Error> {
        debug!("Channel open session: {:?}", channel.id());
        reply.accept().await;
        Ok(())
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
        if self.shell_started {
            session.request_failure();
            return Ok(());
        }
        self.shell_started = true;
        session.request_success();

        let session_manager = self.session_manager.clone();
        let session_id = self.session_id;
        let terminal_size = TerminalSize {
            width: self.terminal_size.width,
            height: self.terminal_size.height,
        };
        let username = self.username.clone().unwrap_or_else(|| "guest".to_string());

        let handle = session.handle();
        let (input_tx, input_rx) = mpsc::channel::<Vec<u8>>(256);
        let (output_tx, mut output_rx) = mpsc::channel::<Vec<u8>>(256);
        let (game_event_tx, game_event_rx) = mpsc::unbounded_channel::<super::session::GameEvent>();

        self.channel_writers.insert(channel, input_tx);

        let output_handle = handle.clone();
        let output_task = tokio::spawn(async move {
            while let Some(data) = output_rx.recv().await {
                let _ = output_handle.data(channel, data).await;
            }
        });

        let width = terminal_size.width as u16;
        let height = terminal_size.height as u16;

        tokio::spawn(async move {
            {
                let mut manager = session_manager.write().await;
                manager.add_session(session_id, terminal_size);
                manager.set_username(session_id, username.clone());
                manager.register_game_event_channel(session_id, game_event_tx);
            }

            let runner = SessionRunner::new(
                session_id,
                session_manager.clone(),
                input_rx,
                output_tx,
                game_event_rx,
                width,
                height,
            );

            runner.run(username).await;

            {
                let mut manager = session_manager.write().await;
                manager.remove_session(session_id);
            }

            // Drain terminal restoration and goodbye output before closing SSH.
            let _ = output_task.await;
            let _ = handle.exit_status_request(channel, 0).await;
            let _ = handle.eof(channel).await;
            let _ = handle.close(channel).await;
        });

        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
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

        if let Some(input_tx) = self.channel_writers.get(&channel) {
            let width = col_width as u16;
            let height = row_height as u16;
            let resize_data = vec![
                0xFF,
                0xFE,
                (width >> 8) as u8,
                (width & 0xFF) as u8,
                (height >> 8) as u8,
                (height & 0xFF) as u8,
            ];
            let _ = input_tx.send(resize_data).await;
        }

        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut RusshSession,
    ) -> Result<(), Self::Error> {
        if let Some(tx) = self.channel_writers.get(&channel) {
            let _ = tx.send(data.to_vec()).await;
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
