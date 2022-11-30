#!/usr/bin/env bash

if [ $PREVIEW ]; then
    mv -f robot/output.html report/preview.html

else
    mv -f robot/output.html report/report.html
fi
