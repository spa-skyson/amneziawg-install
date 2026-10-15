# Repository rename and compatibility

The GitHub repository is now [`wiresock/awg-server-tools`](https://github.com/wiresock/awg-server-tools),
renamed from `wiresock/amneziawg-install` on 2026-10-15. The project is called
**AmneziaWG Server Tools**, by [WireSock](https://www.wiresock.net/).

## Existing installations

No reinstall or configuration migration is needed for the repository rename.
The installer and launchers keep their names: `amneziawg-install.sh`,
`amneziawg-web.sh`, and `amneziawg-proxy.sh`. Binaries, Rust package names,
systemd service names, environment variables, and installed paths are unchanged,
including `/usr/local/bin/amneziawg-install.sh`, `/etc/amnezia/amneziawg/`,
`/etc/amneziawg-web/`, `/etc/amneziawg-proxy/`, and the existing
`/usr/local/lib/amneziawg-install/boringtun/` release store.

Published release tags, assets, and SHA-256 pins are unchanged. New installers
use the new release-download URL; older installers keep their embedded URL
and follow GitHub's redirect with their existing HTTPS-only download code.
They continue to verify the same archive and binary hashes. Existing
installations do not switch their BoringTun release as part of this rename.

GitHub redirects repository links and Git clone/fetch/push operations from
the old name. Keep the old name unused: creating another repository at
`wiresock/amneziawg-install` would break these redirects. See
[GitHub's rename documentation](https://docs.github.com/en/repositories/creating-and-managing-repositories/renaming-a-repository).

## Git checkouts and installation commands

An existing checkout can keep its directory name. From inside it, update the
remote using the transport you already use:

```bash
# HTTPS
git remote set-url origin https://github.com/wiresock/awg-server-tools.git
# Or SSH
git remote set-url origin git@github.com:wiresock/awg-server-tools.git

git fetch origin
```

New clones use `git clone https://github.com/wiresock/awg-server-tools.git`
and `cd awg-server-tools`. Installation commands still invoke the same script
filenames. The README download commands use `curl -fLO` to fail on HTTP errors
and follow redirects. Update saved download links to the new repository URL.

## Signed artifacts

A repository rename does not rewrite signed provenance. Artifacts built before
the rename have the old repository and signing-workflow identity; changing
that identity in a verification command would reject the original signature.
For an older release, use its original release-note command, for example:

```bash
gh attestation verify <archive> \
  --repo wiresock/amneziawg-install \
  --cert-identity https://github.com/wiresock/amneziawg-install/.github/workflows/boringtun-artifacts.yml@refs/heads/main \
  --source-digest <original-artifacts-commit> \
  --source-ref refs/heads/main --deny-self-hosted-runners
```

The original source digest and exact workflow identity remain required.
Artifacts built after the rename use `wiresock/awg-server-tools` for both the
repository and workflow identity. Existing release notes and signatures stay
as published; upstream attribution and historical repository names remain in
the documentation where they describe past work.

The release publisher derives the expected identity from `GITHUB_REPOSITORY`
and continues to require an exact match. For a future publication, use an
artifacts run from the renamed repository on `main`, verify that its bytes
match the reviewed release contract, and record its new run and source digest
before publishing. A pre-rename approval's run identifier does not become a
new signing identity merely because the repository was renamed. Do not relax
provenance checks or replace existing release assets to accommodate the rename.
