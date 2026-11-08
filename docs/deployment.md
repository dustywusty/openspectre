# OVH deployment

The production address is https://spectre.bitchimfabulo.us.
One Node.js container serves the browser files and multiplayer WebSockets.
Caddy handles HTTPS and forwards requests to `127.0.0.1:18081`.
The image includes the open asset pack. Original disc files stay outside the image.

## Automatic releases

After the deployment workflow reaches `main`, each successful `Standalone game` push build triggers a release.
The workflow builds an AMD64 image, checks HTTP and two multiplayer connections, then uploads the image over SSH.
Only the current `main` commit can deploy. Pull requests test the container without deployment credentials.

Repository variables:

| Variable | Value |
| --- | --- |
| `OVH_HOST` | `vps-ea9f56f0.vps.ovh.us` |
| `OVH_USER` | `openspectre-deploy` |
| `OVH_DEPLOY_ENABLED` | `true` |

Repository secrets:

- `OVH_DEPLOY_KEY`: Dedicated SSH private key
- `OVH_KNOWN_HOSTS`: Trusted SSH host keys

The deployment key can only run the image receiver. It cannot open an interactive shell.
The receiver accepts a full commit SHA and keeps the previous image for rollback.
If the new container fails its health check, the receiver restores the previous image.

## Host configuration

The host requires Docker Compose, Caddy, Bash, and `flock`.
These files are installed as root:

| Repository file | Host path |
| --- | --- |
| `deploy/compose.yaml` | `/opt/openspectre/compose.yaml` |
| `deploy/receive-image.sh` | `/usr/local/sbin/openspectre-receive-image` |
| `deploy/ssh-entrypoint.sh` | `/usr/local/bin/openspectre-deploy-key` |
| `deploy/spectre.caddy` | `/etc/caddy/spectre.caddy` |

The main Caddyfile imports `/etc/caddy/spectre.caddy`.
DNS must point to the VPS. Use DNS-only until certificate issuance succeeds.

The deployment user's authorized key has this prefix:

```text
restrict,command="/usr/local/bin/openspectre-deploy-key"
```

Its sudoers rule permits only the root-owned receiver:

```text
openspectre-deploy ALL=(root) NOPASSWD: /usr/local/sbin/openspectre-receive-image *
```

Host configuration changes require a separate administrator update. Image releases do not replace these files.

## Local container check

```sh
docker build --platform linux/amd64 -t openspectre:local .
IMAGE_TAG=local docker compose -f deploy/compose.yaml up -d --wait
npm ci
node scripts/smoke-deployment.mjs http://127.0.0.1:18081
IMAGE_TAG=local docker compose -f deploy/compose.yaml down
```

## Operations

Run these commands on the VPS as an administrator:

```sh
cd /opt/openspectre
sudo docker compose --env-file version.env ps
sudo docker compose --env-file version.env logs --tail 100
```

To restore the previous release:

```sh
cd /opt/openspectre
sudo docker compose --env-file previous.env up -d --wait
sudo cp previous.env version.env
```

Set `OVH_DEPLOY_ENABLED=false` to stop automatic releases during an investigation.
The container has a 512 MiB memory limit and one CPU limit. Capacity at these limits needs load testing.
Multiplayer rooms live in memory. Each deployment disconnects players and clears rooms.
Single-player games run in the browser.
