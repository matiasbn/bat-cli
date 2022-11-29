#!/usr/bin/env bash

# make sure to add `node-modules/.bin` to your PATH
# this is usually in ~ or in the current directory
PATH=$PATH:/opt/reporting-tool/node_modules/.bin

#highlight_style=pygments
#highlight_style=tango
#highlight_style=espresso
#highlight_style=zenburn
#highlight_style=kate
#highlight_style=monochrome
#highlight_style=breezedark
highlight_style=haddock

cp -rnv /opt/reporting-tool/data/* ./

/opt/reporting-tool/generate_content_variables.sh

pandoc \
 --from markdown+fenced_divs+tex_math_dollars+citations \
 --filter pandoc-mustache \
 --filter mermaid-filter \
 --filter pandoc-imagine \
 --filter filters/findings_summary_html.sh \
 --self-contained \
 --standalone \
 --table-of-contents \
 --toc-depth 4 \
 --number-sections \
 --template templates/ks/template.html \
 --css templates/ks/template.css \
 --highlight-style ${highlight_style} \
 --mathjax \
 --variable customer:mustache \
 --metadata date="`date -u '+%d %B %Y'`"\
 -C \
 --csl style.csl \
 -o output.html \
 $@ \
