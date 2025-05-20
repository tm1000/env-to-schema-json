#!/bin/bash

set -o allexport
source .env
set +o allexport

cat ./caddy-schema.json | cargo run -- --prefix=CADDY_ --debug
