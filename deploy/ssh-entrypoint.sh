#!/usr/bin/env bash
set -euo pipefail
exec sudo -n /usr/local/sbin/openspectre-receive-image "${SSH_ORIGINAL_COMMAND:-}"
