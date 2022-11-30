# METHODOLOGY

During every review, the team spends considerable time working with the client to determine correct and expected functionality, business logic, and content to ensure that findings incorporate this business logic into each description and impact. 

Following this discovery phase the team works through the following categories:

* Authentication
* Authorization and Access Control
* Injection and Tampering
* Configuration Issues
* Logic Flaws
* Cryptography

## Tools{-}

The Kudelski Security Services team reviewed the code within the project with an appropriate tooling.

The following list describe the tools that were used during this audit:

* Visual Studio Code
* Semgrep
* Cargo Audit

## Vulnerability Scoring Systems {-}

Kudelski Security utilizes a vulnerability scoring system based on impact of the vulnerability, likelihood of an attack against the vulnerability, and the difficulty of executing an attack against the vulnerability based on a **\textcolor{high}{High}**, **\textcolor{medium}{Medium}**, and **\textcolor{low}{Low}** rating system 

### Severity {-}

Severity is the overall score of the weakness or vulnerability as it is measured from Impact, Likelihood, and Difficulty

### Impact {-}

The overall effect of the vulnerability against the system or organization based on the areas of concern or affected components discussed with the client during the scoping of the engagement. 

| Level | Description |
| :- | :---------------- |
| High | The vulnerability has a severe effect on the company and systems or has an affect within one of the primary areas of concern noted by the client|
| Medium | It is reasonable to assume that the vulnerability would have a measurable effect on the company and systems that may cause minor financial or reputational damage. 
| Low | There is little to no effect from the vulnerability being compromised. These vulnerabilities could lead to complex attacks or create footholds used in more severe attacks. |

### Likelihood {-}

The likelihood of an attacker discovering a vulnerability, exploiting it, and obtaining a foothold varies based on a variety of factors including compensating controls, location of the application, availability of commonly used exploits, and institutional knowledge 

| Level | Description |
| :- | :---------------- |
| High  | It is extremely likely that this vulnerability will be discovered and abused  |
| Medium | It is likely that this vulnerability will be discovered and abused by a skilled attacker | 
| Low | It is unlikely that this vulnerability will be discovered or abused when discovered.  |

### Difficulty {-}

Difficulty is measured according to the ease of exploit by an attacker based on availability of readily available exploits, knowledge of the system, and complexity of attack. It should be noted that a LOW difficulty results in a HIGHER severity. 

| Level | Description |
| :- | :---------------- |
| High  | The vulnerability is easy to exploit or has readily available techniques for exploit |
| Medium | The vulnerability is partially defended against, difficult to exploit, or requires a skilled attacker to exploit. | 
| Low | The vulnerability is difficult to exploit and requires advanced knowledge from a skilled attacker to write an exploit | %   