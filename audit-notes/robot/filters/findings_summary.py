#!/usr/bin/env python3

"""Insert a findings summary at the end of the summary chapter"""

import os
import sys

import pandas as pd
import panflute as pf
import plotly.graph_objects as go
from panflute import Header
from panflute import TableRow, TableHead, TableBody, TableCell
from panflute import stringify

ITEM_PREFIX = "KS-"
FINDING_PREFIX = f"{ITEM_PREFIX}"
OBSERVATION_PREFIX = f"{ITEM_PREFIX}O-"


def action(elem, doc):
    if isinstance(elem, Header) and elem.level == 2:
        orig_text = stringify(elem)
        # find previous header with level == 1
        found = False
        header = elem.prev

        while not found and header is not None:

            if isinstance(header, Header) and header.level == 1:
                # OBSERVATIONS
                if header.identifier == "other-observations":
                    title = stringify(header)
                    found = True

                    # set finding prefix
                    observation_id = f"{OBSERVATION_PREFIX}{str(doc.obs_count).zfill(2)}"
                    elem.content = [
                        pf.Str(f"{observation_id}: "), pf.Str(orig_text)]
                    doc.obs_count += 1
                    doc.findings_severity["informational"] += 1

                    # save observation
                    # status_line = elem.next.content
                    # if status_line[0].content[0].text != "Status:":
                    #     raise Exception(f"It looks like status is missing for the observation: {orig_text}")
                    # status = status_line[2].text
                    #
                    # # Set status in color and bold
                    # colored_inline = color_rawinline(status.lower(), status)
                    # elem.next.content[2] = colored_inline

                    severity = "Informational"
                    observation = [(observation_id, severity, orig_text)]
                    doc.observations += observation

                # FINDINGS
                elif header.identifier == "technical-details-of-security-findings":
                    title = stringify(header)
                    found = True

                    # set finding prefix
                    finding_id = f"{FINDING_PREFIX}{str(doc.finding_count).zfill(2)}"
                    elem.content = [
                        pf.Str(f"{finding_id}: "), pf.Str(orig_text)]
                    doc.finding_count += 1

                    # find the severity of this finding and update counts
                    try:
                        severity_line = elem.next.content.list
                        if severity_line[0].content.list[0].text != "Severity:":
                            raise Exception(
                                f"It looks like severity is missing for the finding: {orig_text}")
                        severity = severity_line[2].text
                        doc.findings_severity[severity.lower()] += 1

                        # Set severity in color and bold
                        colored_inline = color_rawinline(
                            severity.lower(), severity)
                        elem.next.content[2] = colored_inline

                        status_line = elem.next.next.content.list
                        if status_line[0].content.list[0].text != "Status:":
                            raise Exception(
                                f"It looks like status is missing for the finding: {orig_text}")
                        status = status_line[2].text

                        # Set status in color and bold
                        colored_inline = color_rawinline(
                            status.lower(), status)
                        elem.next.next.content[2] = colored_inline

                        # save finding
                        finding = [(finding_id, severity, orig_text, status)]
                        doc.findings += finding
                    except Exception as e:
                        raise e

                else:
                    break

            header = header.prev


def color_rawinline(color_class, text):
    report_format = get_report_format()
    rawinline_format = "latex" if report_format == "pdf" else "html"

    colored_text = raw_color(color_class, text)
    return pf.RawInline(colored_text, format=rawinline_format)


def raw_color(color_class, text):
    report_format = get_report_format()

    if report_format == "pdf":
        colored_text = '\\textbf{\\textcolor{' + \
            color_class + '}{' + text + '}}'
    elif report_format == "html":
        colored_text = '<span class="' + color_class + \
            '-color"><strong>' + text + '</strong></span>'
    else:
        raise Exception("Unsupported report format")

    return colored_text


def get_report_format():
    try:
        report_format = os.environ["REPORT_FORMAT"].lower()
    except KeyError:
        report_format = "pdf"
    return report_format


def prepare(doc):
    doc.finding_count = 1
    doc.obs_count = 1
    doc.findings_severity = {
        "low": 0,
        "medium": 0,
        "high": 0,
        "informational": 0,
    }
    doc.findings = []
    doc.observations = []


def finalize(doc):
    sys.stderr.write(f"{str(doc.findings_severity)}\n")

    replace_findings_summary(doc)
    replace_findings_summary_chart(doc)
    replace_findings_table(doc)
    replace_observations_table(doc)


def replace_findings_table(doc):
    # replace $findings_table with table
    # | ID | Severity | Finding | Status |"
    table_head = TableHead(TableRow(cell("ID"), cell(
        "Severity"), cell("Finding"), cell("Status")))
    line_length = 36
    rows = [
        TableRow(cell(finding_id), scell(severity), div_cell(finding_name, line_length=line_length),
                 scell(status))
        for finding_id, severity, finding_name, status
        in doc.findings
    ]
    # Table column widths
    colspec = []
    colspec += [("AlignLeft", 0.10)]
    colspec += [("AlignLeft", 0.10)]
    colspec += [("AlignLeft", 0.62)]
    colspec += [("AlignLeft", 0.18)]

    findings_table = pf.Table(
        TableBody(*rows), head=table_head, caption=pf.Caption(), colspec=colspec)
    doc.replace_keyword("$findings_table", findings_table)


