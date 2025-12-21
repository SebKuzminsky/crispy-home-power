#!/bin/bash
set -e

cargo build --offline --release
cargo objcopy --release -- -O ihex hello-world.hex
echo "click Reset on teensy"
teensy_loader_cli --mcu=TEENSY40 -w hello-world.hex
