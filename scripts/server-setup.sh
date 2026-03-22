#!/usr/bin/env bash
set -euo pipefail

# Hoosh server setup / update script
# Safe to run multiple times — idempotent for all steps.

BINARY_SRC="${HOME}/.cargo/bin/hoosh"
BINARY_DST="/usr/local/bin/hoosh"
CONFIG_SRC="${HOME}/.config/hoosh"
CONFIG_DST="/etc/hoosh"
DATA_DIR="/var/lib/hoosh"
SERVICE_FILE="/etc/systemd/system/hoosh-daemon.service"
SERVICE_NAME="hoosh-daemon"
SERVICE_USER="hoosh"

# ── helpers ──────────────────────────────────────────────────────────────────

info()    { echo "[+] $*"; }
warning() { echo "[!] $*"; }
die()     { echo "[✗] $*" >&2; exit 1; }

require_root() {
  [[ $EUID -eq 0 ]] || die "Run this script with sudo: sudo $0"
}

# ── steps ────────────────────────────────────────────────────────────────────

install_binary() {
  [[ -f "$BINARY_SRC" ]] || die "Hoosh binary not found at $BINARY_SRC. Run 'cargo install' first."

  local src_ver dst_ver
  src_ver=$("$BINARY_SRC" --version 2>/dev/null || echo "unknown")

  if [[ -f "$BINARY_DST" ]]; then
    dst_ver=$("$BINARY_DST" --version 2>/dev/null || echo "unknown")
    if [[ "$src_ver" == "$dst_ver" ]]; then
      info "Binary already up to date ($dst_ver) — skipping"
      return
    fi
    info "Updating binary: $dst_ver → $src_ver"
  else
    info "Installing binary ($src_ver)"
  fi

  cp "$BINARY_SRC" "$BINARY_DST"
  chown root:root "$BINARY_DST"
  chmod 755 "$BINARY_DST"
}

create_service_user() {
  if id "$SERVICE_USER" &>/dev/null; then
    info "User '$SERVICE_USER' already exists — skipping"
  else
    info "Creating system user '$SERVICE_USER'"
    useradd --system --no-create-home --shell /usr/sbin/nologin "$SERVICE_USER"
  fi
}

sync_config() {
  [[ -d "$CONFIG_SRC" ]] || die "Config directory not found at $CONFIG_SRC"

  info "Syncing config from $CONFIG_SRC to $CONFIG_DST"
  mkdir -p "$CONFIG_DST"
  cp -r "$CONFIG_SRC/." "$CONFIG_DST/"
  chown -R "$SERVICE_USER:$SERVICE_USER" "$CONFIG_DST"
  chmod 600 "$CONFIG_DST/config.toml"
}

create_data_dir() {
  if [[ -d "$DATA_DIR" ]]; then
    info "Data directory $DATA_DIR already exists — skipping"
  else
    info "Creating data directory $DATA_DIR"
    mkdir -p "$DATA_DIR"
  fi
  chown -R "$SERVICE_USER:$SERVICE_USER" "$DATA_DIR"
}

install_service() {
  info "Writing systemd service $SERVICE_FILE"
  cat > "$SERVICE_FILE" << 'EOF'
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

  systemctl daemon-reload
  systemctl enable "$SERVICE_NAME"
}

restart_service() {
  if systemctl is-active --quiet "$SERVICE_NAME"; then
    info "Restarting $SERVICE_NAME"
    systemctl restart "$SERVICE_NAME"
  else
    info "Starting $SERVICE_NAME"
    systemctl start "$SERVICE_NAME"
  fi
  systemctl status "$SERVICE_NAME" --no-pager
}

# ── main ─────────────────────────────────────────────────────────────────────

require_root
install_binary
create_service_user
sync_config
create_data_dir
install_service
restart_service

info "Done."
