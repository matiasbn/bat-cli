#!/usr/bin/env bash

wd=`mktemp -d`
cwd=`pwd`

echo "[-] Working in $wd"
echo "[-] Copying base files to $wd"
cp -r /opt/reporting-tool/data/* $wd
echo "[-] Copying user data from $cwd to $wd"
cp -r ./* $wd
echo "[-] cd to $wd"
cd $wd

# make sure to add `node-modules/.bin` to your PATH
# this is usually in ~ or in the current directory
PATH=$PATH:/opt/reporting-tool/node_modules/.bin

#highlight_style=pygments
#highlight_style=tango
#highlight_style=espresso
#highlight_style=zenburn
#highlight_style=kate
#highlight_style=monochrome
highlight_style=breezedark
#highlight_style=haddock
# To customize the theme, generate the theme file based on an existing theme, then use that file as argument.
# For example, let's use the haddock theme as base:
# $ pandoc --print-highlight-style haddock > my.theme
# Then uncomment the following line to use the file as theme.
#highlight_style=my.theme

/opt/reporting-tool/generate_content_variables.sh

pandoc \
 --from markdown+fenced_divs+tex_math_dollars+multiline_tables-implicit_figures \
 --filter pandoc-mustache \
 --filter pandoc-imagine \
 --filter filters/findings_summary_pdf.sh \
 --table-of-contents \
 --toc-depth 4 \
 --number-sections \
 --template templates/latex-ks/new_report.tex \
 --pdf-engine=xelatex \
 --pdf-engine-opt=-shell-escape \
 --highlight-style ${highlight_style} \
 --metadata date="`date -u '+%d %B %Y'`"\
 -C \
 --csl style.csl \
 -o output.pdf \
 $@ \

 
echo "[-] Copying final PDF to $cwd/output.pdf"
cp output.pdf $cwd/output.pdf
