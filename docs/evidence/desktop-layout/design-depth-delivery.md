# Desktop design depth: twenty further chunks

Delivered for the October 14, 2026 request as native scope `desktop-design-depth-20`. These are **twenty new commits after `fa3ed7a1`**, in addition to the [earlier twenty-chunk pass](design-delivery.md).

The design moves the desktop toward recessed ink and ivory surfaces, authored document pages, illuminated controls, calibrated instruments and a shared material treatment across dialogs. The actual desktop uses a bounded WebGL shader for the activity ring.

| # | Delivered design | Commit | Preview |
| --- | --- | --- | --- |
| 1 | Recessed workspace frame, ink/ivory palettes, surface highlights and inset depth. | `ce18718a` | [Workspace](workspace-light.png) |
| 2 | Project identity marks and connected conversation branches. | `da534937` | [Navigation](workspace-dark.png) |
| 3 | Illuminated attention card, count badge and clearer primary action. | `1333337d` | [Attention](request-dark.png) |
| 4 | Sculpted conversation masthead, breadcrumb and working-copy controls. | `2c3c86a3` | [Masthead](workspace-light.png) |
| 5 | Larger project landing composition with layered geometric illustration. | `56e5443a` | [Landing](welcome-light.png) |
| 6 | Beveled shader ring, directional and strip highlights, radial brushing and contact shadow. | `faac7ed6` | [Shader](welcome-dark.png) |
| 7 | Dimensional project boards with actual counts and embedded conversation tracks. | `88e6bd36` | [Directory](directory-light.png) |
| 8 | Authored message signatures, inset user messages and agent document sheets. | `84274f03` | [Conversation](workspace-dark.png) |
| 9 | Numbered reply contents, larger editorial hierarchy and quotation panels. | `8e73d301` | [Reading](editorial-light.png) |
| 10 | Lit source toolbar, language badge and calibrated line gutter. | `f505d962` | [Source](code-dark.png) |
| 11 | Comparison-table headers, typed result sheets and numeric alignment. | `f10c2365` | [Records](records-light.png) |
| 12 | Distinct patch planes, hatched unmatched positions and exact intraline emphasis. | `15f046d8` | [Patch](patch-comparison-dark.png) |
| 13 | Connected execution timeline, status symbols and expanded result cards. | `c39171c4` | [Tools](execution-dark.png) |
| 14 | Tactile composer toolbar and expanded writing sheet on a separate desk surface. | `67ccc65b` | [Writing](writing-light.png) |
| 15 | Calibrated attempt dial, current-position pointer and numbered launch controls. | `3e2b677a` | [Planned work](work-paused-dark.png) |
| 16 | Radial capacity instrument, exact telemetry and separate unavailable-usage treatment. | `ac8a1f7f` | [Inspector](inspector-light.png) |
| 17 | Amber request sheet, framed typed details and equal-weight answer choices. | `2ef732d1` | [Requests](permission-compact.png) |
| 18 | Larger file-gallery framing, labeled image filmstrip and grouped inspection controls. | `19eb196a` | [Images](image-stage-dark.png) |
| 19 | Saved-library cards, inset reading page, project-grouped search and framed appearance previews. | `ad0d5e91` | [Library](history-dark.png) |
| 20 | Project-opening composition, destination illustration, agent cards, setup review and final adaptive review. | This delivery commit | [Opening](new-conversation-narrow-light.png) |

## Ownership and rendering

Application-owned sessions, source text, drafts, attachments, preferences and usage remain authoritative. The interface changes their presentation. Patch emphasis preserves each line's text and original-source copy; file captions use actual names and dimensions. The work dial indicates an attempt, never inferred completion. The capacity gauge retains the existing semantic meter and distinguishes absent values from reported zero.

The activity material extends the existing local renderer: 48 ray-march steps, at most a 384-pixel drawing buffer, no animation loop, and draws only on meaningful data/theme/size/visibility changes. Context-loss and unchanged-input behavior passed browser checks. The geometry uses a beveled rectangular ring cross-section with enamel-like lighting; it does not claim physical glass. The [WebGL guidance](https://developer.mozilla.org/en-US/docs/Web/API/WebGL_API/WebGL_best_practices) informs the bounded resource lifecycle. A new scene-engine dependency was unnecessary for this one shader object.

## Recorded verification

Each chunk has its own focused checks and reviewed capture receipt in native Magistr state. Final-source verification passed:

- 48 desktop headless WebKit scenarios, covering source copy, patch presentation, selection and reading position, drafts, keyboard navigation, requests, opening, attachment inspection, preferences and reduced motion.
- 92 JavaScript model checks.
- Desktop Rust formatting and all-target Clippy with warnings denied.
- A local debug macOS app build containing the final frontend.
- The screenshot gate: 95 current captures, source hashes, recorded visual review, both palettes' contrast, the canonical icon and all 144 interface modules/harnesses indexed.

The final visual review covers both appearances, the 760 × 550 native minimum, 1920 × 1080 composition, the shader, project boards, opening, setup and agent selection. Earlier per-chunk commits retain their focused surface reviews. The current [review receipt](review.json) lists the images inspected in the closing review; the complete [capture receipt](receipt.json) records all 95 captures.

These checks ran in the background. Native OS input, VoiceOver and live-provider behavior were not exercised. The app build is ad-hoc signed for local use, not notarized or published. No unrelated edits were committed.

Local app: `crates/magistr-desktop/target/aarch64-apple-darwin/debug/bundle/macos/Magistr.app`.

[Desktop architecture](../../architecture/desktop.md) · [Dark workspace](workspace-wide-dark.png) · [Compact light workspace](workspace-compact-final-light.png)
