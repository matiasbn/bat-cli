#!/usr/bin/env bash

BRANCH_NAME=$(git symbolic-ref -q HEAD)
BRANCH_NAME=${BRANCH_NAME##refs/heads/}
BRANCH_NAME=${BRANCH_NAME:-HEAD}

FILE_PATH=notes/$BRANCH_NAME/code-overhaul
echo $FILE_PATH
BASENAME=$(basename $1)
FILENAME="${BASENAME%.*}"
FILE=$FILE_PATH/$FILENAME.md

if [ ! -d $FILE_PATH ]; then
    echo "$FILE_PATH folder does not exist, aborting"
    exit 1
elif test -f "$FILE"; then
    echo "$FILE already exist, aborting"
    exit 1
else
    echo "Creating $FILE file"
    cp templates/code-overhaul.md $FILE
fi
