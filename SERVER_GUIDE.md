# Hoosh Daemon — Server Setup Guide

## Overview

This guide documents how to deploy the Hoosh daemon on a Linux server (Debian 13+),
running as a locked-down system user behind a **Cloudflare Tunnel**, managed by systemd.

The setup assumes you are using **Cloudflare** for DNS and tunneling — the daemon is not
exposed directly to the internet. All traffic reaches it through a Cloudflare Tunnel,
which handles TLS termination and access control.

> **TL;DR:** For initial setup or updates, run the setup script (steps 1–4 automated):
> ```bash
> sudo bash scripts/server-setup.sh
> ```
> Then follow steps 5+ for Cloudflare tunnel setup.

---

## Prerequisites

- Debian 13 (Trixie) or similar systemd-based distro
- `cloudflared` installed and tunnel configured
- Hoosh binary at `/usr/local/bin/hoosh`
- A Cloudflare account with a domain (e.g. `manije.io`)

---

## 1. Install the Binary

The Hoosh installer puts the binary in `~/.cargo/bin/hoosh`. Copy it to a system-wide location:

```bash
sudo cp ~/.cargo/bin/hoosh /usr/local/bin/hoosh
sudo chown root:root /usr/local/bin/hoosh
sudo chmod 755 /usr/local/bin/hoosh
```

> **Note:** Rust compiles to a fully static binary — no runtime dependencies, no need to reinstall from source.

---

## 2. Create a Dedicated Service User

```bash
sudo useradd --system --no-create-home --shell /usr/sbin/nologin hoosh
```

This user cannot log in interactively, cannot sudo, and cannot escalate beyond its assigned permissions.

---

## 3. Create Config and Data Directories

```bash
sudo mkdir -p /etc/hoosh
sudo cp -r ~/.config/hoosh/. /etc/hoosh/
sudo chown -R hoosh:hoosh /etc/hoosh
sudo chmod 600 /etc/hoosh/config.toml

sudo mkdir -p /var/lib/hoosh
sudo chown -R hoosh:hoosh /var/lib/hoosh
```

> **Gotcha:** Hoosh defaults to `~/.local/share/hoosh/daemon/tasks` for the task store.
> Since the service user has no home directory, you MUST pass `--data-dir` explicitly.
> Without it, the daemon crashes with `Permission denied (os error 13)`.

> **Gotcha:** Hoosh checks config file permissions and will warn (but not fail) if
> permissions are not exactly `0600`. Always set `chmod 600`.

---

## 4. Create the systemd Service

```bash
sudo tee /etc/systemd/system/hoosh-daemon.service > /dev/null << 'EOF'
[Unit]
Description=Hoosh Daemon
After=network.target

[Service]
Type=simple
User=hoosh
Group=hoosh
ExecStart=/usr/local/bin/hoosh --config /etc/hoosh/config.toml --data-dir /var/lib/hoosh daemon start --port 7979
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/hoosh

[Install]
WantedBy=multi-user.target
EOF
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable hoosh-daemon
sudo systemctl start hoosh-daemon
sudo systemctl status hoosh-daemon
```

> **Gotcha:** `ProtectSystem=strict` makes the filesystem read-only for the process.
> You must explicitly add `ReadWritePaths=/var/lib/hoosh` or the daemon will crash
> with `Read-only file system (os error 30)` when trying to create the tasks directory.

---

## 5. Cloudflare Tunnel Setup

Install cloudflared:

```bash
curl -L https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64 -o cloudflared
chmod +x cloudflared
sudo mv cloudflared /usr/local/bin/
```

Authenticate and create tunnel:

```bash
cloudflared tunnel login         # opens browser URL — authorize on another machine if headless
cloudflared tunnel create hoosh
cloudflared tunnel route dns hoosh hoosh.manije.io
```

Look up your tunnel ID (you'll need it in the next step):

```bash
cloudflared tunnel list
```

Create the tunnel config (replace `YOUR_TUNNEL_ID` with the ID from above):

```bash
TUNNEL_ID=YOUR_TUNNEL_ID
CREDS_FILE=$(echo ~/.cloudflared/${TUNNEL_ID}.json)

sudo mkdir -p /etc/cloudflared
sudo tee /etc/cloudflared/config.yml > /dev/null << EOF
tunnel: ${TUNNEL_ID}
credentials-file: ${CREDS_FILE}

ingress:
  - hostname: hoosh.manije.io
    service: http://localhost:7979
  - service: http_status:404
EOF
```

Install and start as systemd service:

```bash
sudo cloudflared --config /etc/cloudflared/config.yml service install
sudo systemctl enable cloudflared
sudo systemctl start cloudflared
```

> **Gotcha:** `sudo cloudflared service install` without `--config` fails with
> "Cannot determine default configuration path" because sudo loses the home directory
> context. Always pass the full path explicitly.

> **Gotcha:** `service install` copies your config to `/etc/cloudflared/config.yml`.
> Edit there after installation, not in `~/.cloudflared/`.

---

## 6. GitHub Webhook (Planned)

Add a second ingress route in `/etc/cloudflared/config.yml`:

```yaml
ingress:
  - hostname: hoosh.manije.io
    service: http://localhost:7979
  - hostname: webhook.manije.io
    service: http://localhost:7979
  - service: http_status:404
```

Then configure in `/etc/hoosh/config.toml`:

```toml
[github]
webhook_secret = "your-secret-here"
bot_login = "your-github-bot-username"
```

> **Note:** Without `webhook_secret`, the webhook endpoint returns 500.
> Without `bot_login`, self-trigger protection is disabled (daemon may respond
> to its own commits in a loop).

---

## 7. Useful Commands

```bash
# Check daemon status
sudo systemctl status hoosh-daemon

# View logs
sudo journalctl -u hoosh-daemon -f

# Restart after config change
sudo systemctl restart hoosh-daemon

# Check tunnel status
sudo systemctl status cloudflared
sudo journalctl -u cloudflared -f

# Verify tunnel is reachable locally
curl http://localhost:7979
```

---

## File Layout Summary

| Path | Purpose |
|------|---------|
| `/usr/local/bin/hoosh` | Hoosh binary |
| `/etc/hoosh/config.toml` | Hoosh config (chmod 600, owned by hoosh) |
| `/var/lib/hoosh/` | Daemon runtime data (task store, sessions) |
| `/etc/systemd/system/hoosh-daemon.service` | systemd unit |
| `/etc/cloudflared/config.yml` | Cloudflare tunnel config |
| `~/.cloudflared/*.json` | Tunnel credentials |

---

## Known Issues Summary

| Issue | Cause | Fix |
|-------|-------|-----|
| `Backend 'mock' not found` | Config using mock backend | Set real backend + API key in config.toml |
| `Permission denied` on task store | No `--data-dir`, service user has no home | Pass `--data-dir /var/lib/hoosh` |
| `Read-only file system` | `ProtectSystem=strict` | Add `ReadWritePaths=/var/lib/hoosh` to service |
| `Cannot determine default config path` | sudo loses home context | Pass `--config /full/path/config.yml` explicitly |
| Config permission warning | File is 640 not 600 | `chmod 600 /etc/hoosh/config.toml` |
| Webhook returns 500 | `webhook_secret` not set | Configure `[github]` section in config.toml |
