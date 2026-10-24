---
name: push-and-release
description: Tag and publish a versioned GitHub release, then verify the published asset. Use only when the user explicitly asks to cut, ship, publish, or tag a release. Complete the push-and-verify workflow first; ordinary commits and pushes belong to that skill instead.
---

# Tag and publish a release

A release is public and awkward to undo. Publish only from a clean tree, a green `main`,
and a version that matches the tag. The push-and-verify workflow covers committing,
pushing, and local checks; this skill covers the release-specific steps.

## Preconditions

- The working tree is clean and `main` is pushed.
- `.agents/skills/push-and-verify/SKILL.md` has completed successfully for the current
  `HEAD`.
- The user explicitly asked for a release or tag.

## 1. Bump the version

1. Update `version` in `Cargo.toml`.
2. Update version strings in README/docs examples.
3. Rebuild once without `--locked` so `Cargo.lock` records the new package version:

   ```sh
   cargo build --release
   ```

## 2. Run the local gate

```sh
.agents/skills/push-and-verify/scripts/verify.sh
```

This mirrors CI, builds the release archive, and validates it. Fix failures before
continuing.

## 3. Commit and push the version bump

```sh
git add Cargo.toml Cargo.lock <updated docs>
git commit -m "Release Missile Command v<version>"
git push origin main
.agents/skills/push-and-verify/scripts/watch-workflow.sh HEAD ci.yml
```

Do not create the tag until this workflow run succeeds.

## 4. Create and push the tag

The tag must match `package.version` in `Cargo.toml`; the release workflow rejects a
mismatch.

```sh
git tag -a v<version> -m "Missile Command v<version>"
git push origin v<version>
```

## 5. Watch the release workflow

Watch the release workflow for the tag commit:

```sh
.agents/skills/push-and-verify/scripts/watch-workflow.sh \
  "$(git rev-parse 'v<version>^{commit}')" release.yml
```

Do not report success from a queued or failed run. Read failures with:

```sh
gh run view <run-id> --log-failed
```

## 6. Verify the published asset

A green workflow is not proof that the correct archive was published.

```sh
gh release view v<version>
gh release download v<version> --pattern 'MissileCommand-*.zip' --dir /tmp/release-check
python3 DevUtils/validate_package.py \
  /tmp/release-check/MissileCommand-<version>-linux-x64.zip
```

Confirm the asset name, archive contents, launcher cover, executable permission, and
absence of ROMs, user data, and updater files. Report the release URL and asset name.

## Safety rules

- Never force-push `main`, move a published tag, or rewrite release history.
- Never publish from a dirty tree, a failing `main`, or a tag that disagrees with
  `Cargo.toml`.
- If the user only asked to push code, stop after push-and-verify; do not tag.
- Report exactly what ran, what passed, and what was skipped.
