#!/bin/bash
set -eax

LAST_TAG=$(git describe --abbrev=0 --tags | sed -e s/\-.*//g)
SHA=$(git rev-parse --short HEAD)
COMMIT=$(git rev-parse HEAD)
AUTO_TAG="${LAST_TAG}-${SHA}"
TAG="${1:-$AUTO_TAG}"

if [ -z "$1" ]; then
    git tag --force $TAG
    git push --force git@github.com:temochka/lantern.git $TAG
fi

ghr -t ${GITHUB_TOKEN} -u temochka -r lantern -c ${COMMIT} -delete ${TAG} ./artifacts