def replace_observations_table(doc):
    # replace $observations_table< with table
    # | ID | Severity | Finding | Status |"
    table_head = TableHead(
        TableRow(cell("ID"), cell("Severity"), cell("Finding")))
    line_length = 52
    rows = [
        TableRow(cell(finding_id), scell(severity), div_cell(
            finding_name, line_length=line_length))
        for finding_id, severity, finding_name
        in doc.observations
    ]
    # Table column widths
    colspec = []
    colspec += [("AlignLeft", 0.10)]
    colspec += [("AlignLeft", 0.18)]
    colspec += [("AlignLeft", 0.72)]
    observations_table = pf.Table(
        TableBody(*rows), head=table_head, caption=pf.Caption(), colspec=colspec)
    doc.replace_keyword("$observations_table", observations_table)


def replace_findings_summary(doc):
    # replace $findings_summary with summary
    findings_summary_string = ""
    high = doc.findings_severity["high"]
    medium = doc.findings_severity["medium"]
    low = doc.findings_severity["low"]
    informational = doc.findings_severity["informational"]

    last = 0
    for i, count in enumerate([high, medium, low, informational]):
        if count > 0:
            last = i

    has_previous = False
    if high > 0:
        findings_summary_string += raw_color("high", str(high) + " High")
        has_previous = True
    if medium > 0:
        if has_previous:
            separator = " and " if last == 1 else ", "
            findings_summary_string += separator
        findings_summary_string += raw_color("medium", str(medium) + " Medium")
        has_previous = True
    if low > 0:
        if has_previous:
            separator = " and " if last == 2 else ", "
            findings_summary_string += separator
        findings_summary_string += raw_color("low", str(low) + " Low")
        has_previous = True
    if informational > 0:
        if has_previous:
            separator = " and "
            findings_summary_string += separator
        findings_summary_string += raw_color("informational",
                                             str(informational) + " Informational")

    report_format = get_report_format()
    rawinline_format = "latex" if report_format == "pdf" else "html"
    findings_summary = pf.RawInline(
        f"{findings_summary_string}", format=rawinline_format)
    doc.replace_keyword("$findings_summary", findings_summary)


def replace_findings_summary_chart(doc):
    # replace $findings_summary with summary
    high = doc.findings_severity["high"]
    medium = doc.findings_severity["medium"]
    low = doc.findings_severity["low"]
    informational = doc.findings_severity["informational"]

    # generate chart image
    output_path = "./figures/findings_histogram.png"
    generate_findings_chart(high, medium, low, informational, output_path)

    report_format = get_report_format()

    pdf_attributes = {"width": "17cm", "height": "10.2cm"}

    if report_format == "html":
        pdf_attributes = {}

    # add chart image to document
    findings_summary_chart = pf.Image(
        url=output_path,
        title="Issue severity distribution",
        attributes=pdf_attributes
    )
    doc.replace_keyword("$findings_summary_chart", findings_summary_chart)


def generate_findings_chart(high, medium, low, informational, output_path):
    severity_names = ["High", "Medium", "Low", "Informational"]
    severities = [high, medium, low, informational]
    df = pd.DataFrame(list(zip(severity_names, severities)),
                      columns=["Severity", "Number of issues"])

    fig = go.Figure()
    colors = ["#FF0000", "#F6902E", "#0070C0", "#00B050"]
    fig.add_trace(go.Bar(x=severity_names, y=severities,
                  text=severities, marker_color=colors))

    # show values on bars
    fig.update_traces(textposition="outside")

    fig.update_layout(
        title={
            "text": "Issue severity distribution",
            "y": 0.9,
            "x": 0.5,
            "xanchor": "center",
            "yanchor": "top"
        },
        title_text="Issue severity distribution",
        xaxis_title_text="Severity",
        yaxis_title_text="Number of issues",
        bargap=0.2,  # gap between bars of adjacent location coordinates
        bargroupgap=0.1,  # gap between bars of the same location coordinates
        font=dict(
            family="Helvetica, Arial",
            size=18,
            color="black"
        )
    )

    # do not show labels on the y-axis
    fig.update_yaxes(showticklabels=False)

    fig.write_image(file=output_path, format="png", width=1000, height=600)


def cell(string):
    return TableCell(pf.Plain(pf.Str(string)))


def div_cell(string, line_length=40):
    report_format = get_report_format()
    contents = [pf.Plain(pf.Str(string))]
    if report_format == "pdf" and len(string) > line_length:
        # add spacing at bottom of div
        vertical_spacing = pf.RawBlock("\\vspace{0.12cm}", format="latex")
        contents.append(vertical_spacing)
    return TableCell(pf.Div(*contents))


def scell(string):
    colored_severity_rawinline = color_rawinline(string.lower(), string)
    return TableCell(pf.Plain(colored_severity_rawinline))


def main(doc=None):
    return pf.run_filter(action, doc=doc, prepare=prepare, finalize=finalize)


if __name__ == '__main__':
    main()
