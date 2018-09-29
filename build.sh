#!/usr/bin/env sh

set -e

cd kernel
cargo +nightly xbuild --target ../targets/x86_64-unknown-none.json
cd ..

cd loader
cargo +nightly xbuild --target ../targets/i686-unknown-none.json
cd ..
