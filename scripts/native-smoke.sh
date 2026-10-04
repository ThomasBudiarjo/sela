#!/usr/bin/env bash
# Requires an existing X11 session/window manager; never manages shared services.
set -euo pipefail
binary=${1:?usage: native-smoke.sh /absolute/path/to/sela [artifact-directory]}
binary=$(realpath "$binary")
: "${DISPLAY:?Set DISPLAY to an existing X11 session}"
for tool in xdotool timeout import convert; do command -v "$tool" >/dev/null; done
scratch=$(mktemp -d /tmp/sela-smoke.XXXXXX)
pid=
cleanup() {
    if [[ -n $pid ]] && kill -0 "$pid" 2>/dev/null; then
        kill -TERM "$pid" 2>/dev/null || true
        for ((i=0; i<20; i++)); do
            kill -0 "$pid" 2>/dev/null || break
            sleep 0.1
        done
        kill -KILL "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    fi
    rm -rf "$scratch"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -m 700 "$scratch/runtime" "$scratch/config" "$scratch/data" "$scratch/cache"
run() { timeout 5s xdotool "$@"; }
alive() { kill -0 "$pid"; run getwindowname "$window" >/dev/null; }
key() {
    run windowactivate --sync "$window"
    [[ $(run getwindowfocus) == "$window" ]]
    # XTEST follows verified native focus; --window uses unreliable XSendEvent.
    run key --clearmodifiers "$1"
}
launch() {
    env XDG_RUNTIME_DIR="$scratch/runtime" XDG_CONFIG_HOME="$scratch/config" \
        XDG_DATA_HOME="$scratch/data" XDG_CACHE_HOME="$scratch/cache" \
        "$binary" >"$scratch/app.log" 2>&1 &
    pid=$!
    window=
    for ((i=0; i<100; i++)); do
        kill -0 "$pid" 2>/dev/null || { cat "$scratch/app.log"; return 1; }
        # PID-scoped search only; never match another worker's window/title.
        window=$(run search --onlyvisible --pid "$pid" 2>/dev/null | head -n 1 || true)
        [[ -z $window ]] || break
        sleep 0.1
    done
    [[ -n $window ]] || { echo 'FAIL: window startup timed out'; return 1; }
    run windowactivate --sync "$window"
    [[ $(run getwindowfocus) == "$window" ]]
}
closed() {
    for ((i=0; i<100; i++)); do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
    done
    if kill -0 "$pid" 2>/dev/null; then echo 'FAIL: process exit timed out'; return 1; fi
    wait "$pid"
    pid=
    if run getwindowname "$window" >/dev/null 2>&1; then
        echo 'FAIL: window survived process exit'; return 1
    fi
}
launch
key ctrl+j
sleep 0.2
alive
run windowsize "$window" 720 440
for ((i=0; i<50; i++)); do
    eval "$(run getwindowgeometry --shell "$window")"
    [[ $WIDTH == 720 && $HEIGHT == 440 ]] && break
    sleep 0.1
done
[[ $WIDTH == 720 && $HEIGHT == 440 ]]
sleep 0.5
# Direct-window import is unreliable on this GL/Xvfb stack; capture root, crop.
timeout 10s import -window root "$scratch/root.png"
timeout 10s convert "$scratch/root.png" -crop "${WIDTH}x${HEIGHT}+${X}+${Y}" +repage "$scratch/operator.png"
if [[ -n ${2:-} ]]; then
    mkdir -p "$2"
    cp "$scratch/operator.png" "$2/m0-03-native-small.png"
    cp "$scratch/app.log" "$2/m0-03-native.log"
fi
key ctrl+q
closed
echo 'PASS: PID-scoped focus, unbound key, resize, Ctrl+Q, clean exit/window removal'
launch
key alt+F4
closed
echo 'PASS: WM close, clean exit/window removal'
