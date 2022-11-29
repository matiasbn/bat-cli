#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
export REPORT_FORMAT=pdf
${SCRIPT_DIR}/findings_summary.py