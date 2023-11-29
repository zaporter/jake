#!/usr/bin/env bash
set -euo pipefail

cd `dirname $0`
python3 -m axolotl.cli.merge_lora mistraltr.yml --lora_model_dir="./mistral_out" --load_in_8bit=False --load_in_4bit=False
