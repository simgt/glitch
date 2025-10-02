#!/bin/bash

export GST_PLUGIN_PATH=$PWD/target/debug/
export GST_TRACERS="glitchtracing"
export GST_DEBUG="glitch*:7"

# Array to store PIDs of background processes
PIDS=()

# Function to cleanup background jobs
cleanup() {
    echo "Cleaning up background processes..."

    # Kill processes using stored PIDs
    for pid in "${PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            echo "Killing process $pid"
            kill "$pid" 2>/dev/null
        fi
    done

    # Give processes time to terminate gracefully
    sleep 1

    # Force kill any remaining processes
    for pid in "${PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            echo "Force killing process $pid"
            kill -9 "$pid" 2>/dev/null
        fi
    done



    exit 0
}

# Set trap for both EXIT and SIGINT (Ctrl-C)
trap cleanup EXIT INT

gst-launch-1.0 \
    videotestsrc pattern=ball ! x264enc tune=zerolatency \
    ! queue ! rtph264pay config-interval=2 ! udpsink &
PIDS+=($!)

gst-launch-1.0 \
    udpsrc address=localhost ! application/x-rtp ! rtph264depay ! decodebin ! videoconvert ! identity ! fakesink &
PIDS+=($!)

while true; do
    sleep 999
done
