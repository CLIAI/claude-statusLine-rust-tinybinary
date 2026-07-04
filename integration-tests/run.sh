#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="${ROOT_DIR}/integration-tests/tmp"
IMAGE_NAME="${INTEGRATION_TEST_IMAGE:-claude-statusline-integration:local}"
CONTAINER_NAME="${INTEGRATION_TEST_CONTAINER:-claude-statusline-integration}"
KEEP_DOCKER_ARTIFACTS="${KEEP_DOCKER_ARTIFACTS:-0}"

mkdir -p "${ARTIFACT_DIR}"

cleanup() {
  if [[ "${KEEP_DOCKER_ARTIFACTS}" != "1" ]]; then
    docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true

echo "==> Building integration Docker image"
docker build -t "${IMAGE_NAME}" -f "${ROOT_DIR}/integration-tests/Dockerfile" "${ROOT_DIR}/integration-tests"

docker_args=()
if [[ -n "${INTEGRATION_TESTS_DOCKER_ARGS:-}" ]]; then
  # shellcheck disable=SC2206
  docker_args=(${INTEGRATION_TESTS_DOCKER_ARGS})
fi

echo "==> Starting isolated integration container"
docker run -d --name "${CONTAINER_NAME}" "${docker_args[@]}" "${IMAGE_NAME}" sleep infinity >/dev/null

echo "==> Copying repository into container"
docker exec "${CONTAINER_NAME}" mkdir -p /work/repo
tar \
  --exclude .git \
  --exclude target \
  --exclude integration-tests/tmp \
  -C "${ROOT_DIR}" \
  -cf - . \
  | docker exec -i "${CONTAINER_NAME}" tar -C /work/repo -xf -

echo "==> Building project and running tmux integration checks"
if ! docker exec "${CONTAINER_NAME}" bash /work/repo/integration-tests/bin/run-inside-container.sh; then
  docker cp "${CONTAINER_NAME}:/work/repo/integration-tests/tmp/." "${ARTIFACT_DIR}/" >/dev/null 2>&1 || true
  echo "integration tests failed; artifacts copied to ${ARTIFACT_DIR}" >&2
  exit 1
fi

docker cp "${CONTAINER_NAME}:/work/repo/integration-tests/tmp/." "${ARTIFACT_DIR}/" >/dev/null 2>&1 || true

if [[ "${KEEP_DOCKER_ARTIFACTS}" == "1" ]]; then
  echo "==> Kept Docker container ${CONTAINER_NAME} and image ${IMAGE_NAME}"
else
  echo "==> Integration tests passed; Docker container will be removed"
fi
