#!/bin/sh
set -eax

docker build --tag temochka/lantern-build:i386 -f i386.Dockerfile .
docker push temochka/lantern-build:i386

docker build --tag temochka/lantern-build:amd64 -f amd64.Dockerfile .
docker push temochka/lantern-build:amd64
