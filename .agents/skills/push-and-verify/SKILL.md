---
name: push-and-verify
description: Commit changes, push them to GitHub, and verify that the triggered GitHub Actions workflow succeeds. Use whenever the user asks to commit and push, push a branch, check or wait for CI, fix a failing workflow run, or confirm that a push is green. Use it before claiming a change is pushed or CI passed. For a tagged release, use push-and-release after this workflow.
---

# Push and verify

A push is not finished when `git push` returns. It is finished when the workflow run for
that exact commit succeeds. Watch the run, read the failed logs if necessary, and only
then report success.

## 1. Inspect before changing the index

```sh
git status --short
git branch --show-current
git remote -v
git fetch origin
git status -sb
git diff
```

- Confirm the branch is the one the user named (normally `main`) and is not behind or
  diverged from its upstream.
- Confirm every changed path belongs to the task. Do not commit `.env` files,
  credentials, ROMs, generated `dist/` archives, `target/`, or `data/`.
- If `gh auth status` fails, stop and report it. Do not fall back to pushing without
  watching the workflow.

## 2. Run the local gate

From the repository root, run the bundled script. It mirrors CI, including the
release build and package validation:

```sh
.agents/skills/push-and-verify/scripts/verify.sh
```

The script runs:

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
driver (Mesa lavapipe is sufficient). It creates its own private display:

```sh
python3 DevUtils/window_smoke.py --binary target/release/missile-command
```

Fix failures before continuing. If a check cannot run in the current environment, name
the skipped check and do not describe the run as fully verified.

## 3. Commit

Stage only the intended paths:

```sh
git add <paths>
git commit -m "<imperative summary>"
```

Match the repository's existing style: a short sentence-case subject, with a body when
the reason is not obvious. Do not use `--no-verify` unless the user explicitly asks.
Do not amend a commit that has already been pushed; add a new commit instead.

## 4. Push and watch the workflow

```sh
git push origin <branch>
.agents/skills/push-and-verify/scripts/watch-workflow.sh HEAD ci.yml
```

`watch-workflow.sh` accepts an optional commit and workflow name. Passing `ci.yml`
avoids matching a release workflow that only runs for tags:

```sh
.agents/skills/push-and-verify/scripts/watch-workflow.sh [commit] [workflow]
```

The script:

- waits for a workflow run associated with the commit,
- watches each matching run,
- prints failed logs and exits non-zero if any run fails,
- treats a completed successful run as a pass.

If the workflow fails, read the failed logs:

```sh
gh run view <run-id> --log-failed
```

Fix the cause, commit again, and repeat from step 2. If the push is rejected because the
remote moved, fetch and rebase or merge. Never force-push `main`.

## 5. Report

Report:

- the branch and commit SHA,
- the workflow run URL,
- whether the run succeeded,
- which local checks ran and which were skipped.

Do not claim the push is green until the watched run succeeds for the pushed commit.

## Safety rules

- Never force-push `main` or rewrite published history.
- Do not create tags or releases here; use `push-and-release` after the push is green.
- If the user asked only to commit, do not push.
- Do not stage unrelated working-tree changes.
- Do not claim success from a cached, queued, or failed run.
