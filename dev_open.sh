#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
MODE="run"
CARGO_ARGS=()
APP_ARGS=()

usage() {
    cat <<'EOF'
Usage: ./dev_open.sh [--watch] [--release] [-- <app args>]

Fast development launcher for Viceroy.

Modes:
  --watch    Rebuild and restart automatically when source files change.
  --release  Run the release binary instead of the debug build.
  --help     Show this help text.

Examples:
  ./dev_open.sh
  ./dev_open.sh -- --silent-update-check
  ./dev_open.sh --watch
  ./dev_open.sh --watch -- --silent-update-check
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --watch)
            MODE="watch"
            shift
            ;;
        --release)
            CARGO_ARGS+=("--release")
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        --)
            shift
            while [[ $# -gt 0 ]]; do
                APP_ARGS+=("$1")
                shift
            done
            ;;
        *)
            APP_ARGS+=("$1")
            shift
            ;;
    esac
done

stop_running_viceroy() {
    local did_stop=0

    for process_name in "Viceroy Dev" "Viceroy"; do
        if pgrep -x "$process_name" >/dev/null 2>&1; then
            pkill -x "$process_name" || true
            did_stop=1
        fi
    done

    if pgrep -f "target/.*/viceroy" >/dev/null 2>&1; then
        pkill -f "target/.*/viceroy" || true
        did_stop=1
    fi

    if [[ "$did_stop" -eq 1 ]]; then
        sleep 0.5
    fi
}

append_shell_word() {
    local __result_var="$1"
    local __escaped=""
    shift
    printf -v __escaped '%q' "$1"
    printf -v "$__result_var" '%s %s' "${!__result_var}" "$__escaped"
}

is_macos() {
    [[ "$(uname -s)" == "Darwin" ]]
}

run_once() {
    local cmd=(cargo run --bin viceroy)
    local build_cmd=(cargo build --bin viceroy)
    local profile_dir="debug"

    stop_running_viceroy
    cd "$REPO_ROOT"
    export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-1}"

    if [[ ${#CARGO_ARGS[@]} -gt 0 ]]; then
        cmd=(cargo run "${CARGO_ARGS[@]}" --bin viceroy)
        build_cmd=(cargo build "${CARGO_ARGS[@]}" --bin viceroy)
        for arg in "${CARGO_ARGS[@]}"; do
            if [[ "$arg" == "--release" ]]; then
                profile_dir="release"
            fi
        done
    fi

    if is_macos; then
        local app_out_dir="$REPO_ROOT/target/dev-app"
        local app_name="Viceroy Dev"
        local app_path="$app_out_dir/$app_name.app"
        local open_cmd=(open -a "$app_path" --args --dev-show-on-launch)

        if [[ "$profile_dir" == "release" ]]; then
            app_name="Viceroy"
            app_path="$app_out_dir/$app_name.app"
            open_cmd=(open -a "$app_path" --args --dev-show-on-launch)
        fi

        VICEROY_BUILD_PROFILE="$profile_dir" \
        VICEROY_APP_OUT_DIR="$app_out_dir" \
        VICEROY_APP_NAME="$app_name" \
        VICEROY_SKIP_CODESIGN=1 \
        "$REPO_ROOT/build_app.sh"

        if [[ ! -d "$app_path" ]]; then
            echo "Dev app bundle was not created at $app_path" >&2
            exit 1
        fi

        if [[ ${#APP_ARGS[@]} -gt 0 ]]; then
            open_cmd+=("${APP_ARGS[@]}")
        fi

        echo "Opening macOS dev app bundle: $app_path"
        "${open_cmd[@]}"
        return
    fi

    if [[ ${#APP_ARGS[@]} -gt 0 ]]; then
        cmd+=("--" "${APP_ARGS[@]}")
    fi
    "${cmd[@]}"
}

run_watch() {
    cd "$REPO_ROOT"

    if command -v watchexec >/dev/null 2>&1; then
        local watch_cmd=("./dev_open.sh")
        export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-1}"
        if [[ ${#CARGO_ARGS[@]} -gt 0 ]]; then
            watch_cmd+=("${CARGO_ARGS[@]}")
        fi
        watch_cmd+=("--")
        if [[ ${#APP_ARGS[@]} -gt 0 ]]; then
            watch_cmd+=("${APP_ARGS[@]}")
        fi
        exec watchexec \
            --restart \
            --watch src \
            --watch Cargo.toml \
            --watch Cargo.lock \
            --watch build.rs \
            --ignore target \
            --ignore Viceroy.app \
            -- "${watch_cmd[@]}"
    fi

    if cargo watch --version >/dev/null 2>&1; then
        export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-1}"
        local shell_command="./dev_open.sh"
        if [[ ${#CARGO_ARGS[@]} -gt 0 ]]; then
            for arg in "${CARGO_ARGS[@]}"; do
                append_shell_word shell_command "$arg"
            done
        fi
        shell_command+=" --"
        if [[ ${#APP_ARGS[@]} -gt 0 ]]; then
            for arg in "${APP_ARGS[@]}"; do
                append_shell_word shell_command "$arg"
            done
        fi
        exec cargo watch \
            --watch src \
            --watch Cargo.toml \
            --watch Cargo.lock \
            --watch build.rs \
            --ignore target \
            --ignore Viceroy.app \
            -s "$shell_command"
    fi

    cat <<'EOF' >&2
Watch mode needs either `watchexec` or `cargo-watch`.

Install one of these:
  brew install watchexec
  cargo install cargo-watch

Then run:
  ./dev_open.sh --watch
EOF
    exit 1
}

if [[ "$MODE" == "watch" ]]; then
    run_watch
else
    run_once
fi
