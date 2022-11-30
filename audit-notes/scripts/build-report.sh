#!/usr/bin/env bash

REPORT_PATH=report

# clean
rm -rf $REPORT_PATH/figures/*
rm -rf $REPORT_PATH/02_findings.md
rm -rf temp/findings/*.md
rm -rf temp/*.md

# findings and observations
cp templates/02_findings.md temp
cp -rp notes/**/findings/accepted/*.md temp/findings/ 2>/dev/null
if [ $PREVIEW == true ]; then
    echo "Generating preview with to-review findings"
    cp -rp notes/**/findings/to-review/*.md temp/findings/ 2>/dev/null
fi

for file in temp/findings/*; do
    if test -f "$file"; then
        filename=$(basename $file)
        echo "inserting $filename finding into the findings file"
        echo "" >>$file
        echo "" >>$file
    fi
done

echo ""

cat temp/findings/* >>temp/02_findings.md 2>/dev/null

# # update images pattern to match local figures files
sed -i '' "s|./../../figures/|figures/|g" temp/02_findings.md
sed -i '' "s|../../figures/|figures/|g" temp/02_findings.md

# clone
cp temp/02_findings.md report/
cp report/0*.md robot/temp
cp -r notes/**/figures/ report/figures
cp -r notes/**/figures/ robot/figures

# clean
rm -rf temp/*.md
rm -rf temp/findings/*.md

# # generate report
cd robot
./build.sh

cd ..
scripts/output.sh
