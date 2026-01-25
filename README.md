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

## Dependencies

- `russh` - SSH server implementation
- `ratatui` + `crossterm` - Terminal UI
- `shakmaty` - Chess logic
- `tokio` - Async runtime

## License

MIT
