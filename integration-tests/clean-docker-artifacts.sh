#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_NAME="${INTEGRATION_TEST_IMAGE:-claude-statusline-integration:local}"
CONTAINER_NAME="${INTEGRATION_TEST_CONTAINER:-claude-statusline-integration}"

docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
docker image rm "${IMAGE_NAME}" >/dev/null 2>&1 || true
rm -rf "${ROOT_DIR}/integration-tests/tmp"

echo "removed ${CONTAINER_NAME}, ${IMAGE_NAME}, and integration-tests/tmp"
