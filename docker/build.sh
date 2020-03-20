#!/bin/sh
set -eax

docker build --tag temochka/lantern-build:musl .
docker push temochka/lantern-build:musl
