#!/bin/bash
set -e

cargo build --offline --release
cargo objcopy --release -- -O ihex teensy-battery-controller.hex
echo "click Reset on teensy"
teensy_loader_cli --mcu=TEENSY40 -w teensy-battery-controller.hex
