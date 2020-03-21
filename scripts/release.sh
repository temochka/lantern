#!/bin/sh
set -eax

ARCHITECTURE=${1:-"x86_64-unknown-linux-musl"}

cargo build --release --target=${ARCHITECTURE}
mkdir -p artifacts
tar -C target/${ARCHITECTURE}/release -cf artifacts/lantern-${ARCHITECTURE}.tar.gz lantern
