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

# root
cargo set-version $NEXT_VERSION
git add Cargo.toml
pushd hannibal-derive
cargo set-version $NEXT_VERSION
git add Cargo.toml
popd

git commit -m "chore: bump version"
git tag v$NEXT_VERSION

convco changelog > CHANGELOG.md
git add CHANGELOG.md
git commit -m "chore: changelog"
