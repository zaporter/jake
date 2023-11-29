#!/usr/bin/env bash
set -euo pipefail

echo "starting"
cd `dirname $0`
rm -rf ./last_run_prepared 
rm -rf ./mistral_out 
accelerate launch -m axolotl.cli.train mistralpretrain.yml
