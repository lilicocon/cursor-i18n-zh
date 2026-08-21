---
name: release
description: >
  Release this repo's GitHub workbench version. Use when the user says 发版,
  推新版本, 发布, ship a release, tag a version, or bump v0.x. Read this skill
  first and finish every step; do not invent a release process.
---

# Release

This is the only release playbook. Read it once, then execute in order. A step is done only when its criterion is true.

## 1. Scope

Use `origin/main` plus only the feature branches the user named. Unnamed draft PRs stay out.

Done when: you can list the included branch names and the excluded ones.

## 2. Version

```bash
git fetch --tags origin
git tag --list 'v*' --sort=-v:refname | head
```

Next patch (`0.4.7` → `0.4.8`) unless the user said minor or major. Write OLD and NEW down.

Done when: NEW is `x.y.z` and `git rev-parse -q --verify refs/tags/vNEW` is empty.

## 3. Branch

From `origin/main`, create `cursor/release-<digits>-<agent-suffix>` (Cloud Agent suffix from the run instructions). Merge each named feature branch.

Done when: `git log origin/main..HEAD --oneline` is exactly the work that should ship, plus later release commits.

## 4. Bump

```bash
node scripts/bump-version.js NEW
```

Then replace every `（发版时填写）` stub:

- `CHANGELOG.md` — real bullets under `## [NEW]`
- `.github/releases/vNEW.md` — real `## 本版` (or topic headings). Keep `## 下载`, `## 已保留的安全能力`, `## 使用提醒`. Rewrite those standing sections only when the facts changed.

The file list lives in `scripts/bump-version.js`. Do not invent extra versioned files. Do not rewrite example exe names in `self_update.rs` tests.

Done when: `node scripts/bump-version.js --check` prints `version NEW is synchronized`.

## 5. Verify

```bash
npm test
```

If `dict/` or `src/engine.js` is in the release, also `npm run dict-check`.

Done when: those commands exit 0.

## 6. Commit and push

Commit `chore(release): vNEW`. Push the branch.

Done when: `git status -sb` shows the branch in sync with origin.

## 7. Tag

The tag is what publishes. `.github/workflows/build.yml` job `release` runs only on `refs/tags/v*`, and it fails if the tag is not `v` + `package.json` version. Create an annotated tag on the release commit:

```bash
git tag -a vNEW -m "vNEW"
git push origin vNEW
```

Use git tag push. Leave GitHub Release asset upload to the workflow.

Done when: `git ls-remote --tags origin refs/tags/vNEW` shows the tag.

## 8. Pull request

Open or update a PR vs `main` with ManagePullRequest (not `gh` write). Title: `Release vNEW: <one line>`. Body: what shipped, tag URL, actions URL. If the user asked to 发版/ship, create it ready (not draft).

Done when: the PR URL exists.

## 9. Assets

Wait for the tag workflow. The release page must have every file named in `.github/releases/vNEW.md` plus `SHA256SUMS.txt` and `SHA256SUMS-macos.txt`.

Done when: the `release` job succeeded and those assets are on https://github.com/lilicocon/cursor-i18n-zh/releases/tag/vNEW

## 10. Hand-off

Tell the user:

- merge the PR to `main` (the agent does not merge)
- old workbenches must install this version once by hand before in-app update works
- if i18n shipped: re-apply 汉化 after installing the new workbench

Done when: that message is sent and the tag workflow is linked.
