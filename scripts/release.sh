#!/usr/bin/env bash
#
# release.sh — Bump the Zero project version, commit, tag, and push to all remotes.
#
# Usage:
#   ./scripts/release.sh 0.0.14
#   ./scripts/release.sh 0.0.14-beta --dry-run
#   ./scripts/release.sh 0.0.14 --no-push
#   ./scripts/release.sh 0.0.14 -m "custom commit message"
set -euo pipefail

# ---------- helpers ----------
usage() {
    sed -n '2,9p' "$0"
    exit 1
}

# ---------- parse args ----------
DRY_RUN=false
NO_PUSH=false
MESSAGE=""

VERSION=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --no-push) NO_PUSH=true; shift ;;
        -m) MESSAGE="$2"; shift 2 ;;
        --help|-h) usage ;;
        -*)
            echo "Unknown option: $1"
            usage
            ;;
        *)
            if [[ -z "$VERSION" ]]; then
                VERSION="$1"
            else
                echo "Version already set to '$VERSION', unexpected extra argument: $1"
                usage
            fi
            shift
            ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    echo "Error: version argument is required (e.g. 0.0.14)"
    usage
fi

# validate format
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo "Error: invalid version format. Expected X.Y.Z or X.Y.Z-suffix. Got: '$VERSION'"
    exit 1
fi

TAG_NAME="v${VERSION}"
MESSAGE="${MESSAGE:-release: v${VERSION}}"

# ---------- cd to repo root ----------
cd "$(dirname "$0")/.."
REPO_ROOT="$(pwd)"

# ---------- prerequisites ----------
if [[ ! -f Cargo.toml ]]; then
    echo "Error: Cargo.toml not found in $REPO_ROOT"
    exit 1
fi

if ! git diff-index --quiet HEAD --; then
    echo "Error: working tree is not clean. Commit or stash changes before releasing."
    exit 1
fi

CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
echo -e "\033[36mCurrent branch: ${CURRENT_BRANCH}\033[0m"

# ---------- gather remotes ----------
mapfile -t REMOTES < <(git remote)
if [[ ${#REMOTES[@]} -eq 0 ]]; then
    echo "Error: no git remotes configured."
    exit 1
fi
echo -e "\033[36mRemotes: ${REMOTES[*]}\033[0m"

# ---------- read current version ----------
CURRENT_VERSION=$(awk '/^\[workspace\.package\]/{found=1} found && /^version[[:space:]]*=[[:space:]]*"/{match($0, /"[^"]+"/); print substr($0, RSTART+1, RLENGTH-2); exit}' Cargo.toml)
CURRENT_VERSION="${CURRENT_VERSION:-unknown}"

echo -e "\033[33mCurrent version: ${CURRENT_VERSION} -> New version: ${VERSION}\033[0m"
echo -e "\033[33mTag: ${TAG_NAME}\033[0m"

if $DRY_RUN; then
    echo -e "\033[32m[DRY RUN] Would update Cargo.toml, commit, tag $TAG_NAME, push to: ${REMOTES[*]}\033[0m"
    exit 0
fi

# ---------- confirm ----------
read -r -p "Proceed with release v${VERSION}? [y/N] " CONFIRM
if [[ ! "$CONFIRM" =~ ^[yY] ]]; then
    echo "Aborted."
    exit 0
fi

# ---------- update Cargo.toml ----------
echo -e "\033[36mUpdating version in Cargo.toml...\033[0m"
# Replace only the version line inside [workspace.package]
sed -i "/^\[workspace\.package\]/,/^\[workspace\.dependencies\]/ s/^\(version[[:space:]]*=[[:space:]]*\"\)[^\"]*\(\".*\)/\1${VERSION}\2/" Cargo.toml

# verify it changed
NEW_VERSION=$(awk '/^\[workspace\.package\]/{found=1} found && /^version[[:space:]]*=[[:space:]]*"/{match($0, /"[^"]+"/); print substr($0, RSTART+1, RLENGTH-2); exit}' Cargo.toml)
if [[ "$NEW_VERSION" != "$VERSION" ]]; then
    echo "Error: failed to update version in Cargo.toml (expected '$VERSION', got '$NEW_VERSION')"
    exit 1
fi
echo -e "\033[32m  version = \"${VERSION}\"\033[0m"

# ---------- commit ----------
echo -e "\033[36mCommitting...\033[0m"
git add Cargo.toml
git commit -m "$MESSAGE"
echo -e "\033[32m  commit: ${MESSAGE}\033[0m"

# ---------- tag ----------
echo -e "\033[36mCreating tag ${TAG_NAME}...\033[0m"
git tag -a "$TAG_NAME" -m "$MESSAGE"
echo -e "\033[32m  tag: ${TAG_NAME}\033[0m"

# ---------- push ----------
if ! $NO_PUSH; then
    for remote in "${REMOTES[@]}"; do
        echo -e "\033[36mPushing to ${remote}...\033[0m"
        git push "$remote" "$CURRENT_BRANCH"
        git push "$remote" "$TAG_NAME"
        echo -e "\033[32m  ${remote}: pushed ${CURRENT_BRANCH} + ${TAG_NAME}\033[0m"
    done
    echo -e "\033[32mDone. Version ${VERSION} released and pushed.\033[0m"
else
    echo -e "\033[33mSkipped push (--no-push). Commit and tag are local only.\033[0m"
fi
