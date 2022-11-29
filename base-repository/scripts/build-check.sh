#!/usr/bin/env bash

# this script checks that there are no findings or observations in the to-review folder before merging

BUILD_CHECK_TEMP="build_check_temp"

HTML_FLAG=0
scripts/build-report.sh
mkdir $BUILD_CHECK_TEMP
scripts/output.sh $BUILD_CHECK_TEMP

# if different then HTML_FLAG = 1
cmp --silent $BUILD_CHECK_TEMP/report.html report/report.html || HTML_FLAG=1

if [ $HTML_FLAG = 1 ]; then
    echo "HTML report is not updated"
    exit 1
fi

printf "report folder is up to date \n\n"

rm -rf $BUILD_CHECK_TEMP