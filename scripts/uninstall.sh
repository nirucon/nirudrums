#!/bin/sh
set -eu

prefix=${PREFIX:-"$HOME/.local"}
vst3_dir=${VST3_DIR:-"$HOME/.vst3"}
data_home=${XDG_DATA_HOME:-"$HOME/.local/share"}
data_dir="$data_home/nirudrums"
bin_dir="$prefix/bin"

rm -f "$bin_dir/nirudrums"
rm -f "$data_home/applications/nirudrums.desktop"
rm -f "$data_home/icons/hicolor/scalable/apps/nirudrums.svg"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data_home/applications" >/dev/null 2>&1 || true
fi
rm -rf "$vst3_dir/NIRUDRUMS.vst3"
rm -rf "$data_dir/kits/MERCY-80"
for preset in mercy-80-classic mercy-80-dry mercy-80-gated mercy-80-heavy mercy-80-industrial; do
    rm -f "$data_dir/presets/$preset.json"
done
for document in README.md CHANGELOG.md KNOWN_LIMITATIONS.md TEST_RESULTS.md RUNTIME-REQUIREMENTS.md; do
    rm -f "$data_dir/docs/$document"
done

if [ "${1:-}" = "--purge" ]; then
    rm -rf "$data_dir"
    printf 'NIRUDRUMS removed, including user data (--purge).\n'
else
    printf 'NIRUDRUMS binaries and factory assets removed. User projects and presets were kept.\n'
fi
