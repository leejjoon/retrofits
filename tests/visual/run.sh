#!/usr/bin/env bash
# Visual regression test for terminal graphics protocols (Kitty / Sixel).
#
# Runs REAL terminal emulators (kitty, konsole) headlessly under Xvfb,
# drives retrofits through every panel (summary, extension picker, manual
# cut, protocol picker, help, header viewer), screenshots the actual
# framebuffer, and pixel-diffs each after-close frame against a baseline.
#
# This is the regression test for the non-overlapping panel layout: closing
# a panel must restore the screen exactly, with no graphics-protocol
# artifacts and no forced screen clears (see DEVELOP.md).
#
# Usage:      tests/visual/run.sh
# Requires:   Xvfb, ImageMagick (import + compare), and kitty and/or
#             konsole (missing terminals are skipped).
# Artifacts:  tests/visual/output/ (screenshots; kept on failure)
# Exit code:  0 = all available protocols pass, 1 = artifact detected,
#             2 = missing hard dependency.

set -u

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/../.." && pwd)
BIN="$REPO_ROOT/target/release/retrofits"
FITS="${RETROFITS_TEST_FILE:-$REPO_ROOT/example_fits/18109J000.fits}"
OUT="$SCRIPT_DIR/output"
DISPLAY_NUM=""
XVFB_PID=""
TERM_PID=""
FAILURES=0

# Panels: open key -> label. All panels close with 'q'.
PANEL_KEYS=(w e m P '?' v)
PANEL_NAMES=(summary extension cut protocol help header)

log()  { printf '%s\n' "$*"; }
fail() { log "FAIL: $*"; FAILURES=$((FAILURES + 1)); }

cleanup() {
    [ -n "$TERM_PID" ] && kill "$TERM_PID" 2>/dev/null
    [ -n "$XVFB_PID" ] && kill "$XVFB_PID" 2>/dev/null
    wait 2>/dev/null
}
trap cleanup EXIT INT TERM

# --- dependency checks ----------------------------------------------------
for cmd in Xvfb import compare; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        log "missing hard dependency: $cmd (install Xvfb / ImageMagick)"
        exit 2
    fi
done
if [ ! -x "$BIN" ]; then
    log "building release binary..."
    (cd "$REPO_ROOT" && cargo build --release) || exit 2
fi
[ -f "$FITS" ] || { log "test FITS file not found: $FITS"; exit 2; }

mkdir -p "$OUT"
rm -f "$OUT"/*.png

# --- start Xvfb on a free display ------------------------------------------
for n in 99 100 101 102 103; do
    if [ ! -e "/tmp/.X${n}-lock" ]; then
        Xvfb ":$n" -screen 0 1280x800x24 2>/dev/null &
        XVFB_PID=$!
        DISPLAY_NUM=":$n"
        break
    fi
done
[ -n "$DISPLAY_NUM" ] || { log "no free X display found"; exit 2; }
sleep 1
log "Xvfb running on $DISPLAY_NUM"

shot() { # shot <name>
    DISPLAY=$DISPLAY_NUM import -window root "$OUT/$1.png" 2>/dev/null
}

diff_ae() { # diff_ae <a> <b> -> prints number of differing pixels
    compare -metric AE "$OUT/$1.png" "$OUT/$2.png" null: 2>&1 | awk '{print $1}'
}

# check_panels <prefix> <send_fn>
# Cycles every panel: open, screenshot, close with q, screenshot, then
# pixel-diffs each after-close frame against the baseline.
check_panels() {
    local prefix=$1 send=$2 i key name opened

    # Normalize the screen first (R force-clears; absorbs any startup
    # scroll quirks, e.g. konsole+sixel shifting the first frame by a row).
    "$send" "R"; sleep 2
    shot "${prefix}_baseline"

    for i in "${!PANEL_KEYS[@]}"; do
        key=${PANEL_KEYS[$i]}
        name=${PANEL_NAMES[$i]}
        "$send" "$key"; sleep 1.5
        shot "${prefix}_${name}_open"
        "$send" "q"; sleep 1.5
        shot "${prefix}_${name}_closed"

        # Positive control: the panel must actually have opened, otherwise
        # a 0-diff result would be vacuous.
        opened=$(diff_ae "${prefix}_baseline" "${prefix}_${name}_open")
        if [ "${opened:-0}" -eq 0 ] 2>/dev/null; then
            fail "$prefix/$name: panel did not open (key '$key' not received?)"
            continue
        fi

        local d
        d=$(diff_ae "${prefix}_baseline" "${prefix}_${name}_closed")
        if [ "${d:-1}" -eq 0 ] 2>/dev/null; then
            log "PASS: $prefix/$name (open changed ${opened}px, close restored exactly)"
        else
            fail "$prefix/$name: $d pixels differ after close (see $OUT/${prefix}_${name}_closed.png)"
        fi
    done
}

# --- kitty (Kitty graphics protocol) ---------------------------------------
KITTY_SOCK="unix:@retrofits-vistest-$$"

kitty_send() {
    if [ "$1" = "R" ] || [ "$1" = "q" ] || [ "${#1}" -eq 1 ]; then
        DISPLAY=$DISPLAY_NUM kitten @ --to "$KITTY_SOCK" send-text -- "$1"
    fi
}

if command -v kitty >/dev/null 2>&1 && command -v kitten >/dev/null 2>&1; then
    log ""
    log "=== kitty (Kitty graphics protocol) ==="
    env -u WAYLAND_DISPLAY DISPLAY=$DISPLAY_NUM kitty \
        -o allow_remote_control=yes \
        -o linux_display_server=x11 \
        -o remember_window_size=no \
        -o initial_window_width=1260 -o initial_window_height=780 \
        -o font_size=11 \
        --listen-on "$KITTY_SOCK" \
        "$BIN" --protocol kitty "$FITS" 2>/dev/null &
    TERM_PID=$!
    sleep 5
    if kill -0 "$TERM_PID" 2>/dev/null; then
        check_panels kitty kitty_send
    else
        fail "kitty failed to start"
    fi
    kill "$TERM_PID" 2>/dev/null; wait "$TERM_PID" 2>/dev/null; TERM_PID=""
else
    log "SKIP: kitty not installed"
fi

# --- konsole (Sixel protocol) -----------------------------------------------
KONSOLE_PID=""

konsole_send() {
    dbus-send --session --type=method_call \
        --dest="org.kde.konsole-$KONSOLE_PID" /Sessions/1 \
        org.kde.konsole.Session.sendText string:"$1"
}

if command -v konsole >/dev/null 2>&1 && command -v dbus-send >/dev/null 2>&1; then
    log ""
    log "=== konsole (Sixel protocol) ==="
    env -u WAYLAND_DISPLAY QT_QPA_PLATFORM=xcb DISPLAY=$DISPLAY_NUM konsole \
        --hide-menubar --hide-tabbar \
        -e "$BIN" --protocol sixel "$FITS" 2>/dev/null &
    TERM_PID=$!
    KONSOLE_PID=$TERM_PID
    sleep 6
    if kill -0 "$TERM_PID" 2>/dev/null; then
        check_panels sixel konsole_send
    else
        fail "konsole failed to start"
    fi
    kill "$TERM_PID" 2>/dev/null; wait "$TERM_PID" 2>/dev/null; TERM_PID=""
else
    log "SKIP: konsole/dbus-send not installed"
fi

# --- summary ----------------------------------------------------------------
log ""
if [ "$FAILURES" -eq 0 ]; then
    log "All visual checks passed."
    exit 0
else
    log "$FAILURES visual check(s) FAILED. Screenshots kept in $OUT"
    exit 1
fi
