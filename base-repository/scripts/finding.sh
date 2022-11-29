#!/usr/bin/env bash
BRANCH_NAME=$(git symbolic-ref -q HEAD)
BRANCH_NAME=${BRANCH_NAME##refs/heads/}
BRANCH_NAME=${BRANCH_NAME:-HEAD}

TEMPLATE=""
SEVERITY=$1

if [ $SEVERITY == 1 ] || [ $SEVERITY == 2 ] || [ $SEVERITY == 3 ]; then
    TEMPLATE="finding"
elif [ $SEVERITY == 4 ]; then
    TEMPLATE="informational"
else
    echo "SEVERITY="$SEVERITY""
    echo "SEVERITY should be 1 (high), 2 (medium), 3 (low) or 4 (informational)"
    exit 1
fi

FILE_PATH=notes/$BRANCH_NAME/findings/to-review
BASENAME=$(basename $2)
FILENAME="${BASENAME%.*}"
FILE=$FILE_PATH/$SEVERITY-$FILENAME.md

if [ ! -d $FILE_PATH ]; then
    echo "$FILE_PATH folder does not exist, aborting"
    exit 1
elif test -f "$FILE"; then
    echo "$FILE already exist, aborting"
    exit 1
else
    echo "Creating $FILE file"
    cp templates/$TEMPLATE.md $FILE
fi
