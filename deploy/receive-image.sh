#!/usr/bin/env bash
# Root-owned on the host. The restricted deployment key streams one image archive.
set -euo pipefail
release=${1:-}
[[ "$release" =~ ^[a-f0-9]{40}$ ]] || { echo 'Expected a full commit SHA' >&2; exit 2; }
cd /opt/openspectre
exec 9>/opt/openspectre/deploy.lock
flock -w 300 9
previous=''
if [[ -f version.env ]]; then previous=$(sed -n 's/^IMAGE_TAG=//p' version.env); fi
gzip -dc | docker load
docker image inspect "openspectre:$release" >/dev/null
printf 'IMAGE_TAG=%s\n' "$release" > candidate.env
if docker compose --env-file candidate.env up -d --wait --wait-timeout 90; then
    if [[ -n "$previous" && "$previous" != "$release" ]]; then printf 'IMAGE_TAG=%s\n' "$previous" > previous.env; fi
    mv candidate.env version.env
    echo "Deployed $release"
else
    echo 'Deployment failed; restoring the previous image' >&2
    if [[ -f version.env ]]; then docker compose --env-file version.env up -d --wait --wait-timeout 90; fi
    rm -f candidate.env
    exit 1
fi
# Keep the deployed and immediately previous application images, never prune other apps.
keep_previous=$(sed -n 's/^IMAGE_TAG=//p' previous.env 2>/dev/null || true)
while read -r tag; do
    if [[ "$tag" =~ ^[a-f0-9]{40}$ && "$tag" != "$release" && "$tag" != "$keep_previous" ]]; then
        docker image rm "openspectre:$tag" || true
    fi
done < <(docker image ls openspectre --format '{{.Tag}}')
