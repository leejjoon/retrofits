#!/bin/bash
# Script to generate the animated GIFs using vhs (https://github.com/charmbracelet/vhs)

# Build the release binary
cargo build --release

# Add the release directory to PATH so vhs can find the retrofits command
export PATH=$PATH:$(pwd)/target/release

# Ensure assets directory exists
mkdir -p assets

# Generate GIFs
vhs kitty.tape
vhs halfblocks.tape

echo "GIF generation complete."
