#!/usr/bin/env bash
# Convierte el directorio de cuadros de `galaxy --record` en un mp4 (960x540, 60 fps, H.264 yuv420p).
# Uso: scripts/video.sh [DIRECTORIO_DE_CUADROS] [SALIDA.mp4]
#   FPS_ENTRADA (variable de entorno): cuadros por segundo reales que representa la secuencia.
#   Con `--every 1` son 60 (por defecto); con `--every 2` son 30, etc. El mp4 siempre sale a 60 fps.
set -euo pipefail
cd "$(dirname "$0")/.."

frames="${1:-frames}"
out="${2:-videos/galaxy-demo.mp4}"
fps_in="${FPS_ENTRADA:-60}"

if ! command -v ffmpeg >/dev/null 2>&1; then
  cat >&2 <<MSG
ffmpeg no está instalado. Instálalo y vuelve a ejecutar este script:
  Arch / CachyOS:  sudo pacman -S ffmpeg
  Debian / Ubuntu: sudo apt install ffmpeg
  macOS (brew):    brew install ffmpeg
  Windows:         winget install ffmpeg
MSG
  exit 1
fi

if ! ls "$frames"/frame-*.png >/dev/null 2>&1; then
  echo "No hay cuadros en '$frames'. Genéralos con: scripts/record-demo.sh" >&2
  exit 1
fi

mkdir -p "$(dirname "$out")"
ffmpeg -y -loglevel error -framerate "$fps_in" -i "$frames/frame-%06d.png" \
  -vf "scale=960:540:flags=lanczos,fps=60,format=yuv420p" \
  -c:v libx264 -crf 18 -preset medium -movflags +faststart "$out"
echo "$out"
