#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
export REPORT_FORMAT=html
${SCRIPT_DIR}/findings_summary.py