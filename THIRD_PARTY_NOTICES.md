# Third-party notices

Cowork itself (the desktop app, the guest CLI, and all code in this repository)
is licensed under **Apache-2.0** — see [LICENSE](./LICENSE). This file documents
third-party material that Cowork **distributes** or relies on, and how the
corresponding obligations are met.

## Ubuntu 24.04 WSL root filesystem (redistributed)

Cowork's primary provisioning path imports a pinned Ubuntu 24.04 (noble) WSL root
filesystem. To avoid a hard Microsoft Store dependency and to pin a known-good
image, that root filesystem is **re-hosted byte-identically** on this project's
GitHub Releases (asset `cowork-ubuntu-24.04-rootfs.tar.gz`, published by
`.github/workflows/rootfs.yml`). It is **unmodified** Canonical Ubuntu, copied
bit-for-bit from Canonical's official image
`ubuntu-noble-wsl-amd64-wsl.rootfs.tar.gz`
(`https://cloud-images.ubuntu.com/wsl/releases/noble/current/`),
SHA-256 `8251e27ffff381a4af5f41dcb94d867de3e0d9774a9241908ab34555d99315ea`.

### Licensing

The root filesystem contains many independent software packages under their own
licenses (GPL-2.0, GPL-3.0, LGPL, MIT, BSD, and others). Distributing it does not
place Cowork's own code under those licenses: Cowork links against none of these
packages and creates no derivative work of them — it ships the image as a separate
artifact (mere aggregation) and invokes the contained programs as ordinary
subprocesses. Each package's license text and copyright notice are preserved
inside the image, unmodified, under `/usr/share/doc/<package>/copyright`.

### Corresponding source (written offer)

Because the image is unmodified Ubuntu, the complete corresponding source for its
GPL- and LGPL-licensed components is the source published by Ubuntu and is
available from Ubuntu's official archives:

- `https://archive.ubuntu.com/ubuntu/` (and regional mirrors)
- `https://launchpad.net/ubuntu/+source/<package>`

For any GPL/LGPL component for which the source is not readily available from the
above, the maintainers will, **for at least three years**, provide the complete
corresponding source on request — open an issue at
`https://github.com/conr2d/cowork/issues`.

### Trademark

"Ubuntu" is a registered trademark of Canonical Ltd. Cowork is not affiliated with
or endorsed by Canonical. The distribution is provisioned under the name **Cowork**
(never "Ubuntu"), and the Ubuntu image is redistributed unmodified.

## Components installed at runtime (not redistributed by Cowork)

Cowork does **not** redistribute the toolchain or AI agents. During setup they are
downloaded and installed on the user's machine, from their own upstream sources, by
the user's action:

- **Homebrew (Linuxbrew)**, **mise**, and any language runtimes they manage;
- the selected AI coding agents — **Claude Code** (Anthropic), **Codex** (OpenAI),
  **Antigravity** (Google).

These remain under their respective upstream licenses and terms; Cowork only
orchestrates their official installers.
