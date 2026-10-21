# Recorded workflow — initial visual review

Task: present an actual recorded implementation as interactive playback on the landing page. Opened all 36 PNGs at full frame and readable detail. Exact viewports, appearance, renderer (WebKit 27.2), capture times and source hashes are in [browser-checks.json](browser-checks.json).

Verdict: **Fail**, corrected in the following refined iteration. Native selects remain too short; compact milestone navigation clips later milestones, including the current one. The compact source font needs to be larger. Preserve the actual recording and fix its presentation. The no-JavaScript and failed-download fallback remain usable; a full-size video link will make the small preview easier to inspect.

| Capture | Task and observations | Verdict |
| --- | --- | --- |
| home-intro-wide-light.png | Product description, setup action and project guide. Hero hierarchy and spacing pass; the visible playback pace menu is too small. | Fail |
| recording-overview-wide-light.png | Recorded build with completed app preview and five milestones. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-action-14-wide-light.png | Run the storage checks before building the store. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-action-15-wide-light.png | Implement transactional mail intake and conversation storage. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-action-35-wide-light.png | Give the filters and assignment menu usable targets. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-action-44-wide-light.png | Run the finished app’s checks. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-keyboard-wide-light.png | Milestone selected through the keyboard with focus retained. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| home-intro-wide-dark.png | Product description, setup action and project guide. Hero hierarchy and spacing pass; the visible playback pace menu is too small. | Fail |
| recording-overview-wide-dark.png | Recorded build with completed app preview and five milestones. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-action-14-wide-dark.png | Run the storage checks before building the store. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-action-15-wide-dark.png | Implement transactional mail intake and conversation storage. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-action-35-wide-dark.png | Give the filters and assignment menu usable targets. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-action-44-wide-dark.png | Run the finished app’s checks. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| home-intro-compact-light.png | Product description, setup action and project guide. Hero, preferred-agent subtitle and project link are readable and wrap cleanly. | Pass |
| recording-overview-compact-light.png | Recorded build with completed app preview and five milestones. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-action-14-compact-light.png | Run the storage checks before building the store. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-action-15-compact-light.png | Implement transactional mail intake and conversation storage. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-action-35-compact-light.png | Give the filters and assignment menu usable targets. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-action-44-compact-light.png | Run the finished app’s checks. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-320-light.png | Recorded source edit at 320 CSS pixels. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| home-intro-compact-dark.png | Product description, setup action and project guide. Hero, preferred-agent subtitle and project link are readable and wrap cleanly. | Pass |
| recording-overview-compact-dark.png | Recorded build with completed app preview and five milestones. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-action-14-compact-dark.png | Run the storage checks before building the store. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-action-15-compact-dark.png | Implement transactional mail intake and conversation storage. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-action-35-compact-dark.png | Give the filters and assignment menu usable targets. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-action-44-compact-dark.png | Run the finished app’s checks. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-keyboard-compact-dark.png | Milestone selected through the keyboard with focus retained. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-320-dark.png | Recorded source edit at 320 CSS pixels. Recorded selection, timestamps and source/check content agree. Menus are too short. The horizontal milestone strip hides later stages; source needs a larger compact size. | Fail |
| recording-720-light.png | Source playback at intermediate width. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-720-dark.png | Source playback at intermediate width. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-820-light.png | Source playback at intermediate width. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-820-dark.png | Source playback at intermediate width. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-1024-light.png | Source playback at intermediate width. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-1024-dark.png | Source playback at intermediate width. Recorded selection, timestamps and source/check content agree. Menus are too short. | Fail |
| recording-unavailable.png | Failed recording download retains real video and downloads. App video and download links remain available, with a readable fallback explanation. Add a full-size video link. | Pass |
| recording-without-javascript.png | App recording and downloadable transcript remain available without JavaScript. App video and download links remain available, with a readable fallback explanation. Add a full-size video link. | Pass |

Checks are headless browser coverage. Native OS-input and assistive-technology testing are not claimed. Scroll containers intentionally show a partial adjacent row at their boundary; the active action remains visible. No fabricated check result or task state is used.
