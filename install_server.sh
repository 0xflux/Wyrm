#!/bin/bash

set -euo pipefail

cd "$(dirname "$0")"

SCRIPT_DIR=$(pwd)
C2_DIR="${SCRIPT_DIR}/c2"

echo
echo "This installation will use the credentials supplied in ./c2/.env"
echo "If you want to change them, quit this installer and edit the file first."
echo -n "Otherwise, press 'y' to continue with the installation: "

read -r proceed
if [[ "$proceed" != "y" && "$proceed" != "Y" ]]; then
    echo "[-] Installation aborted."
    exit 1
fi


#
# Load environment variables from c2/.env
#
if [[ ! -f "${C2_DIR}/.env" ]]; then
  echo "[-] Missing ${C2_DIR}/.env"
  exit 1
fi

set -o allexport
source ./c2/.env
set +o allexport


#
# Install OS deps
#
sudo apt update -qq
sudo apt install -qq -y postgresql postgresql-contrib build-essential \
    pkg-config libssl-dev gcc-mingw-w64-x86-64 \
    g++-mingw-w64-x86-64 curl libgtk-3-dev clang


#
# Install Rust deps
#
if ! command -v rustup &>/dev/null; then
  echo "[i] Installing rustup + stable toolchain…"
  curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable
  echo "Adding cargo to .bashrc path. If you use a different shell please change this manually."
  echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> "$HOME/.bashrc"
  export PATH="$HOME/.cargo/bin:$PATH"
  source "$HOME/.cargo/env"
else
  echo "[i] rustup already installed"
fi

[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
export PATH="$HOME/.cargo/bin:$PATH"

# Check up to date
rustup self update

# Install nightly
if ! rustup toolchain list | grep -q '^nightly'; then
  echo "[i] Installing nightly toolchain…"
  rustup toolchain install nightly
fi

# Install Windows targets (using xwin)
echo "[i] Adding x86_64-pc-windows-gnu target to stable & nightly…"
rustup component add llvm-tools
rustup target add x86_64-pc-windows-msvc
rustup target add x86_64-pc-windows-msvc --toolchain stable
rustup target add x86_64-pc-windows-msvc --toolchain nightly
cargo install cargo-xwin

# Set nightly
rustup override set nightly


#
# Setup Postgres db
#
sudo systemctl start postgresql

sudo -u postgres -H psql <<EOF
ALTER USER $POSTGRES_USER PASSWORD '$POSTGRES_PASSWORD';
CREATE DATABASE $POSTGRES_DB;
EOF


#
# Build the C2
#
echo "[→] Building C2"
pushd "${C2_DIR}" >/dev/null
  cargo build --release
popd >/dev/null


# Prefer the user who invoked sudo, fallback to current user
DEPLOY_USER="${SUDO_USER:-$(id -un)}"
DEPLOY_GROUP="$(id -gn "$DEPLOY_USER")"

#
# Create a service to run the C2
#
SERVICE_PATH=/etc/systemd/system/wyrm.service

echo "[i] Writing systemd unit to ${SERVICE_PATH}..."

sudo tee "${SERVICE_PATH}" > /dev/null <<EOF
[Unit]
Description=Wyrm C2 Service
After=network.target postgresql.service

[Service]
User=${DEPLOY_USER}
Group=${DEPLOY_GROUP}
WorkingDirectory=${C2_DIR}
EnvironmentFile=${C2_DIR}/.env

# Ensure cargo/rustup are visible to the service
Environment=PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/home/${DEPLOY_USER}/.cargo/bin
Environment=CARGO_HOME=/home/${DEPLOY_USER}/.cargo
Environment=RUSTUP_HOME=/home/${DEPLOY_USER}/.rustup

ExecStart=${SCRIPT_DIR}/target/release/c2
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

echo "[i] Reloading systemd & enabling service..."
sudo systemctl daemon-reload
sudo systemctl enable wyrm
sudo systemctl restart wyrm