// Give bat-cli's link cards a real Miro link.
//
// The REST API refuses to set `linkedTo` — it answers
// `Field [linkedTo] is not supported` — so bat-cli falls back to putting an
// `<a href>` inside the shape's text. That navigates, but it reads as a text
// link rather than Miro's own link, which shows an arrow and a preview.
//
// The Web SDK does have `linkedTo`, and it only runs inside a board, which is
// what this app is for. It finds the anchors bat-cli left behind, moves each one
// onto the item itself, and takes the anchor out of the text so the card is left
// reading as a title.

/** Matches the anchor bat-cli writes, pointing at an object on this board. */
const CARD_ANCHOR = /<a[^>]*href="([^"]*moveToWidget[^"]*)"[^>]*>(.*?)<\/a>/i;

/** Miro stores content HTML-escaped, so `=` comes back as `&#61;`. */
function decodeEntities(text) {
  const element = document.createElement('textarea');
  element.innerHTML = text;
  return element.value;
}

const log = (message) => {
  document.getElementById('log').textContent += `${message}\n`;
};

async function linkCards() {
  const button = document.getElementById('run');
  button.disabled = true;
  document.getElementById('log').textContent = '';

  const shapes = await miro.board.get({ type: 'shape' });
  let linked = 0;
  let alreadyDone = 0;

  for (const shape of shapes) {
    const match = CARD_ANCHOR.exec(shape.content || '');
    if (!match) continue;

    const [, rawUrl, label] = match;
    const url = decodeEntities(rawUrl);

    if (shape.linkedTo === url) {
      alreadyDone += 1;
      continue;
    }

    shape.linkedTo = url;
    // The link lives on the item now, so the anchor in the text is redundant —
    // and leaving it would give the card two ways to be clicked.
    shape.content = shape.content.replace(CARD_ANCHOR, label);
    await shape.sync();

    linked += 1;
    log(`linked ${label}`);
  }

  if (linked === 0 && alreadyDone === 0) {
    log('No bat-cli cards on this board.');
  } else {
    log(`\n${linked} linked, ${alreadyDone} already were.`);
  }
  button.disabled = false;
}

document.getElementById('run').addEventListener('click', () => {
  linkCards().catch((error) => log(`failed: ${error.message}`));
});
