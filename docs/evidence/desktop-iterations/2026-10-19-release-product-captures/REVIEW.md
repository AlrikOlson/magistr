# Fresh product screenshots

Verdict: **PASS for the four selected images** below. Each saved image was opened in full and inspected. Earlier rejected attempts remain as evidence; they are not used on the website.

The user's task was to refresh the app screenshots and include real Codex and Claude Code sessions before initial release. These are actual product/client output, with no generated replies, image editing or annotations. [Provenance](provenance.json) records source and binary hashes; each image has a capture-time JSON receipt.

Selected images:
- [Magistr, dark](desktop-resumed-dark.png)
- [Magistr, light](desktop-resumed-light.png)
- [Codex](codex-resume.png)
- [Claude Code](claude-code-resume.png)

The native images show an actual Claude conversation in an isolated Contact list project. The agent investigated source and proposed a fix and verification command; this native conversation did not implement the change or run the proposed test. Both appearances were reopened with `--continue` against the same saved conversation. The source is `cfd80a5718c6642eb3322e4abab70cc04a4b86c0`; the native frame is 1280×832 at scale 2 (2560×1664 PNG). ScreenCaptureKit captured only the disposable app identified by `dev.magistr.desktop.release-capture`. The app activated itself despite background launch being requested. No OS keyboard, pointer or clipboard input was posted, and no foreground-input journey or assistive-technology test is claimed.

The agent images show real interactive Codex 0.163.0 and Claude Code 2.1.304 processes, with actual Magistr MCP source `2237d01a993eab0cb909de0523a7c83410c4fad7`. Each original task fixed the same small whitespace defect, preserved the existing test, ran `python3 -m unittest -q` successfully and recorded completion. Each photographed fresh session called Magistr context to retrieve that completed task and its verification. Both clients exited normally with code 0. Their unmodified PTY bytes were rendered by xterm 6.0.0 in headless WebKit 27.2: 118 columns ×44 rows, browser viewport 1320×1000, scale 2; the full terminal element is 2370×2120 pixels. The terminal's unused rows are retained. This is a real terminal session rendered in a browser, not a screenshot of the Codex desktop application or a simulated transcript.

Captures use 2026-10-19 local time (America/Los_Angeles); JSON timestamps are UTC on 2026-10-20. The image SHA-256 values in the website manifest match these selected files byte for byte. [Website review](../2026-10-19-release-session-gallery-reviewed/REVIEW.md) covers wide/compact, light/dark presentation and full-size links.

| File | User task and concrete visual observations | Verdict |
| --- | --- | --- |
| desktop-dark.png | Investigate source. The native layout was readable, but a read-only Magistr request incorrectly stopped at a host permission prompt. This exposed a real permission-routing defect. | FAIL, superseded |
| desktop-dark-corrected.png | Inspect the native conversation. A 3456×66 black/menu-bar frame was selected instead of the app; the capture helper now requires a full-size same-process window. | FAIL, superseded |
| desktop-conversation-dark.png | Read the proposed normalization fix. Navigation, Python lines, explanation, verification command and composer were readable with distinct dark surfaces and no overlapping controls. The read-only turn completed after the permission fix. | PASS, superseded by resumed dark |
| desktop-conversation-light.png | Reopen saved work. The dialog reported that advertised continuation was unsupported. This exposed the saved-session admission defect. | FAIL, superseded |
| desktop-resumed-light.png | Reopen and read the same conversation. The saved reply and project selection return; code, explanation, verification and composer are legible. Light surfaces, selected rail item and focus ring are distinct. No refusal dialog or overlapping controls. | PASS, selected |
| desktop-resumed-dark.png | Reopen the same work in dark appearance. Reply and selection match light appearance, the lavender controls and focus ring contrast clearly, and code lines remain readable. No permission stall or continuation refusal. | PASS, selected |
| claude-code-session.png | Implement and verify a small change. The real completion and recorded test result are readable, but the longer scroll position is less useful for introducing the client. | PASS, superseded by fresh-session view |
| codex-session.png | Review completed work. An inherited, unrelated external MCP authentication warning obscures the session. | FAIL, superseded |
| codex-completed.png | Read completion. Completed task and verification are visible; a transient dismissal hint makes this a weaker final frame. | PASS, not selected |
| codex-resume.png | Recover completed work in Codex. Client/version, actual context call, three result bullets and prompt are readable. No warning overlay or clipped text; spacing and monospaced hierarchy distinguish tool output from the response. | PASS, selected |
| claude-code-resume.png | Recover completed work in Claude Code. Client/version, actual Magistr call, source-change summary and verified receipt are readable. The prompt and mode line are distinct, with no overlay or clipping. | PASS, selected |

The capture exercise found and fixed two owning defects: unified public MCP tools now reach native authorization while governance restrictions retain precedence (`2237d01a`); continuation selects the latest checkpoint in the current working copy and uses the existing saved-session admission path (`cfd80a57`). Native verified receipts: `d1691439-a178-420e-bbbb-4ea01719822a` (58 permission/catalog tests, fmt and Clippy), and `1d78adaf-4693-44d5-84fa-2b6fbe688e76` (41 continuation/admission tests, seven desktop argument tests, fmt and both Clippy targets). No trust-policy bypass was added.

This focused visual review covers the photographed conversation and terminal states. It does not recertify every desktop tab, native compact layout, IME, screen reader, or all OS-input journeys. The desktop remains a separate development build outside Cargo/npm's initial CLI distribution.
