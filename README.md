# CheSSH

Try it now from your terminal:

```bash
ssh -p 2222 chess.rafaelcl.com
```

CheSSH is a multiplayer chess game played entirely over SSH, with a pixel-art
board right in your terminal. All you need is an SSH client — no signup or game
installation required.

Once connected, type `/play` and press **Enter** to find an opponent, or `/solo`
to explore the board and play both sides yourself. Choose **Play computer** (or
`/ai`) to challenge Zander and select its level.

![CheSSH terminal interface with a pixel-art chessboard, move history and clickable game controls](assets/screenshot.png)

*Solo practice shown above. In solo mode, you control both sides.*

## Features

- Connect via SSH and play chess against other players
- Real-time multiplayer with matchmaking
- Play against Zander with selectable engine levels (0–20)
- Beautiful terminal UI with pixel art-inspired colors
- Legal move validation via shakmaty

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run
```

The server listens on port 2222 by default. Connect with:

```bash
ssh -p 2222 localhost
```

For a local-only listener, run `CHESSH_BIND_ADDR=127.0.0.1:2222 cargo run`.

## Playing against Zander

Build [Zander](https://github.com/rafaelcl292/zander#build-and-play) and download
its NNUE network following its README. Then start CheSSH with absolute paths:

```bash
CHESSH_ENGINE_PATH=/path/to/zander/zig-out/bin/zander \
CHESSH_ENGINE_EVAL_FILE=/path/to/zander/networks/nn-134a887f4c8f.nnue \
cargo run
```

Without `CHESSH_ENGINE_PATH`, the server looks for `zander` on `PATH`.
Without `CHESSH_ENGINE_EVAL_FILE`, Zander uses its default network location,
relative to the server's working directory.

In the lobby, select **Play computer**, press **3**, or type `/ai` (also
`/computer`). Use **Left/Right**, **j/k**, or click **−/+** to select level
**0–20**, then press **Enter** or click **Start game**. **Esc** returns to the
menu. Level 0 is the weakest setting; 20 is the strongest. These are Zander's
native `Skill Level` settings, not Elo estimates. You play White.

Each game runs a separate UCI process with one thread, a 16 MiB hash table and
one second of search per move. NNUE and other engine allocations use additional
memory. Search runs asynchronously; leaving, resigning or disconnecting releases
the process. Engine errors return you to the lobby and are logged on the server.
Computer games, like practice boards, are not archived and do not support draw offers.

To run the optional real-engine test, set the two environment variables above and run:

```bash
cargo test --locked real_zander -- --ignored
```

## Deployment

See [deployment instructions](docs/deployment.md) for the systemd service,
persistent data, Cloudflare DNS and release checks.

## Server identity

The server creates an Ed25519 key in `host_key` on its first run and reuses it on
subsequent starts. On Unix it is created with permissions `0600`. Set
`CHESSH_HOST_KEY` to use a different path (its parent directory must exist).
Keep this file across restarts and deployments so SSH clients recognize the server.
An invalid or unreadable existing key stops startup instead of silently replacing
its identity.

## Game history

Multiplayer games are saved to `game_history.jsonl` when they finish, including
player names, result (`1-0`, `0-1`, `1/2-1/2`), termination reason, SAN moves,
final FEN and a Unix timestamp. `/solo` is a local practice board and is not archived.
Set `CHESSH_HISTORY_PATH` to choose another file; its parent directory must exist.

The history is loaded at startup and game IDs continue from the highest saved ID.
Only one server can write a history file at a time. Completed records are synced
to disk; an incomplete final append is recovered on startup, while corrupt complete
records produce an error. Write failures are logged and retained in memory for retry
on the next completed game or graceful shutdown. SIGINT/SIGTERM archives active
games with result `*` and reason `Server shutdown`.

## Playing

Navigate the lobby with **j/k**, **Up/Down**, or **Tab**. Press **l**, **Right**,
or **Enter** to open an option; **h**, **Left**, or **Esc** returns from help or
cancels matchmaking. Number keys **1–5** open menu options directly. Practice mode
lets you control both sides; it does not include an AI opponent.

You can also click menu options. On the board, click a piece to see its legal
moves, then click a destination. Click the selected piece again to deselect it.
Pawn promotion opens a chooser: click a piece or press **q/r/b/n**, with **Esc**
to cancel. Mouse input requires a terminal that forwards SGR mouse events over SSH.

The `/play`, `/solo`, `/ai`, and `/quit` commands remain available from the lobby.
Enter moves in SAN (`Nf3`, `O-O`, `a8=Q`) or UCI (`g1f3`, `e1g1`, `a7a8q`).
During a game, click the action buttons or use **F2** (resign), **F3** (offer or
accept a draw), **F4** (lobby), and **F5** (disconnect). Resignation and draw actions
require confirmation. Leaving an online game also asks for confirmation because
it forfeits the game. Dialogs default to Cancel: use **Tab** then **Enter**, **Y**,
or click Confirm to proceed; **Esc** cancels. Slash commands use the same dialogs.
Draw offers are disabled in practice mode.

After checkmate, a draw, resignation or disconnection, the final board stays visible
with the result and move history. Moves are disabled so you can inspect the position
for as long as you like. Press **Enter** to return to the lobby and start another
game, or **Q** to disconnect.

## Tests

```bash
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

The suite covers move encoding, promotion, results for both colors, final-board
rendering, rematches, disconnects, persistent history and host keys. The SSH
integration test opens two real loopback SSH connections on an ephemeral port and
plays through checkmate, returning to the lobby, a draw and a disconnection.

## Dependencies

- `russh` - SSH server implementation
- `ratatui` + `crossterm` - Terminal UI
- `shakmaty` - Chess logic
- `tokio` - Async runtime

## License

MIT
