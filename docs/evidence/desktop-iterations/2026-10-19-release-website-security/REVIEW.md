# Website security and installation — first visual review

**FAIL — superseded by the corrected iteration.** These captures were saved directly here
at 2026-10-20 02:18 UTC. They are retained and must not be used as release approval.

The task is to install the preview, select an agent, and find setup recovery commands.
The full source hashes, viewport, appearance, renderer (headless WebKit 27.2), time,
and PNG hashes are recorded in [browser-checks.json](browser-checks.json).
Behavior checks passed independently; they did not establish visual quality.

Opened and inspected:

| File | Observation | Verdict |
| --- | --- | --- |
| home-wide-light.png | Hierarchy and wrapping are clear. The install code panel stretches far beyond its four lines. | Fail |
| home-wide-dark.png | Primary-button white text on pale blue measures 2.38:1. The install panel also stretches unnecessarily. | Fail |
| home-compact-light.png | External documentation arrows move to an extra row; long commands are clipped inside horizontally scrolling code panels. | Fail |
| home-compact-dark.png | The same arrow and code problems recur, and the primary-button contrast is insufficient. | Fail |
| home-install-wide-dark.png | Readable command text occupies a small part of a large stretched panel; the planning handoff is clear. | Fail |
| home-menu-compact-dark.png | The translucent expanded menu shows underlying installation text; code lines remain clipped. | Fail |
| installation-install-the-preview-compact-light.png | The install commands extend beyond the visible code panel. Heading, body size and spacing are otherwise readable. | Fail |
| installation-connect-another-agent-compact-dark.png | Portable instructions and inline commands wrap legibly; the translucent header still shows underlying text. | Fail |

The other 54 images in this first set were not individually reviewed after these blocking
defects were found. Their generated metadata remains pending. The correction belongs in
the shared stylesheet: explicit foreground-on-accent colors, opaque navigation, wrapped
code, content-height installation panels and explicit arrow placement. The corrected
iteration receives new captures; none of these images will be overwritten.

This is headless browser evidence. Native keyboard focus traversal and assistive technology
were not tested, and nothing was deployed.
