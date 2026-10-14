#!/usr/bin/env sh
# Off-umbrella proof for oxideav-meta.
#
# Packages this crate exactly as `cargo publish` would, unpacks the
# `.crate` into a directory outside any cargo workspace (so no umbrella
# `[patch.crates-io]` applies), then builds and tests the consumer
# template in `consumer/` against the unpacked copy with a small
# feature subset resolved from crates.io. Proves that `build.rs` reads
# the NORMALIZED manifest cargo ships (every dependency as its own
# `[dependencies.oxideav-x]` table) and that `register_all` is populated
# for a crates.io consumer. CI runs it as the `off-umbrella` job.
#
#   usage: tests/off_umbrella/run.sh <work-dir>
#
# <work-dir> must be outside every cargo workspace (CI: $RUNNER_TEMP).
# Set META_CRATE_FILE=/path/to/oxideav-meta-X.Y.Z.crate to test an
# already-packaged archive (e.g. a published release) instead of
# packaging the checkout.
set -eu

CRATE_DIR=$(cd "$(dirname "$0")/../.." && pwd)
WORK=${1:?usage: run.sh <work-dir outside any cargo workspace>}
mkdir -p "$WORK"
WORK=$(cd "$WORK" && pwd)
case "$WORK" in
    "$CRATE_DIR"/*) echo "work dir must be outside the crate / umbrella checkout" >&2; exit 2 ;;
esac

if [ -n "${META_CRATE_FILE:-}" ]; then
    CRATE_FILE=$META_CRATE_FILE
else
    echo "== packaging $CRATE_DIR"
    (cd "$CRATE_DIR" && cargo package --no-verify --allow-dirty --target-dir "$WORK/package-target")
    CRATE_FILE=$(ls "$WORK"/package-target/package/oxideav-meta-*.crate | head -n 1)
fi
echo "== crate archive: $CRATE_FILE"

rm -rf "$WORK/unpacked" "$WORK/consumer"
mkdir -p "$WORK/unpacked" "$WORK/consumer"
tar -xzf "$CRATE_FILE" -C "$WORK/unpacked"
META_PATH=$(ls -d "$WORK"/unpacked/oxideav-meta-*/ | head -n 1)
META_PATH=${META_PATH%/}
echo "== unpacked manifest shape:"
grep -c '^\[dependencies\.oxideav-' "$META_PATH/Cargo.toml" | sed 's/^/   per-dependency tables: /'

cp -R "$CRATE_DIR/tests/off_umbrella/consumer/." "$WORK/consumer/"
sed "s|@META_PATH@|$META_PATH|" "$WORK/consumer/Cargo.toml.in" > "$WORK/consumer/Cargo.toml"
rm "$WORK/consumer/Cargo.toml.in"

echo "== building + testing the consumer against the unpacked crate"
export CARGO_TARGET_DIR="$WORK/target"
cd "$WORK/consumer"
cargo test ${CARGO_BUILD_JOBS:+-j "$CARGO_BUILD_JOBS"} -- --nocapture --test-threads=1
echo "== off-umbrella proof PASSED"
