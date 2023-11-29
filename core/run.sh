#!/usr/bin/env bash

cd `dirname $0`
accelerate launch -m main
