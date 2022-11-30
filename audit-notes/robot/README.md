# Reporting tool

Generate HTML and PDF reports from Markdown

# Quickstart

Run using docker:

```
./build.sh
```

The PDF and HTML files will appear in the `output/` directory.

# Run locally

See comments in `make-html.sh` and `make-pdf.sh`.

```
sudo apt install pandoc npm -y
npm i mermaid.cli mermaid-filter
sudo python3 -m pip install nwdiag seqdiag actdiag blockdiag
sudo apt install graphviz
sudo pip3 install git+https://github.com/hertogp/imagine
sudo pip3 install -U pandoc-mustache plotly kaleido
```

## HTML

Run `./make-html.sh`

## PDF

Run `./make-pdf.sh`

# Writing reports

Set the variables in `00_config.md`.

Write the contents of the report in the other Markdown files.

# Examples

See the examples in `99_examples.md`

# Limitations

The pandoc filter `filters/findings_summary.py` expects that there is a section named "Technical details of security findings" and 
a section named "Other observations".

Each sub-section in the findings section must start with:

```
**Severity:** Low/Medium/High

**Status:** Remediated/Open/Other
```

Otherwise, the filter will crash. It expects those subsections to start with exactly that text, because it automatically
computes the number of high/medium/low/informational findings.

In `01_summary.md`, the variables `$findings_summary`, `$findings_table` and `$observations_table` are replaced with 
actual contents by the filter.
