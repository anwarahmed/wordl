# Releasing wordl

A release is a pull request that raises `VERSION` at the top of the `wordl` script.
Merging it publishes the release; nothing else does. This file is the whole procedure.
Steps a machine can check are checked by a machine, and the rest are the checklist
below, which CI will not let a release skip.

## What is enforced, and by what

| Step | Enforced by |
|------|-------------|
| shellcheck, the tests on Linux and macOS, the oldest supported bash | `ci.yml`, required to merge |
| The archive builds; `install.sh` installs it; the AUR package renders | `release.yml` on the pull request |
| The version only goes up | `release-checklist.yml`, required to merge |
| Every line of the checklist below is ticked in the pull request | `release-checklist.yml`, required to merge |
| `wordl-<version>.tar.gz`, `SHA256SUMS` and `PKGBUILD` are published under tag `v<version>` | `release.yml` on merge |
| The Homebrew formula reaches the new version | `release.yml` on merge, when the `TAP_TOKEN` secret is set; a warning on the run otherwise |
| The AUR package is pushed | `release.yml` on merge, when `AUR_SSH_PRIVATE_KEY` is set |

## Before merging: the checklist

Copy these lines into the description of the pull request that raises the version and
tick each one (`- [x]`). A line that does not apply is ticked too, with a note after it
saying why. The `release checklist` check fails while any line is missing or unticked,
and it reads the lines from this file, so a line added here is required from then on.

- [ ] `VERSION` in the `wordl` script is raised
- [ ] Every change meant for this release is on `main` or in this pull request
- [ ] `README.md` matches the game: features, keys table, options, install and uninstall sections
- [ ] The in-game help and `wordl --help` match the game
- [ ] `CLAUDE.md` is current: decisions, patterns to keep, known gaps
- [ ] Word lists changed: `tools/build-words.py` was re-run and both lists are committed
- [ ] Packaging changed: `PKGBUILD.in` and `SRCINFO.in` changed together, the archive's name and layout did not
- [ ] The lines `#!/usr/bin/env bash`, `VERSION="..."` and `MANAGED_BY=""` in `wordl` are exactly as they were (the packages rewrite them)
- [ ] The changes were played in a real terminal, including a tiny window, not only run through the tests
- [ ] What only a Mac can show is listed in the pull request as not verified
- [ ] Pull request titles since the last release read well as release notes

Choosing the number: raise the patch (`0.1.0` to `0.1.1`) for fixes and small additions,
the minor (`0.1.x` to `0.2.0`) when users have to do something, or when saved files or
the release archive's name or layout change.

## After merging

Do these in order. They cannot be ticked in a pull request, so the release run does the
checking where it can.

1. Watch the release run to the end; it must be green.

   ```sh
   gh run list --workflow release.yml --limit 1
   gh release view v<version>    # wordl-<version>.tar.gz, SHA256SUMS, PKGBUILD
   ```

2. Homebrew. With `TAP_TOKEN` set the run has already waited for the formula and fails
   if it did not arrive. Without it the run carries a "Homebrew tap not notified"
   warning, and this is by hand:

   ```sh
   gh workflow run update.yml --repo anwarahmed/homebrew-tap
   gh api repos/anwarahmed/homebrew-tap/contents/Formula/wordl.rb -q .content | base64 -d | grep url
   ```

3. Installed copies. Homebrew: `brew update && brew upgrade wordl`, then
   `wordl --version`. Copies from the install script update themselves the next time
   they start; `wordl update` does it at once.

4. Confirm on a Mac whatever the pull request listed as not verified.

5. Remove the branch's worktree and branch, and fast-forward `main`.
