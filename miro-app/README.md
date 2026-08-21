# bat-cli linker

A small Miro app that turns bat-cli's link cards into real Miro links — the kind
with the arrow, that show a preview on hover.

## Why it exists

bat-cli deploys over the REST API, and the REST API refuses to set an item's
link:

```
400  Field [linkedTo] is not supported
400  Field [data.linkedTo] is not supported
```

`linkedTo` exists only in the Web SDK, which runs inside a board rather than
from a terminal. So bat-cli falls back to writing an `<a href>` into the card's
text. That navigates correctly, but it reads as a text link.

This app closes the gap: it finds those anchors, moves each one onto the item
itself, and removes it from the text.

## Setup, once

1. Serve this folder over HTTP. Anything static will do:

   ```bash
   cd miro-app && python3 -m http.server 3000
   ```

2. Open your app at <https://miro.com/app/settings/user-profile/apps> — the same
   one `bat-cli login --setup` had you create.

3. Under **App URL** (SDK v2), put `http://localhost:3000/index.html`. Miro
   allows `localhost` over plain HTTP for development; anywhere else needs
   HTTPS.

4. Make sure the app is installed on the team that owns the board.

## Use

Open the board, click the app's icon in the left toolbar, then **Link the
cards**. It reports what it linked:

```
linked FeeLib.feeOf
linked MathLib.wadMul

2 linked, 0 already were.
```

Running it again is harmless — a card that already carries its link is skipped.

Deploy more diagrams, click it again.
