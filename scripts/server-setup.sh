#!/usr/bin/env bash
set -euo pipefail

# Hoosh server setup / update script
# Safe to run multiple times — idempotent for all steps.

INVOKING_USER="${SUDO_USER:-$USER}"
INVOKING_HOME=$(getent passwd "$INVOKING_USER" | cut -d: -f6)
BINARY_SRC="${INVOKING_HOME}/.cargo/bin/hoosh"
BINARY_DST="/usr/local/bin/hoosh"
CONFIG_SRC="${INVOKING_HOME}/.config/hoosh"
CONFIG_DST="/etc/hoosh"
SSH_DIR="/var/lib/hoosh/.ssh"
SSH_KEY="${SSH_DIR}/id_ed25519"
ENV_FILE="/etc/hoosh/env"
DATA_DIR="/var/lib/hoosh"
SERVICE_FILE="/etc/systemd/system/hoosh-daemon.service"
SERVICE_NAME="hoosh-daemon"
SERVICE_USER="hoosh"

# ── helpers ──────────────────────────────────────────────────────────────────

BOLD="\033[1m"
CYAN="\033[36m"
YELLOW="\033[33m"
GREEN="\033[32m"
RED="\033[31m"
RESET="\033[0m"

info()    { echo -e "${GREEN}[+]${RESET} $*"; }
warning() { echo -e "${YELLOW}[!]${RESET} $*"; }
die()     { echo -e "${RED}[✗]${RESET} $*" >&2; exit 1; }
action()  { echo -e "\n${BOLD}${CYAN}╔══ ACTION REQUIRED ══╗${RESET}"; echo -e "${CYAN}$*${RESET}"; echo -e "${BOLD}${CYAN}╚═════════════════════╝${RESET}\n"; }

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
    info "User '$SERVICE_USER' already exists — ensuring home dir is $DATA_DIR"
    usermod -d "$DATA_DIR" "$SERVICE_USER"
  else
    info "Creating system user '$SERVICE_USER'"
    useradd --system --no-create-home --home-dir "$DATA_DIR" --shell /usr/sbin/nologin "$SERVICE_USER"
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

setup_ssh_key() {
  mkdir -p "$SSH_DIR"
  chmod 700 "$SSH_DIR"

  if [[ -f "$SSH_KEY" ]]; then
    info "SSH deploy key already exists at $SSH_KEY — skipping generation"
  else
    info "Generating SSH deploy key at $SSH_KEY"
    ssh-keygen -t ed25519 -C "hoosh-daemon" -N "" -f "$SSH_KEY"
    SSH_KEY_GENERATED=true
    action "Add this deploy key to your GitHub repo:\n\n  $(cat "${SSH_KEY}.pub")\n\n  Go to: GitHub repo → Settings → Deploy keys → Add deploy key\n  Enable 'Allow write access'."
  fi

  chown -R "$SERVICE_USER:$SERVICE_USER" "$SSH_DIR"
  chmod 600 "$SSH_KEY"

  info "Adding GitHub to system known_hosts"
  ssh-keyscan github.com 2>/dev/null | tee -a /etc/ssh/ssh_known_hosts > /dev/null

  info "Configuring ssh_key_path in $CONFIG_DST/config.toml"
  if grep -q "ssh_key_path" "$CONFIG_DST/config.toml" 2>/dev/null; then
    sed -i "s|.*ssh_key_path.*|ssh_key_path = \"$SSH_KEY\"|" "$CONFIG_DST/config.toml"
  else
    echo "ssh_key_path = \"$SSH_KEY\"" >> "$CONFIG_DST/config.toml"
  fi
}

setup_env_file() {
  if [[ -f "$ENV_FILE" ]]; then
    info "Env file $ENV_FILE already exists — skipping"
    return
  fi

  echo -e "\n${BOLD}GitHub token${RESET} (repo scope) is needed for PR creation."
  echo -e "Generate one at: https://github.com/settings/tokens"
  read -r -p "  Enter GH_TOKEN (leave blank to skip): " gh_token

  if [[ -n "$gh_token" ]]; then
    printf 'GH_TOKEN=%s\n' "$gh_token" > "$ENV_FILE"
    chown "$SERVICE_USER:$SERVICE_USER" "$ENV_FILE"
    chmod 600 "$ENV_FILE"
    info "GH_TOKEN saved to $ENV_FILE"
  else
    touch "$ENV_FILE"
    chown "$SERVICE_USER:$SERVICE_USER" "$ENV_FILE"
    chmod 600 "$ENV_FILE"
    GH_TOKEN_NEEDED=true
    warning "GH_TOKEN not set — PR creation will not work until you add it to $ENV_FILE"
  fi
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
EnvironmentFile=/etc/hoosh/env
ExecStart=/usr/local/bin/hoosh --config /etc/hoosh/config.toml --data-dir /var/lib/hoosh daemon start --port 7979
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
ProtectSystem=strict
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

SSH_KEY_GENERATED=false
GH_TOKEN_NEEDED=false

require_root
if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
  info "Stopping $SERVICE_NAME"
  systemctl stop "$SERVICE_NAME"
fi
install_binary
create_service_user
sync_config
create_data_dir
setup_ssh_key
setup_env_file
install_service
restart_service

echo ""
info "Done."

if [[ "$SSH_KEY_GENERATED" == true || "$GH_TOKEN_NEEDED" == true ]]; then
  echo ""
  echo -e "${BOLD}${YELLOW}══ Pending manual steps ══${RESET}"
  [[ "$SSH_KEY_GENERATED" == true ]] && echo -e "  ${CYAN}→${RESET} Add deploy key to GitHub repo (printed above) with write access"
  [[ "$GH_TOKEN_NEEDED" == true ]]   && echo -e "  ${CYAN}→${RESET} Set ${BOLD}GH_TOKEN${RESET} in ${BOLD}$ENV_FILE${RESET}, then: ${BOLD}sudo systemctl restart $SERVICE_NAME${RESET}"
  echo ""
fi
