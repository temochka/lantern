#!/bin/sh
set -eax

VERSION=${1:-"dev"}

scripts/release.sh $VERSION darwin
docker run --rm -v "$PWD":/usr/src/lantern -w /usr/src/lantern temochka/lantern-build:amd64 scripts/release.sh $VERSION linux_amd64
docker run --rm -v "$PWD":/usr/src/lantern -w /usr/src/lantern temochka/lantern-build:i386 scripts/release.sh $VERSION linux_i386
