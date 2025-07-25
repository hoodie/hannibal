#!/usr/bin/env bash

set -xe

current_branch=`git rev-parse --abbrev-ref HEAD`
release_branch="main"

if [ "${current_branch}" != "${release_branch}" ]; then
echo can only bump version on $release_branch
exit 1
fi

which convco
which cargo-set-version

NEXT_VERSION=`convco version --bump HEAD`

cargo set-version $NEXT_VERSION
cargo set-version $NEXT_VERSION --manifest-path hannibal-derive/Cargo.toml
echo "now update the version of hannibal_derive in Cargo.toml"
read
git add Cargo.toml
git add Cargo.lock
git add hannibal-derive/Cargo.toml
git add hannibal-derive/Cargo.lock

git commit -m "chore: bump version to ${NEXT_VERSION}"
git tag v${NEXT_VERSION}

convco changelog > CHANGELOG.md
git add CHANGELOG.md
git commit -m "chore: changelog for ${NEXT_VERSION}"
