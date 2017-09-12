#!/bin/bash
set -e
base_dir="$(dirname $0)"
configuration=${CONFIGURATION:-debug}
RUST_LOG=debug exec $base_dir/target/$configuration/vk-playground
