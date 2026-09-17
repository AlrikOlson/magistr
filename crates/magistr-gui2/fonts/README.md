# Embedded font provenance

The Hanken Grotesk faces are static instances made from its variable masters.
Their copyright and SIL Open Font License are in `HankenGrotesk-OFL.txt`.

`SourceSerif4Display-Regular.ttf` contains **Magistr Display Regular**, a
renamed derivative of Adobe's Source Serif 4 Display Regular, release 4.005R.
The existing subset has 427 glyphs; its coverage is described in
`../src/display_face.rs`. The asset filename remains stable for the embedded
include path and records its upstream provenance.

Adobe's [4.005R license](https://github.com/adobe-fonts/source-serif/blob/4.005R/LICENSE.md)
reserves the name “Source”. The [OFL FAQ, section 3.1](https://openfontlicense.org/ofl-faq/)
requires a derivative's internal name to change when reserved names apply.
`SourceSerif4-OFL.txt` and the font's original copyright, trademark, authorship
and license metadata are retained. This font remains licensed under OFL 1.1.

The naming change replaces these OpenType `name` records:

| ID | Value |
| --- | --- |
| 1 — family | Magistr Display |
| 3 — unique identifier | 4.005;MagistrDisplay-Regular |
| 4 — full name | Magistr Display Regular |
| 6 — PostScript name | MagistrDisplay-Regular |

All other font metadata and glyph data are retained.
