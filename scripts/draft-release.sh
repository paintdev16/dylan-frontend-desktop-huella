#!/usr/bin/env bash
set -euo pipefail

asset_dir=$1
tag=${GITHUB_REF_NAME:?}
repo=${GITHUB_REPOSITORY:?}

if [[ ! $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo 'Expected a stable tag in the form vX.Y.Z.' >&2
  exit 1
fi

cd "$asset_dir"
openssl pkeyutl -verify -pubin -inkey DYLAN_SIGNING_PUBLIC_KEY.pem -rawin \
  -in SHA256SUMS.txt -sigfile SHA256SUMS.txt.sig
sha256sum --check --strict SHA256SUMS.txt

releases=$(gh api --paginate "repos/$repo/releases?per_page=100" --jq ".[] | select(.tag_name == \"$tag\") | [.id, .draft] | @tsv")
if [[ -z $releases ]]; then
  gh api "repos/$repo/git/ref/tags/$tag" >/dev/null
  releases=$(gh api --method POST "repos/$repo/releases" \
    -f tag_name="$tag" \
    -f name="$tag" \
    -F draft=true \
    -F generate_release_notes=true \
    -f make_latest=false \
    --jq '[.id, .draft] | @tsv')
fi
IFS=$'\t' read -r release_id draft <<< "$releases"
if [[ ! $release_id =~ ^[0-9]+$ || $draft != true ]]; then
  echo "The tag $tag does not have an editable draft. Leaving the release unchanged." >&2
  exit 1
fi

release="repos/$repo/releases/$release_id"
remote_assets=$(gh api --paginate "$release/assets?per_page=100" --jq '.[] | [.name, .digest] | @tsv')
for asset in *; do
  expected="sha256:$(sha256sum "$asset" | cut -d' ' -f1)"
  existing=$(awk -F '\t' -v name="$asset" '$1 == name { print $2 }' <<< "$remote_assets")
  if [[ $existing == "$expected" ]]; then
    continue
  fi
  if [[ -n $existing ]]; then
    echo "The draft already contains a different asset named $asset." >&2
    exit 1
  fi
  if [[ $(gh api "$release" --jq '.draft') != true ]]; then
    echo 'The draft was published while preparing its assets.' >&2
    exit 1
  fi
  remote_digest=$(gh api --method POST \
    -H 'Content-Type: application/octet-stream' --input "$asset" \
    "https://uploads.github.com/$release/assets?name=$asset" --jq '.digest')
  if [[ $remote_digest != "$expected" ]]; then
    echo "GitHub did not confirm the checksum for $asset." >&2
    exit 1
  fi
done

if [[ $(gh api "$release" --jq '.draft') != true ]]; then
  echo 'Expected the release to remain a draft.' >&2
  exit 1
fi
gh api "$release" --jq '.html_url'
