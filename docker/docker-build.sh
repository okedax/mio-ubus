#!/bin/bash

set -e

docker build -t ubus-builder \
    --build-arg UID=$(id -u) \
    --build-arg GID=$(id -g) \
    -f "docker/Dockerfile.ubus-builder" .

docker build -t lib-ubus-builder \
    --build-arg UID=$(id -u) \
    --build-arg GID=$(id -g) \
    -f "docker/Dockerfile.lib-builder" .
