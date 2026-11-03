#!/bin/sh
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix=${PREFIX:-"$HOME/.local"}
vst3_dir=${VST3_DIR:-"$HOME/.vst3"}
data_home=${XDG_DATA_HOME:-"$HOME/.local/share"}
data_dir="$data_home/nirudrums"
bin_dir="$prefix/bin"

fail() {
    printf 'NIRUDRUMS install failed: %s\n' "$1" >&2
    exit 1
}

[ -x "$here/NIRUDRUMS" ] || fail "standalone executable is missing"
[ -d "$here/NIRUDRUMS.vst3" ] || fail "VST3 bundle is missing"
[ -f "$here/kits/MERCY-80/kit.json" ] || fail "MERCY-80 kit is missing"
[ -f "$here/assets/nirudrums.svg" ] || fail "application icon is missing"
[ -f "$here/packaging/nirudrums.desktop" ] || fail "desktop entry is missing"

install -d "$bin_dir" "$vst3_dir" "$data_dir/kits" "$data_dir/presets" "$data_dir/docs"
install -d "$data_home/applications" "$data_home/icons/hicolor/scalable/apps"
install -m 755 "$here/NIRUDRUMS" "$bin_dir/nirudrums"

rm -rf "$vst3_dir/NIRUDRUMS.vst3"
cp -a "$here/NIRUDRUMS.vst3" "$vst3_dir/NIRUDRUMS.vst3"

rm -rf "$data_dir/kits/MERCY-80"
cp -a "$here/kits/MERCY-80" "$data_dir/kits/MERCY-80"
for preset in "$here"/presets/*.json; do
    [ -f "$preset" ] && install -m 644 "$preset" "$data_dir/presets/"
done
install -m 644 "$here/assets/nirudrums.svg" "$data_home/icons/hicolor/scalable/apps/nirudrums.svg"
sed "s|@EXEC@|$bin_dir/nirudrums|g" "$here/packaging/nirudrums.desktop" > "$data_home/applications/nirudrums.desktop"
chmod 644 "$data_home/applications/nirudrums.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data_home/applications" >/dev/null 2>&1 || true
fi
for document in README.md CHANGELOG.md KNOWN_LIMITATIONS.md TEST_RESULTS.md RUNTIME-REQUIREMENTS.md; do
    if [ -f "$here/$document" ]; then
        install -m 644 "$here/$document" "$data_dir/docs/$document"
    fi
done

"$bin_dir/nirudrums" --selftest >/dev/null || fail "installed binary failed its self-test"

printf 'NIRUDRUMS installed.\n'
printf '  standalone: %s\n' "$bin_dir/nirudrums"
printf '  VST3:       %s\n' "$vst3_dir/NIRUDRUMS.vst3"
printf '  kit/presets:%s\n' "$data_dir"
printf '  launcher:   %s\n' "$data_home/applications/nirudrums.desktop"
printf 'No root permissions were used.\n'
