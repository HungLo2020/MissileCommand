---
name: push-and-release
description: Push repository changes and verify that CI and GitHub release workflows succeed. Use this whenever the user asks to commit and push, check or fix CI, cut/ship/publish a release, bump the version, create a tag, or confirm a release is live. Use it before claiming that changes are pushed or that a release succeeded.
---

# Push and release

Ship changes only after the same checks CI runs have passed locally. A push that fails
CI wastes a review cycle, and a release is public and awkward to undo, so verify before
publishing rather than discovering problems from GitHub.

## 1. Inspect before touching anything

```sh
git status --short
git branch --show-current
git remote -v
git fetch origin
git status -sb
git diff
```

- Confirm the branch is `main` (or the branch the user named) and that it is not behind
  or diverged from `origin/main`.
- Read the diff (`git diff` and `git diff --cached`) and confirm every change belongs to
  the task. Never commit `.env` files, credentials, ROMs, build output, `dist/`, or
  `data/`.
- If `gh auth status` fails, stop and report it instead of falling back to raw git only.

## 2. Run the local gate

From the repository root, use the bundled script, which mirrors CI and packages a release
candidate:

```sh
.agents/skills/push-and-release/scripts/verify.sh
```

It runs:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --no-default-features
python3 -m unittest discover -s DevUtils -p 'test_*.py'
python3 DevUtils/verify_engine.py
cargo build --locked --release
python3 DevUtils/package.py
python3 DevUtils/validate_package.py dist/MissileCommand-<version>-linux-x64.zip
```

The optional native-window check needs Xvfb, xauth, xdotool, x11-utils, and a Vulkan 1.3
driver (Mesa lavapipe is sufficient). It creates its own private display, so it is safe
to run on a desktop:

```sh
python3 DevUtils/window_smoke.py --binary target/release/missile-command
```

Fix failures before continuing. If a check is impossible in the current environment
(for example, no Xvfb), say which one was skipped and do not describe the run as fully
verified.

## 3. Commit

Stage only the intended paths, then write a message that explains the change:

```sh
git add <paths>
git commit -m "<imperative summary>"
```

Match the repository's existing style: a short sentence-case subject, with detail in the
body when the reason is not obvious. Do not commit generated `dist/` archives or
`target/` output. Do not use `--no-verify` unless the user explicitly asks. Do not amend
a commit that has already been pushed; add a new commit instead.

## 4. Push and watch CI

```sh
git push origin main
.agents/skills/push-and-release/scripts/watch-workflow.sh HEAD ci.yml
```

`watch-workflow.sh` accepts an optional commit and workflow name. Passing `ci.yml`
avoids matching a release workflow that only runs for tags:

```sh
.agents/skills/push-and-release/scripts/watch-workflow.sh [commit] [workflow]
```

Wait for the run to finish. If it fails:

```sh
gh run view <run-id> --log-failed
```

Fix the cause, commit again, and repeat from step 2. Do not push a release tag while
`main` is red. If the push is rejected because the remote moved, fetch and rebase or
merge; never force-push `main`.

## 5. Release, only when the user asked for one

The tag must match `package.version` in `Cargo.toml`. The release workflow rejects a
mismatch.

1. Bump `version` in `Cargo.toml` and update version strings in README/docs examples.
2. Rebuild once without `--locked` so `Cargo.lock` records the new package version, then
   rerun the local gate.
3. Commit and push the bump to `main`, then watch CI:

   ```sh
   .agents/skills/push-and-release/scripts/watch-workflow.sh HEAD ci.yml
   ```

4. Create and push an annotated tag:

   ```sh
   git tag -a v<version> -m "Missile Command v<version>"
   git push origin v<version>
   ```

5. Watch the release workflow (`release.yml`) for that tag:

   ```sh
   .agents/skills/push-and-release/scripts/watch-workflow.sh \
     "$(git rev-parse 'v<version>^{commit}')" release.yml
   ```

6. Verify the published asset before reporting success:

   ```sh
   gh release view v<version>
   gh release download v<version> --pattern 'MissileCommand-*.zip' --dir /tmp/release-check
   python3 DevUtils/validate_package.py /tmp/release-check/MissileCommand-<version>-linux-x64.zip
   ```

   Confirm the asset name, the archive contents, and the executable bit. A green workflow
   alone is not proof that the right archive was published.

## Safety rules

- Never force-push `main` or rewrite published tags.
- Never publish a release from a dirty tree, a failing `main`, or a tag that disagrees
  with `Cargo.toml`.
- Ask before pushing a tag or creating a release if the user only asked to push code.
- Do not stage unrelated working-tree changes.
- Report exactly what was run, what passed, and what was skipped. Link the workflow run
  or release URL so the user can check it.
