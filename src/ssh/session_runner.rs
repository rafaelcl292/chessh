use std::sync::Arc;

use ratatui::backend::Backend;
use ratatui::Terminal;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use crate::server::SessionManager;
use crate::ssh::session::SessionId;
use crate::ui::{parse_input, App, AppAction, InputEvent, SshBackend};

pub struct SessionRunner {
    session_id: SessionId,
    session_manager: Arc<RwLock<SessionManager>>,
    input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
    width: u16,
    height: u16,
}

impl SessionRunner {
    pub fn new(
        session_id: SessionId,
        session_manager: Arc<RwLock<SessionManager>>,
        input_rx: mpsc::Receiver<Vec<u8>>,
        output_tx: mpsc::Sender<Vec<u8>>,
        width: u16,
        height: u16,
    ) -> Self {
        Self {
            session_id,
            session_manager,
            input_rx,
            output_tx,
            width,
            height,
        }
    }

    pub async fn run(mut self, username: String) {
        info!("Starting session runner for {}", self.session_id);

        let backend = SshBackend::new(self.output_tx.clone(), self.width, self.height);
        let mut terminal = match Terminal::new(backend) {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to create terminal: {}", e);
                return;
            }
        };

        let _ = terminal.clear();

        let mut app = App::new(username);

        {
            let manager = self.session_manager.read().await;
            app.set_online_count(manager.session_count());
            app.set_queue_size(manager.queue_size());
        }

        loop {
            if app.should_quit() {
                break;
            }

            if let Err(e) = terminal.draw(|frame| {
                app.draw(frame);
            }) {
                error!("Failed to draw: {}", e);
                break;
            }

            let input =
                tokio::time::timeout(std::time::Duration::from_millis(100), self.input_rx.recv())
                    .await;

            match input {
                Ok(Some(data)) => {
                    let event = parse_input(&data);
                    let action = app.handle_input(event);

                    match action {
                        AppAction::JoinQueue => {
                            app.join_queue();
                            let mut manager = self.session_manager.write().await;
                            manager.join_queue(self.session_id);
                            app.set_queue_size(manager.queue_size());
                        }
                        AppAction::LeaveQueue => {
                            app.leave_queue();
                            let mut manager = self.session_manager.write().await;
                            manager.leave_queue(self.session_id);
                            app.set_queue_size(manager.queue_size());
                        }
                        AppAction::Quit => {
                            break;
                        }
                        AppAction::Resign => {
                            if let Some(game) = app.game_mut() {
                                game.resign(shakmaty::Color::White);
                            }
                            app.set_status(Some("You resigned".to_string()));
                        }
                        AppAction::OfferDraw => {
                            app.set_status(Some("Draw offered".to_string()));
                        }
                        AppAction::SubmitMove(from, to) => {
                            let uci = format!("{}{}", from, to);
                            if let Some(game) = app.game_mut() {
                                match game.play_uci(&uci) {
                                    Ok(_) => {
                                        app.set_status(None);
                                    }
                                    Err(e) => {
                                        app.set_status(Some(format!("Invalid move: {}", e)));
                                    }
                                }
                            }
                        }
                        AppAction::None => {}
                    }

                    if let InputEvent::Resize(w, h) = event {
                        terminal.backend_mut().resize(w, h);
                    }
                }
                Ok(None) => {
                    debug!("Input channel closed");
                    break;
                }
                Err(_) => {
                    let manager = self.session_manager.read().await;
                    app.set_online_count(manager.session_count());
                    app.set_queue_size(manager.queue_size());
                }
            }
        }

        let _ = terminal.clear();
        let _ = terminal.show_cursor();
        let _ = terminal.backend_mut().flush();

        let goodbye = "\x1b[?25h\x1b[0m\x1b[2J\x1b[H\r\nGoodbye!\r\n";
        let _ = self.output_tx.send(goodbye.as_bytes().to_vec()).await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        info!("Session runner ended for {}", self.session_id);
    }
}
