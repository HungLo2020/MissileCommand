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
```

- Confirm the branch is `main` (or the branch the user named) and that it is not behind
  or diverged from `origin/main`.
- Read the diff (`git diff` and `git diff --cached`) and confirm every change belongs to
  the task. Never commit `.env` files, credentials, ROMs, build output, or `data/`.
- If `gh auth status` fails, stop and report it instead of falling back to raw git only.

## 2. Run the local gate

From the repository root, use the bundled script, which mirrors CI and packages a release candidate:

```sh
.agents/skills/push-and-release/scripts/verify.sh
```

Or run the steps directly from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
python3 -m unittest discover -s DevUtils -p 'test_*.py'
cargo build --locked --release
python3 DevUtils/package.py
python3 DevUtils/validate_package.py dist/MissileCommand-<version>-linux-x64.zip
```

The window smoke check needs Xvfb, xauth, xdotool, and x11-utils. It always creates its
own private display, so it is safe to run on a desktop:

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
`target/` output.

## 4. Push and watch CI

```sh
git push origin main
run_id=$(gh run list --branch main --commit "$(git rev-parse HEAD)" --limit 1 \
  --json databaseId --jq '.[0].databaseId')
gh run watch "$run_id" --exit-status
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
2. Rebuild so `Cargo.lock` records the new version, then rerun the local gate.
3. Commit and push the bump to `main` and wait for CI to pass.
4. Create and push an annotated tag:

   ```sh
   git tag -a v<version> -m "Missile Command v<version>"
   git push origin v<version>
   ```

5. Watch the release workflow (`release.yml`) for that tag:

   ```sh
   run_id=$(gh run list --workflow release.yml \
     --commit "$(git rev-parse "v<version>^{commit}")" --limit 1 \
     --json databaseId --jq '.[0].databaseId')
   gh run watch "$run_id" --exit-status
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
- Report exactly what was run, what passed, and what was skipped. Link the workflow run
  or release URL so the user can check it.
