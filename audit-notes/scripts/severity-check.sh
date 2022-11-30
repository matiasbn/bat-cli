#!/usr/bin/env bash

# this script checks that the accepted findings are correctly appended with severity code
# High = 1, Medium = 2, Low = 3, Medium = 4

echo "Checking that the accepted findings were correctly appended with severity level"
check=0

for file in notes/**/**/accepted/**
do
    if test -f "$file"; then
        filename=$(basename $file)
        extension="${filename##*.}"
        if [ $extension = "md" ]; then
            severity=${filename:0:1}
            # if [ $severity -gt 4 ] || [ $severity -eq 0 ]; then
            if ! [[ $severity =~ ^-?[1-4]+$ ]]; then
                echo "$file severity has not been correclty assigned, severity should be a number between 1 and 4"
                check=1
            fi
        fi
    fi
done

printf "\n"

if [ $check = 1 ]; then
    echo "Please assign a severity by appending a number to the finding file followed by a -"
    echo "1 = High severity, 2 = Medium severity, 3 = Low severity, 4 = Informational "
    echo "example: 1-name_of_finding.md is a High severity finding"
    printf "\n"
    exit 1
fi

printf "All accepted findings correctly tagged \n\n"