#!/usr/bin/env bash
# Graba el Tour automático sin ventana (`galaxy --record`) y arma el mp4 de la demostración.
# Uso: scripts/record-demo.sh [CUADROS_SIMULADOS] [CADA_K]
#   CUADROS_SIMULADOS: cuadros de simulación a 60 Hz (por defecto 5400 = 90 s de Tour).
#   CADA_K: se guarda un PNG cada K cuadros (por defecto 2 => 30 PNG por segundo de video).
# Con CADA_K=1 el video es más fluido pero ocupa el doble de disco (cada PNG pesa ~2 MB).
# Variable de entorno: FRAMES_DIR (por defecto `frames`, ignorado por git), VIDEO (salida).
set -euo pipefail
cd "$(dirname "$0")/.."

total="${1:-5400}"
every="${2:-2}"
dir="${FRAMES_DIR:-frames}"
video="${VIDEO:-videos/galaxy-demo.mp4}"

cargo build --release
rm -rf "$dir"
target/release/galaxy --record "$dir" --frames "$total" --every "$every"

# Un PNG cada K cuadros a 60 Hz de simulación equivale a 60/K cuadros por segundo de video real.
FPS_ENTRADA=$((60 / every)) scripts/video.sh "$dir" "$video"
