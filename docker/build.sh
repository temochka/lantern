#!/bin/sh
set -eax

docker build --tag temochka/lantern-build:amd64 -f amd64.Dockerfile .
docker push temochka/lantern-build:amd64
