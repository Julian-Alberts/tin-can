#!/usr/bin/env bash
if ! [ -f "./Cargo.toml" ] || ! grep '^name = "tin-can"$' < Cargo.toml > /dev/null; then
    echo 'Script must be run from the tin-can root directory.' >&2
    exit 1
fi
mkdir test-data 2>/dev/null
mkdir test-data/alpine test-data/alpine-upper ./test-data/work ./test-data/root ./test-data/alpine-upper/put-old 2>/dev/null
wget -O test-data/alpine-minirootfs.tar.gz https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/alpine-minirootfs-3.21.0-x86_64.tar.gz
tar -C test-data -xzf alpine-minirootfs.tar.gz -C alpine
