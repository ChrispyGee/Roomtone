#!/usr/bin/env bash
# Tries the three kinds of dragging in the real window on the demo system, with a simulated mouse,
# and says whether each worked. It needs a built program (cargo build) and no speakers.
#   scripts/check-drag.sh
set -euo pipefail
cd "$(dirname "$0")/.."
bin=target/debug/roomtone
[ -x "$bin" ] || cargo build
home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
song="$home/Dropped Song.wav"
python3 - "$song" <<'PY'
import struct, sys
n = 8000
wav = b"RIFF" + struct.pack("<I", 36 + n * 2) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 1, 8000, 16000, 2, 16) + b"data" + struct.pack("<I", n * 2) + bytes(n * 2)
open(sys.argv[1], "wb").write(wav)
PY
fail=0
echo "A room card dragged onto another:"
out="$(HOME="$home" ROOMTONE_TEST_DRAG=room ROOMTONE_SHOT_FRAME=80 "$bin" --view home --screenshot "$home/shot.png" 2>&1 | grep DRAG-TEST || true)"; echo "$out"
grep -q "frame 70.*joined=true" <<<"$out" || { echo "FAILED: the rooms were not joined"; fail=1; }
echo "A queue row dragged down two places:"
out="$(HOME="$home" ROOMTONE_TEST_QUEUE=2 ROOMTONE_TEST_DRAG=queue ROOMTONE_SHOT_FRAME=80 "$bin" --view home --screenshot "$home/shot.png" 2>&1 | grep DRAG-TEST || true)"; echo "$out"
grep -q 'frame 70.*first four=\["Slow Motion Summer", "Highway Glow", "Neon Coastline"' <<<"$out" || { echo "FAILED: the queue row did not move"; fail=1; }
echo "A song file dropped on the window:"
out="$(HOME="$home" ROOMTONE_TEST_DRAG="files:$song" ROOMTONE_SHOT_FRAME=100 "$bin" --view home --screenshot "$home/shot.png" 2>&1 | grep DRAG-TEST || true)"; echo "$out"
grep -q "Dropped Song" <<<"$out" || { echo "FAILED: the song was not added"; fail=1; }
[ "$fail" = 0 ] && echo "All three worked." || exit 1
