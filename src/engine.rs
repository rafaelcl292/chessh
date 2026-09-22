//! A bounded UCI worker. Dropping its owner aborts search and kills the subprocess.
use crate::chess::Game;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::mpsc,
    task::JoinHandle,
};

pub struct EngineWorker {
    pub positions: mpsc::Sender<Game>,
    pub replies: mpsc::Receiver<Result<Option<String>, String>>,
    task: JoinHandle<()>,
}
impl Drop for EngineWorker {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl EngineWorker {
    pub fn start(level: u8) -> Self {
        let path = std::env::var_os("CHESSH_ENGINE_PATH").unwrap_or_else(|| "zander".into());
        let network = std::env::var("CHESSH_ENGINE_EVAL_FILE").ok();
        Self::spawn(path, network, level)
    }
    fn spawn(path: std::ffi::OsString, network: Option<String>, level: u8) -> Self {
        let (positions, mut requests) = mpsc::channel::<Game>(1);
        let (tx, replies) = mpsc::channel(1);
        let task = tokio::spawn(async move {
            let result: Result<(), String> = async {
                let mut child = Command::new(path).stdin(Stdio::piped()).stdout(Stdio::piped())
                    .stderr(Stdio::null()).kill_on_drop(true).spawn().map_err(|e| e.to_string())?;
                let mut stdin = child.stdin.take().unwrap();
                let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
                async fn send(input: &mut tokio::process::ChildStdin, text: &str) -> Result<(), String> {
                    input.write_all(text.as_bytes()).await.map_err(|e| e.to_string())
                }
                async fn until(lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>, prefix: &str) -> Result<String, String> {
                    tokio::time::timeout(Duration::from_secs(15), async {
                        while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
                            if line.starts_with("info string error ") { return Err(line); }
                            if line == prefix || line.starts_with(&format!("{prefix} ")) { return Ok(line); }
                        }
                        Err("Engine closed its output".into())
                    }).await.map_err(|_| "Engine timed out".to_string())?
                }
                send(&mut stdin, "uci\n").await?;
                until(&mut lines, "uciok").await?;
                send(&mut stdin, &format!("setoption name Threads value 1\nsetoption name Hash value 16\nsetoption name UCI_LimitStrength value false\nsetoption name Skill Level value {}\n", level.min(20))).await?;
                if let Some(network) = network {
                    if network.contains(['\n', '\r']) { return Err("Invalid EvalFile path".into()); }
                    send(&mut stdin, &format!("setoption name EvalFile value {network}\n")).await?;
                }
                send(&mut stdin, "ucinewgame\nisready\n").await?;
                until(&mut lines, "readyok").await?;
                tx.send(Ok(None)).await.map_err(|e| e.to_string())?;
                while let Some(game) = requests.recv().await {
                    let moves = game.move_history().iter().map(|mv| mv.to_uci(shakmaty::CastlingMode::Standard).to_string()).collect::<Vec<_>>().join(" ");
                    send(&mut stdin, &format!("position startpos moves {moves}\ngo movetime 1000\n")).await?;
                    let reply = until(&mut lines, "bestmove").await?;
                    let mv = reply.split_whitespace().nth(1).ok_or("Missing bestmove")?.to_string();
                    tx.send(Ok(Some(mv))).await.map_err(|e| e.to_string())?;
                }
                Ok(())
            }.await;
            if let Err(error) = result {
                let _ = tx.send(Err(error)).await;
            }
        });
        Self {
            positions,
            replies,
            task,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_executable_reports_error() {
        let mut worker = EngineWorker::spawn("/nonexistent/chessh-engine".into(), None, 5);
        assert!(worker.replies.recv().await.unwrap().is_err());
    }

    #[tokio::test]
    #[ignore = "requires CHESSH_ENGINE_PATH and CHESSH_ENGINE_EVAL_FILE"]
    async fn real_zander_plays_at_both_level_limits() {
        for level in [0, 20] {
            let mut worker = EngineWorker::start(level);
            assert_eq!(worker.replies.recv().await.unwrap().unwrap(), None);
            let mut game = Game::new();
            for white in ["e2e4", "g1f3"] {
                game.play_uci(white).unwrap();
                worker.positions.send(game.clone()).await.unwrap();
                let reply = worker.replies.recv().await.unwrap().unwrap().unwrap();
                game.play_uci(&reply).unwrap();
            }
            assert_eq!(game.move_count(), 4);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn protocol_sets_level_and_preserves_move_history() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("chessh-uci-{}", std::process::id()));
        std::fs::write(
            &path,
            r#"#!/bin/sh
level=0
while IFS= read -r command; do
  case "$command" in
    uci) echo uciok ;;
    'setoption name Skill Level value 7') level=7 ;;
    isready) echo readyok ;;
    'position startpos moves e2e4') position=yes ;;
    'go movetime 1000')
      if [ "$level" = 7 ] && [ "$position" = yes ]; then
        echo 'info depth 1 score cp 0'
        echo 'bestmove e7e5 ponder g1f3'
      else
        echo 'info string error incorrect protocol'
      fi ;;
  esac
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut worker = EngineWorker::spawn(path.clone().into_os_string(), None, 7);
        assert_eq!(worker.replies.recv().await.unwrap().unwrap(), None);
        let mut game = Game::new();
        game.play_uci("e2e4").unwrap();
        worker.positions.send(game).await.unwrap();
        assert_eq!(
            worker.replies.recv().await.unwrap().unwrap().as_deref(),
            Some("e7e5")
        );
        drop(worker);
        std::fs::remove_file(path).unwrap();
    }
}
