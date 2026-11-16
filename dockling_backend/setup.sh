#!/bin/bash
set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
python3.10 -m venv .venv
.venv/bin/pip install --upgrade pip
.venv/bin/pip install -r requirements.txt
source .venv/bin/activate
pyinstaller --onefile --name dockling main.py
mv ./dist/dockling ../src-tauri/binaries/dockling-aarch64-apple-darwin
chmod +x ../src-tauri/binaries/dockling-aarch64-apple-darwin
echo "dockling_backend setup complete"
