# CheSSH

An interactive SSH server for playing chess via the terminal.

## Demo

Watch the [demo video](https://youtu.be/ICkVy5-rHRw) to see CheSSH in action!

## Features

- Connect via SSH and play chess against other players
- Real-time multiplayer with matchmaking
- Beautiful terminal UI with pixel art-inspired colors
- Full chess rules validation via shakmaty

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

## Playing

Use `/play` to join matchmaking or `/solo` to play both sides locally. Enter moves
in SAN (`Nf3`, `O-O`, `a8=Q`) or UCI (`g1f3`, `e1g1`, `a7a8q`).
During a multiplayer game, `/resign` concedes and `/draw` offers or accepts a draw.

After checkmate, a draw, resignation or disconnection, the final board stays visible
with the result and move history. Moves are disabled so you can inspect the position
for as long as you like. Press **Enter** to return to the lobby and start another
game, or **Q** to disconnect.

## Dependencies

- `russh` - SSH server implementation
- `ratatui` + `crossterm` - Terminal UI
- `shakmaty` - Chess logic
- `tokio` - Async runtime

## License

MIT
