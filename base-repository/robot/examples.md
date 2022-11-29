\newpage
# Math formula

This is an example math formula:

$\lim\limits_{x \to \infty} \exp(-x) = 0$

# Content variables

The customer is named: {{customer}}

Any variable defined in `00_config.md` can be used in the template (HTML/PDF) and the content (here, Markdown).

# Mermaidjs charts

```{.mermaid im_opt="-p .puppeteer.json" width=15cm height=10cm}
sequenceDiagram
    Alice ->> Bob: Hello Bob, how are you?
    Bob-->>John: How about you John?
    Bob--x Alice: I am good thanks!
    Bob-x John: I am good thanks!
    Note right of John: Bob thinks a long<br/>long time, so long<br/>that the text does<br/>not fit on a row.

    Bob-->Alice: Checking with John...
    Alice->John: Yes... John, how are you?
```

# Citations

[@article1] is a nice source of information. [@article2; @article3] also.
