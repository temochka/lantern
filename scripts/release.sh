#!/bin/sh
set -eax

VERSION=${1:-"alpha"}
ARCHITECTURE=${2:-"x86_64-unknown-linux-musl"}

cargo build --release --target=${ARCHITECTURE}
tar -C target/${ARCHITECTURE}/release -cf lantern-${VERSION}_${ARCHITECTURE}.tar.gz lantern
