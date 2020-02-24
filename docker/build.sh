#!/bin/sh
set -eax

docker build --tag temochka/lantern-build:i386 -f Dockerfile.i386 .
docker push temochka/lantern-build:i386

docker build --tag temochka/lantern-build:amd64 -f Dockerfile.amd64 .
docker push temochka/lantern-build:amd64
