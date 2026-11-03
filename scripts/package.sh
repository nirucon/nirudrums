#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
name="nirudrums-$version-linux-x86_64"
stage="dist/$name"

cargo xtask bundle nirudrums --release
rm -rf "$stage"
mkdir -p "$stage"
cp target/bundled/NIRUDRUMS "$stage/NIRUDRUMS"
cp -a target/bundled/NIRUDRUMS.vst3 "$stage/NIRUDRUMS.vst3"
cp -a kits "$stage/kits"
cp -a presets "$stage/presets"
cp -a assets "$stage/assets"
cp -a packaging "$stage/packaging"
cp README.md CHANGELOG.md VERSION LICENSE "$stage/"
cp -a docs "$stage/docs"
cp KNOWN_LIMITATIONS.md TEST_RESULTS.md RUNTIME-REQUIREMENTS.md "$stage/"
cp scripts/install.sh scripts/uninstall.sh "$stage/"
chmod +x "$stage/install.sh" "$stage/uninstall.sh" "$stage/NIRUDRUMS"

"$stage/NIRUDRUMS" --selftest >/dev/null
(cd dist && zip -qr "$name.zip" "$name")
sha256sum "dist/$name.zip" > "dist/$name.zip.sha256"
printf 'Created %s\n' "dist/$name.zip"
cat "dist/$name.zip.sha256"
