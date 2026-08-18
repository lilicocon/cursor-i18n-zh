#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_PATH="${1:-build/cursor-release-macos.json}"
BASELINE_PATH="${2:-compat/cursor-stable-macos.json}"
FORCE="${FORCE_COMPAT_CHECK:-false}"
CHANNEL="stable"
PLATFORM="darwin-universal"
API_URL="https://cursor.com/api/download?platform=${PLATFORM}&releaseTrack=${CHANNEL}"

resolve_repo_path() {
  local value="$1"
  if [[ "$value" = /* ]]; then
    printf '%s\n' "$value"
  else
    printf '%s\n' "$ROOT/$value"
  fi
}

OUTPUT_FILE="$(resolve_repo_path "$OUTPUT_PATH")"
BASELINE_FILE="$(resolve_repo_path "$BASELINE_PATH")"
mkdir -p "$(dirname "$OUTPUT_FILE")"
API_FILE="$(dirname "$OUTPUT_FILE")/cursor-download-api-macos.json"

curl -fsSL "$API_URL" -o "$API_FILE"

PARSED="$(
  API_FILE="$API_FILE" OUTPUT_FILE="$OUTPUT_FILE" CHANNEL="$CHANNEL" PLATFORM="$PLATFORM" node -e '
const fs = require("fs");
const apiFile = process.env.API_FILE;
const outputFile = process.env.OUTPUT_FILE;
const channel = process.env.CHANNEL;
const platform = process.env.PLATFORM;
const response = JSON.parse(fs.readFileSync(apiFile, "utf8"));
const version = String(response.version || "");
const commit = String(response.commitSha || "");
const downloadUrl = String(response.downloadUrl || "");
if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error("Cursor API returned an invalid version: " + version);
}
if (!/^[0-9a-f]{40}$/.test(commit)) {
  throw new Error("Cursor API returned an invalid commit: " + commit);
}
const uri = new URL(downloadUrl);
if (uri.protocol !== "https:" || uri.hostname !== "downloads.cursor.com") {
  throw new Error("Cursor API returned an unexpected download URL: " + downloadUrl);
}
const release = {
  schema: 1,
  channel,
  platform,
  version,
  commit,
  downloadUrl,
  checkedAt: new Date().toISOString(),
};
fs.writeFileSync(outputFile, JSON.stringify(release, null, 2) + "\n");
process.stdout.write(version + "\t" + commit + "\t" + downloadUrl + "\n");
'
)"

VERSION="${PARSED%%$'\t'*}"
REST="${PARSED#*$'\t'}"
COMMIT="${REST%%$'\t'*}"
DOWNLOAD_URL="${REST#*$'\t'}"

BASELINE_VERSION=""
BASELINE_COMMIT=""
if [[ -f "$BASELINE_FILE" ]]; then
  BASELINE_INFO="$(
    BASELINE_FILE="$BASELINE_FILE" node -e '
const fs = require("fs");
const baseline = JSON.parse(fs.readFileSync(process.env.BASELINE_FILE, "utf8"));
const version = String(baseline.version || "");
const commit = String(baseline.releaseCommit || baseline.commit || "");
process.stdout.write(version + "\t" + commit + "\n");
'
  )"
  BASELINE_VERSION="${BASELINE_INFO%%$'\t'*}"
  BASELINE_COMMIT="${BASELINE_INFO#*$'\t'}"
fi

CHANGED="false"
if [[ "$FORCE" == "true" || "$VERSION" != "$BASELINE_VERSION" || "$COMMIT" != "$BASELINE_COMMIT" ]]; then
  CHANGED="true"
fi

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  {
    echo "changed=$CHANGED"
    echo "version=$VERSION"
    echo "commit=$COMMIT"
    echo "download_url=$DOWNLOAD_URL"
  } >> "$GITHUB_OUTPUT"
fi

echo "Cursor stable: $VERSION (${COMMIT:0:8})"
if [[ -n "$BASELINE_COMMIT" ]]; then
  echo "Recorded baseline: $BASELINE_VERSION (${BASELINE_COMMIT:0:8})"
else
  echo "Recorded baseline: none"
fi
echo "Compatibility build required: $CHANGED"
