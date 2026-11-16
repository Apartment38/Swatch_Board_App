#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
image=${SWATCH_TEST_IMAGE:-swatch-board-tests}
build_args=()
if [[ -n ${CODEX_PROXY_CERT:-} && -f $CODEX_PROXY_CERT ]]; then
  build_args+=(--secret "id=proxy_ca,src=$CODEX_PROXY_CERT")
fi
docker build "${build_args[@]}" -f testing/Dockerfile -t "$image" .
# Pass a writable directory to retain screenshots and JSON results.
args=()
git_tmp=$(mktemp -d "${TMPDIR:-/tmp}/swatch-test-git.XXXXXX")
container=
cleanup() {
  if [[ -n $container ]]; then docker rm -f "$container" >/dev/null; fi
  rm -rf "$git_tmp"
}
trap cleanup EXIT
chmod 755 "$git_tmp"
git bundle create "$git_tmp/repository.bundle" --all
chmod 644 "$git_tmp/repository.bundle"
args+=(-v "$git_tmp/repository.bundle:/repository.bundle:ro"
       -e CARGO_TARGET_DIR=/tmp/swatch-target
       -e CARGO_NET_OFFLINE=true -e PYTHONDONTWRITEBYTECODE=1 -e HOME=/tmp)
if [[ -n ${SWATCH_TEST_ARTIFACTS:-} ]]; then
  mkdir -p "$SWATCH_TEST_ARTIFACTS"
  artifacts=$(cd "$SWATCH_TEST_ARTIFACTS" && pwd)
  args+=(-e SWATCH_TEST_ARTIFACTS=/tmp/swatch-artifacts)
fi
container=$(docker create "${args[@]}" "$image" bash -c \
  'git clone --quiet --bare /repository.bundle /tmp/test-git && export GIT_DIR=/tmp/test-git && exec bash testing/run-tests.sh')
result=0
docker start -a "$container" || result=$?
if [[ -n ${SWATCH_TEST_ARTIFACTS:-} ]]; then
  docker cp "$container:/tmp/swatch-artifacts/." "$artifacts/" || {
    if (( result == 0 )); then result=1; fi
  }
fi
exit "$result"
