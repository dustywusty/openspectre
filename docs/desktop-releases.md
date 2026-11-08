# Desktop releases

A version tag starts the **Desktop client** workflow.
The workflow builds six packages, verifies their contents, and publishes them together on GitHub Releases.
Normal branch and pull-request runs only produce downloadable Actions artifacts.

## Supported packages

| Operating system | Architecture | Package | Build runner |
| --- | --- | --- | --- |
| macOS | Apple silicon / ARM64 | `.app` in ZIP | `macos-15` |
| macOS | Intel / x86-64 | `.app` in ZIP | `macos-15-intel` |
| Windows | Intel or AMD / x86-64 | EXE in ZIP | `windows-2025` |
| Windows | ARM64 | EXE in ZIP | `windows-11-arm` |
| Linux | Intel or AMD / x86-64 | TAR.GZ | `ubuntu-22.04` |
| Linux | ARM64 | TAR.GZ | `ubuntu-22.04-arm` |

These are native builds on [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Each runner executes the Rust tests and Clippy before packaging.
The executable header must match the package's operating system and architecture.

Linux packages target glibc 2.35 or newer and require desktop OpenGL and ALSA libraries.
The macOS app requires macOS 11 or newer.
The project does not build 32-bit, mobile, or console clients.
The local macOS ARM64 smoke test covers the graphical flow. CI compilation does not certify every GPU or audio device.

## Publish a version

1. Set the version in `crates/spectre-client/Cargo.toml`.
2. Update the lockfile with `cargo check -p spectre-client`.
3. Add the release notes to `CHANGELOG.md`.
4. Commit the changes and merge them to `main`.
5. Tag that commit with the same version:

```sh
git tag -a v0.1.0 -m "OpenSpectre 0.1.0"
git push origin v0.1.0
```

The tag must match the client version exactly. A mismatch stops the workflow before the builds.
For a prerelease, use a version such as `0.2.0-rc.1` and the tag `v0.2.0-rc.1`.
The workflow marks that GitHub Release as a prerelease.

The workflow waits for all six builds. It then verifies every archive's version, commit, and executable architecture.
It creates a draft release, uploads all packages and `SHA256SUMS.txt`, then publishes the release.
An incomplete build matrix cannot publish a new release.
No additional repository secret is necessary. Only the publication job receives permission to write releases.

You can also publish a GitHub Release for an existing matching tag through the GitHub interface.
That event starts the same build process and adds the packages to the existing release.
For that route, the release is already public while the builds run.
The tag route publishes a new release only after its packages are ready.

## Test without publishing

Open **Actions > Desktop client > Run workflow**.
Select the branch and leave **publish** disabled.
Leave **tag** empty to build that branch, or enter an existing version tag to test a release candidate.
The combined artifact contains all six packages and their checksums.

To publish from a manual run, select **publish** and supply an existing matching tag.
The workflow checks out the tag and verifies its commit before building.

## Reruns

A rerun can upload missing assets. It skips existing assets whose SHA-256 digest matches.
It refuses to replace an asset with different bytes.
If a fix changes a published package, create a new version and tag.
An upload failure leaves a new release as a draft until a successful rerun completes it.

Build jobs use the locked Rust dependencies. Compiler and runner updates can still change the output bytes.
Use the artifacts from the original run when you retry a failed publication job.

## Local packaging

The packaging script defaults to the current Rust compiler's host architecture:

```sh
python3 scripts/package-desktop.py
```

For a specific architecture on the same operating system, install its Rust target and supply it explicitly:

```sh
rustup target add x86_64-apple-darwin
python3 scripts/package-desktop.py --target x86_64-apple-darwin
```

The script reads the executable from `target/<target>/release/`.
It includes a `build-info.json` file with the version, target, and source commit.
The `--no-build` flag packages an existing binary at that path.
Each archive uses a stable name, such as `OpenSpectre-0.1.0-windows-arm64.zip`.

The macOS app has an ad-hoc signature. It does not have Developer ID signing or Apple notarization.
Windows packages do not have an Authenticode signature.
Those distribution credentials can be added separately without changing the version-tag workflow.

## Release checks

```sh
python3 scripts/test_desktop_release.py
actionlint .github/workflows/desktop.yml
```

The tests cover tag mismatches, architecture mismatches, missing packages, source revisions, checksums, draft publication, and reruns.
The shared matrix and publication code live in `scripts/desktop_release.py`.
