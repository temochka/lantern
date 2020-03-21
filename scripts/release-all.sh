#!/bin/sh
set -eax

VERSION=${1:-"dev"}

scripts/release.sh $VERSION x86_64-apple-darwin
docker run --rm -v "$PWD":/usr/src/lantern -w /usr/src/lantern temochka/lantern-build:amd64 scripts/release.sh $VERSION x86_64-unknown-linux-musl
docker run --rm -v "$PWD":/usr/src/lantern -w /usr/src/lantern temochka/lantern-build:i386 scripts/release.sh $VERSION i686-unknown-linux-musl
