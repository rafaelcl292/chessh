# Deployment

The production deployment uses an ARM64 Linux executable built on the workstation,
a dedicated `chessh` system account and `packaging/chessh.service`. Build tools and
source code are not installed on the server. Machine-specific inventory and the
release/rollback helper live in `.local/deploy/README.md` and `.local/deploy/deploy.sh`,
which are intentionally excluded from Git.

## Public access

```bash
ssh -p 2222 chess.rafaelcl.com
```

The `chess` A record uses **DNS only** in Cloudflare. Its regular HTTP reverse proxy
cannot carry native SSH. TCP port 2222 must be allowed by both the host firewall and
the Oracle VCN security rules. The host's administrative SSH remains on port 22.
This hostname provides an SSH game, not an HTTPS website.

## Service and data

- Executable: `/usr/local/bin/chessh`
- Unit: `/etc/systemd/system/chessh.service`
- User/group: `chessh`
- Persistent state: `/var/lib/chessh` (mode `0700`)
- Host identity: `/var/lib/chessh/host_key`
- Match archive: `/var/lib/chessh/game_history.jsonl`

The unit limits CPU use to half a CPU and memory to 512 MiB, with a 256 MiB soft
limit. It has no elevated capabilities and cannot access home directories or write
outside its state directory and private temporary directory. These limits isolate
resource use from other applications on the server; they are not a capacity claim.

Back up both state files. Updates and rollbacks replace only the executable and
unit; they preserve the SSH identity and completed games. A graceful restart
archives active matches as interrupted and disconnects their players. Avoid
restarting while games are active when possible.

```bash
sudo systemctl status chessh
sudo journalctl -u chessh -n 100 --no-pager
sudo systemctl restart chessh
```

## Zander engine

The service expects `/usr/local/bin/zander` and the NNUE network at
`/usr/local/share/zander/nn-134a887f4c8f.nnue`. Build Zander locally for ARM64
(`zig build -Doptimize=ReleaseFast -Dnnue-backend=auto -Dtarget=aarch64-linux-musl
-Dcpu=baseline`), then install the executable with mode `0755` and the network
with mode `0644`. Both must be readable by the `chessh` service account. Keep
Zander's license, credits and source revision alongside the network.

The corresponding `CHESSH_ENGINE_PATH` and `CHESSH_ENGINE_EVAL_FILE` variables
are set in the systemd unit. Install these assets before deploying the service.
Each computer game starts a separate engine process inside the service's CPU
and memory limits. Verify an actual engine reply over SSH after deployment,
as the SSH listener check alone does not exercise Zander.

## Release checks

Run formatting, tests and Clippy locally, then cross-compile for the host:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo zigbuild --release --locked --target aarch64-unknown-linux-gnu
```

Use the local deploy helper to upload an exact committed revision. Verify the
binary checksum, systemd health, an SSH login and the lobby from outside the server.
Recheck any co-hosted application's health after a deployment. On failure, restore
the previous executable and unit, leaving the state directory intact.
