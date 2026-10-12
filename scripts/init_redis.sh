#!/usr/bin/env bash
set -x
set -eo pipefail

# Check if a Redis container is already publishing port 6379
RUNNING_CONTAINER=$(docker ps --filter 'publish=6379' --format '{{.Names}}')
if [[ -n $RUNNING_CONTAINER ]]; then
  echo >&2 "there is a redis container already running on port 6379, kill it with"
  echo >&2 "    docker kill ${RUNNING_CONTAINER}"
  exit 1
fi

if [[ -z "${SKIP_DOCKER}" ]]; then
  docker run \
      -p "6379:6379" \
      -d \
      --name "redis_$(date '+%s')" \
      redis:8-alpine
fi

>&2 echo "Redis is ready to go!"
