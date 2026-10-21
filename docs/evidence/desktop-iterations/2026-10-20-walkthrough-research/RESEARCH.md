# Recorded walkthrough design research

Research date: 2026-10-20 (Pacific). Existing implementation: 4c15e7ea, magistr.dev.

Task: understand how a coding agent uses Magistr to take a bounded project from a plan through implementation, review and checks. The visitor should be able to follow the work without learning a tool-log viewer.

Primary references read:

- [Arcade: viewer navigation and chapters](https://www.arcade.software/post/a-better-experience-for-your-viewers-is-here), published August 26, 2024. Context-setting chapters, direct navigation and viewer-controlled pace. Apply named story chapters and one stable next/back control. Do not transfer the vendor’s conversion claims to this site.
- [Screen Studio](https://screen.studio/), accessed October 20, 2026. Large screen presentation, deliberate framing, and detail views that remain legible. Apply a dominant visual stage and readable source excerpts. Avoid decorative zooms or invented cursor movement.
- [Scrimba: interactive screencasts](https://docs.scrimba.com/explain/scrims), accessed October 20, 2026. A recorded moment remains connected to the code at that moment. Preserve exact source and check output under the explanation; do not emulate editable code or claim to run the project in the website.
- [Nielsen Norman Group: Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/), published December 3, 2006. Keep the primary task apparent and label the way to detailed evidence. Put all 47 raw actions and file snapshots in an explicitly named disclosure, not the primary navigation.

Architecture: recording.json owns the historical facts, source writes and checks. The website owns editorial explanation and navigation. Its chapter model may reference recorded event IDs; it must never write synthetic historical events or imply that the completed app existed before its build. The original video, images, source archive and checksums remain unchanged.

Current problem observed in the live page: the player is 1,301 px tall at a 1440 × 1000 viewport, before the surrounding explanation. The permanent action list, canvas timeline, milestone row, view switch and file picker compete for attention. The video is confined to the remaining pane. Selecting a milestone replaces the app with raw tool output without explaining its importance to the project.

Design decision: replace the log dashboard with a guided, chapter-based project story. Show the finished inbox as the entry point, then let the viewer follow the real brief, implementation, review and final checks. One large stage changes between the saved plan, focused source, actual review captures and completed-app video. A short explanation stays beside it on desktop and above it on compact screens. Navigation is manual by default; optional chapter playback stops on interaction. Detailed actions stay accessible in one disclosure.

This is a design judgment grounded in the sources and observed interface, not a user-study result.
