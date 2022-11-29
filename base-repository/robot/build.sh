#!/usr/bin/env bash

docker build -t reporting .

# docker run --rm -v $(pwd):/data/ -w /data/ reporting:latest mkpdf temp/0*.md
docker run --rm -v $(pwd):/data/ -w /data/ reporting:latest mkhtml temp/0*.md

rm -rf temp/0*.md
rm -rf figures/*