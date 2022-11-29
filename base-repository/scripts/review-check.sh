#!/usr/bin/env bash

# this script checks that there are no findings or observations in the to-review folder before merging

check=0
echo "Checking that the to-review files were moved to accepted or rejected"

for file in notes/**/**/to-review/**
do
    if test -f "$file"; then
        filename=$(basename $file)
        extension="${filename##*.}"
        if [ $extension = "md" ]; then
            echo "$file has not been accepted or rejected"
            check=1
        fi
    fi
done
if [ $check = 1 ]; then
    echo "Please move your pending to-review to accepted or rejected"
    exit 1
fi
printf "All to-review files correctly updated \n\n"