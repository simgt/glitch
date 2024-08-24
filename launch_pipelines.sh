export GST_PLUGIN_PATH=$PWD/target/debug/
export GST_TRACERS="glitchtracing"
export GST_DEBUG="glitch*:7"

gst-launch-1.0 videotestsrc ! tee name=tee ! queue ! x264enc ! h264parse ! decodebin ! fakesink &
gst-launch-1.0 videotestsrc ! tee name=tee ! queue ! fakesink tee. ! fakesink tee. ! fakesink &
gst-launch-1.0 videotestsrc ! videoconvert ! fakesink &

trap 'kill $(jobs -p)' EXIT

sleep 10;
