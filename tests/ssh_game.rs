use std::sync::Arc;
use std::time::Duration;

use chessh::server::SessionManager;
use chessh::ssh::SshServer;
use russh::client;
use russh::keys::{Algorithm, PrivateKey, PublicKeyOrCertificate};
use russh::server::Server;
use russh::{Channel, ChannelMsg};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

struct TestClient;
impl client::Handler for TestClient {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

async fn connect(
    address: std::net::SocketAddr,
    name: &str,
) -> (client::Handle<TestClient>, Channel<client::Msg>) {
    let mut handle = client::connect(Arc::new(client::Config::default()), address, TestClient)
        .await
        .unwrap();
    assert!(handle.authenticate_none(name).await.unwrap().success());
    let mut channel = handle.channel_open_session().await.unwrap();
    channel
        .request_pty(true, "xterm-256color", 100, 30, 0, 0, &[])
        .await
        .unwrap();
    channel.request_shell(true).await.unwrap();
    read_until(&mut channel, "Lobby").await;
    (handle, channel)
}

async fn read_until(channel: &mut Channel<client::Msg>, expected: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut data = Vec::new();
        loop {
            match channel.wait().await.expect("SSH channel closed") {
                ChannelMsg::Data { data: bytes } => data.extend_from_slice(&bytes),
                _ => continue,
            }
            if String::from_utf8_lossy(&data).contains(expected) {
                return;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("did not render {expected:?}"));
}

async fn type_text(channel: &Channel<client::Msg>, text: &str) {
    // Individual SSH data messages emulate typed keys.
    for byte in text.bytes() {
        channel.data(&[byte][..]).await.unwrap();
    }
}

async fn wait_for_match(manager: &Arc<RwLock<SessionManager>>, after: u64) -> (u64, bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            {
                let manager = manager.read().await;
                if let Some(alice) = manager.find_by_username("alice") {
                    if let Some(id) = manager.get_player_game_id(alice.id) {
                        if id > after {
                            return (id, manager.get_match_info(id).unwrap().white_id == alice.id);
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn ssh_players_can_finish_review_rematch_draw_and_disconnect() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let manager = Arc::new(RwLock::new(SessionManager::new()));
        let mut server = SshServer::new(manager.clone());
        let config = Arc::new(russh::server::Config {
            keys: vec![PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap()],
            ..Default::default()
        });
        let server_task = tokio::spawn(async move {
            server.run_on_socket(config, &listener).await.unwrap();
        });
        let (alice_handle, mut alice) = connect(address, "alice").await;
        let (bob_handle, mut bob) = connect(address, "bob").await;
        type_text(&alice, "\r").await;
        read_until(&mut alice, "Searching for opponent").await;
        type_text(&alice, "\x1b").await;
        tokio::time::timeout(Duration::from_secs(5), async {
            while manager.read().await.queue_size() != 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        type_text(&alice, "\r").await;
        type_text(&bob, "\r").await;
        let (game_id, alice_white) = wait_for_match(&manager, 0).await;
        read_until(&mut alice, "Moves").await;
        read_until(&mut bob, "Moves").await;
        let (white, black) = if alice_white {
            (&mut alice, &mut bob)
        } else {
            (&mut bob, &mut alice)
        };
        for (index, (is_white, mv)) in [
            (true, "f3\r"),
            (false, "e5\r"),
            (true, "g4\r"),
            (false, "Qh4#\r"),
        ]
        .iter()
        .enumerate()
        {
            if index == 0 {
                let area =
                    chessh::ui::GameView::board_area(ratatui::layout::Rect::new(0, 0, 100, 30));
                for square in [shakmaty::Square::F2, shakmaty::Square::F3] {
                    let (x, y) = (area.y..area.bottom())
                        .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
                        .find(|(x, y)| {
                            chessh::ui::BoardWidget::square_at(area, *x, *y, false) == Some(square)
                        })
                        .unwrap();
                    // Deliberately split a mouse report across SSH packets.
                    white.data(b"\x1b[<0;".as_slice()).await.unwrap();
                    white
                        .data(format!("{};{}M", x + 1, y + 1).as_bytes())
                        .await
                        .unwrap();
                }
            } else {
                type_text(if *is_white { &*white } else { &*black }, mv).await;
            }
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let guard = manager.read().await;
                    let moved = guard
                        .get_game(game_id)
                        .is_none_or(|g| g.game.move_count() > index);
                    drop(guard);
                    if moved {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
        }
        read_until(white, "YOU LOSE").await;
        read_until(black, "YOU WIN").await;
        {
            let guard = manager.read().await;
            assert!(guard.get_game(game_id).is_none());
            let record = &guard.history().get_records()[0];
            assert_eq!(record.result, "0-1");
            assert_eq!(record.moves.last().unwrap(), "Qh4#");
        }
        // Final boards remain until both players explicitly press Enter.
        type_text(&alice, "\r").await;
        type_text(&bob, "\r").await;
        read_until(&mut alice, "Lobby").await;
        read_until(&mut bob, "Lobby").await;
        type_text(&alice, "/play\r").await;
        type_text(&bob, "/play\r").await;
        let (next, _) = wait_for_match(&manager, game_id).await;
        read_until(&mut alice, "Moves").await;
        read_until(&mut bob, "Moves").await;
        type_text(&alice, "/draw\ry").await;
        read_until(&mut bob, "offers a draw").await;
        type_text(&bob, "/draw\ry").await;
        read_until(&mut alice, "DRAW").await;
        read_until(&mut bob, "DRAW").await;
        assert!(manager.read().await.get_game(next).is_none());
        type_text(&alice, "\r").await;
        type_text(&bob, "\r").await;
        read_until(&mut alice, "Lobby").await;
        read_until(&mut bob, "Lobby").await;
        type_text(&alice, "/play\r").await;
        type_text(&bob, "/play\r").await;
        let (third, _) = wait_for_match(&manager, next).await;
        read_until(&mut alice, "Moves").await;
        read_until(&mut bob, "Moves").await;
        alice_handle
            .disconnect(russh::Disconnect::ByApplication, "test", "en")
            .await
            .unwrap();
        read_until(&mut bob, "YOU WIN").await;
        assert!(manager.read().await.get_game(third).is_none());
        assert_eq!(manager.read().await.history().get_records().len(), 3);
        type_text(&bob, "q").await;
        read_until(&mut bob, "Goodbye!").await;
        drop(bob_handle);
        server_task.abort();
    })
    .await
    .expect("SSH game flow timed out");
}
