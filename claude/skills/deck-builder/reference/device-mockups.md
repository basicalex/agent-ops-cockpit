# Device mockups (phones, browser windows, floating cards)

Product screens on a slide are drawn, not screenshotted, with `@intrface/mockups` (`~/dev/intrface-mockups`). The style is the flat squircle illustration Infobip uses on its site, measured from their assets on 2026-09-13; the numbers live in that repo's `SPEC.md` and are not restated here.

## Why a library

- One geometry for decks and web pages. The same JSON spec renders a ppt-master-safe `<g data-pptx-bounds>` fragment for a slide and an inline SVG for a page.
- Text wraps from real glyph widths (Arial, Georgia tables in `data/font-widths.json`), so bubbles and chips never spill.
- Every shape is native: rect, circle, path, text. No filters, no transforms, no group opacity. PowerPoint keeps it editable.

## Deck use

Write the screen as JSON (vocabulary in the repo README: header, bubble, chips, card, rows, steps, stamp, field, button, text, call, choice, spacer), then:

```bash
cd ~/dev/intrface-mockups
bun run bin/mockup.ts specs/polis/s04-sms.json --deck --x 160 --y 214 --id phone-sms --theme polis > /tmp/phone-sms.svg
```

Paste the fragment into the page as a root group. The first line of the output is `<!-- bounds: x y w h -->`; the fragment already carries `data-pptx-bounds`, so only make sure it does not overlap another root group. A `caption` in the spec renders under the shell inside the same bounds.

Rules that the gate enforces and the library respects:

- Fonts inside a device are Arial only; Georgia stays outside devices.
- Minimum 9 px inside a screen. The renderer drops blocks that do not fit and prints a warning; it never shrinks text. Shorten the copy or raise `width`.
- The shell has no shadow and no stroke. Put it on a panel (`panel` colour of the theme) or straight on the paper; do not add `feDropShadow`.
- Screenshots go only inside a `browser` spec's `image` slot, which emits the full-frame `clipPath` the gate accepts.

Themes: `polis` (paper, ink, teal) and `infobip` (their tokens). Pass `--theme` or set `theme` in the spec.

## Web use

```html
<script type="module" src="/path/to/intrface-mockups/src/element.ts"></script>
<intrface-mockup theme="polis" width="280">
  <script type="application/json">{ "kind": "phone", "header": { "name": "Polis · Općina Vrsar" }, "blocks": [ { "type": "bubble", "side": "out", "text": "Rupa na cesti kod škole." } ], "bar": "input" }</script>
</intrface-mockup>
```

The element renders into light DOM, so page CSS reaches the text. `demo/gallery.html` in the repo shows every block in both themes.

## Verifying

Run `deck-gate.sh` and look at the render as with any page. Specific things to check on a mockup slide: bands align with the squircle, chips are equal width and inside the gutter, the outgoing bubble sits right-aligned, captions are centred under the shell, diacritics survived.
