#!/bin/sh

set -e

timestamp() {
    date '+%Y-%m-%d %H:%M:%S %Z'
}

if [ -z "$VID_THEMER_VIDEO_DIR" ]; then
    echo "ERROR: VID_THEMER_VIDEO_DIR environment variable is required"
    exit 1
fi

validate_arg() {
    case "$1" in
        *\'*|*\"*|*\`*|*\$\;*) return 1 ;;
        *) return 0 ;;
    esac
}

set -- video-clip-extractor "$VID_THEMER_VIDEO_DIR"

if [ -n "$VID_THEMER_STRATEGY" ]; then
    validate_arg "$VID_THEMER_STRATEGY" || { echo "ERROR: Invalid characters in strategy"; exit 1; }
    set -- "$@" --strategy "$VID_THEMER_STRATEGY"
fi

if [ -n "$VID_THEMER_RESOLUTION" ]; then
    validate_arg "$VID_THEMER_RESOLUTION" || { echo "ERROR: Invalid characters in resolution"; exit 1; }
    set -- "$@" --resolution "$VID_THEMER_RESOLUTION"
fi

if [ "$VID_THEMER_AUDIO" = "false" ]; then
    set -- "$@" --audio false
fi

if [ -n "$VID_THEMER_CLIP_COUNT" ]; then
    set -- "$@" --clip-count "$VID_THEMER_CLIP_COUNT"
fi

if [ -n "$VID_THEMER_INTRO_EXCLUSION" ]; then
    set -- "$@" --intro-exclusion "$VID_THEMER_INTRO_EXCLUSION"
fi

if [ -n "$VID_THEMER_OUTRO_EXCLUSION" ]; then
    set -- "$@" --outro-exclusion "$VID_THEMER_OUTRO_EXCLUSION"
fi

if [ -n "$VID_THEMER_MIN_DURATION" ]; then
    set -- "$@" --min-duration "$VID_THEMER_MIN_DURATION"
fi

if [ -n "$VID_THEMER_MAX_DURATION" ]; then
    set -- "$@" --max-duration "$VID_THEMER_MAX_DURATION"
fi

if [ "$VID_THEMER_FORCE" = "true" ]; then
    set -- "$@" --force
fi

if [ "$VID_THEMER_HW_ACCEL" = "true" ]; then
    set -- "$@" --hw-accel
fi

echo "[$(timestamp)] Vid-Themer job started"
echo "[$(timestamp)] Effective config:"
echo "[$(timestamp)]   video_dir=${VID_THEMER_VIDEO_DIR}"
echo "[$(timestamp)]   strategy=${VID_THEMER_STRATEGY:-random}"
echo "[$(timestamp)]   resolution=${VID_THEMER_RESOLUTION:-1080p}"
echo "[$(timestamp)]   audio=${VID_THEMER_AUDIO:-true}"
echo "[$(timestamp)]   clip_count=${VID_THEMER_CLIP_COUNT:-2}"
echo "[$(timestamp)]   intro_exclusion=${VID_THEMER_INTRO_EXCLUSION:-2.0}"
echo "[$(timestamp)]   outro_exclusion=${VID_THEMER_OUTRO_EXCLUSION:-40.0}"
echo "[$(timestamp)]   min_duration=${VID_THEMER_MIN_DURATION:-20.0}"
echo "[$(timestamp)]   max_duration=${VID_THEMER_MAX_DURATION:-30.0}"
echo "[$(timestamp)]   force=${VID_THEMER_FORCE:-false}"
echo "[$(timestamp)]   hw_accel=${VID_THEMER_HW_ACCEL:-false}"
echo "[$(timestamp)] Running: $*"

set +e
"$@"
status=$?
set -e

echo "[$(timestamp)] Vid-Themer job finished with status ${status}"
exit "$status"
