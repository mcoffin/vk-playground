#!/bin/bash
set -e
base_dir="$(dirname $0)"
configuration=${CONFIGURATION:-debug}
RUST_LOG=${RUST_LOG:-trace}
RUST_LOG=$RUST_LOG exec $base_dir/target/$configuration/vk-playground
