FROM archlinux AS base

RUN pacman -Syy
RUN pacman -Syu --noconfirm
RUN pacman -S python-pip git --noconfirm
RUN pip install git+https://github.com/hertogp/imagine
RUN pacman -S blockdiag --noconfirm
RUN pacman -S pandoc npm graphviz texlive-most --noconfirm
WORKDIR /opt/reporting-tool
RUN npm i mermaid.cli mermaid-filter
RUN pip install -U pandoc-mustache
RUN pacman -S python-pygments --noconfirm
RUN pacman -S which --noconfirm
RUN pacman -S atk at-spi2-atk libdrm libxkbcommon libxcomposite libxdamage libxrandr mesa --noconfirm
RUN pacman -S alsa-lib --noconfirm
RUN pacman -S noto-fonts --noconfirm

ENV PATH="node-modules/.bin:/opt/reporting-tool:${PATH}"

COPY config/requirements.txt ./
RUN pip install -r requirements.txt
COPY config/.puppeteer.json ./

COPY *.sh ./
# RUN mv make-pdf-interactive.sh mkpdf && mv make-html-interactive.sh mkhtml && chmod +x ./mkpdf && chmod +x ./mkhtml
RUN mv make-html-interactive.sh mkhtml && chmod +x ./mkhtml

COPY config/style.csl ./data/
COPY filters ./data/filters
COPY templates ./data/templates
COPY figures ./data/figures

WORKDIR /compile
