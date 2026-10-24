#!/usr/bin/env bash
# Watch GitHub Actions runs for a commit.
# Usage: watch-workflow.sh [commit] [workflow]
# With no commit, watches HEAD. With no workflow, watches every run for the commit.
set -euo pipefail

commit=$(git rev-parse "${1:-HEAD}")
workflow=${2:-}
runs=""

for _ in $(seq 1 30); do
    args=(run list --commit "$commit" --limit 20 --json databaseId,workflowName,status,conclusion,url)
    if [[ -n "$workflow" ]]; then
        args+=(--workflow "$workflow")
    fi
    runs=$(gh "${args[@]}" --jq '.[] | [.databaseId, .workflowName, .status, .conclusion, .url] | @tsv' 2>/dev/null || true)
    if [[ -n "$runs" ]]; then
        break
    fi
    sleep 2
done

if [[ -z "$runs" ]]; then
    echo "No GitHub Actions run found for commit $commit" >&2
    exit 1
fi

failed=0
while IFS=$'\t' read -r id name status conclusion url; do
    [[ -n "$id" ]] || continue

    if [[ "$status" == "completed" ]]; then
        if [[ "$conclusion" == "success" ]]; then
            printf 'PASS %s: %s\n' "$name" "$url"
        else
            printf 'FAIL %s (%s): %s\n' "$name" "${conclusion:-unknown}" "$url" >&2
            gh run view "$id" --log-failed >&2 || true
            failed=1
        fi
        continue
    fi

    printf 'Watching %s (%s): %s\n' "$name" "$status" "$url"
    if gh run watch "$id" --exit-status; then
        printf 'PASS %s: %s\n' "$name" "$url"
    else
        printf 'FAIL %s: %s\n' "$name" "$url" >&2
        gh run view "$id" --log-failed >&2 || true
        failed=1
    fi
done <<< "$runs"

if (( failed )); then
    exit 1
fi

echo "All workflow runs for $commit succeeded."
