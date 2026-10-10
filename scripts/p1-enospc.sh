#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Authorized isolated Linux-only tmpfs; never fill a host disk.
set -euo pipefail
p="$(mktemp -d "$RUNNER_TEMP/tram-p1-enospc.XXXXXXXX")"
mounted=0
cleanup() {
  if [ "$mounted" = 1 ]; then sudo umount "$p"; fi
  rmdir "$p"
}
trap cleanup EXIT
sudo mount -t tmpfs -o size=2m,nosuid,nodev,noexec,mode=0777 tmpfs "$p"
mounted=1
sudo chown "$(id -u):$(id -g)" "$p"
echo "P1_ENOSPC_ISOLATED_MOUNT=$p"
TRAM_P1_ENOSPC_ROOT="$p" cargo +1.88.0 test -p tram-engine --test enospc --locked -- --ignored --nocapture
