#!/bin/bash

set -o allexport
source .env
set +o allexport

cargo run -- --prefix=CADDY_ --debug --schema ./caddy-schema.json
