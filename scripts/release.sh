#!/bin/sh
set -eax

VERSION=${1:-"alpha"}
ARCHITECTURE=${2:-"unknown"}

cargo build --release
tar -C target/release -cf lantern-${VERSION}_${ARCHITECTURE}.tar.gz lantern
