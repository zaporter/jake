#!/usr/bin/env bash
set -euo pipefail

echo "starting"
cd `dirname $0`
rm -rf ./last_run_prepared 
accelerate launch -m axolotl.cli.train mistraltr.yml
