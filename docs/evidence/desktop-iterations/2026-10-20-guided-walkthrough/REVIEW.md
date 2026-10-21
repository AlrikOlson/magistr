# Guided walkthrough — first iteration

Task: follow the real support-inbox build without operating a tool-log dashboard.

Opened all five saved captures at full resolution. WebKit 27.2, 1440 × 1000, light appearance, reduced motion; capture time and exact source/image hashes are in browser-checks.json.

| Capture | Observation | Visual verdict |
| --- | --- | --- |
| home-intro-wide-light.png | Clear single-color headline and product introduction; walkthrough introduction aligns with the page. | Pass |
| recording-overview-wide-light.png | Large readable actual app capture, one primary start action, six legible chapters, subordinate detailed evidence. Image can scroll; full capture is linked. | Pass |
| recording-chapter-0-wide-light.png | The saved five-part plan is readable; completion appears only for the scope task at this recorded time. Brief and planned work have a clear relationship. | Pass |
| recording-chapter-1-wide-light.png | Exact code and passing check are legible. Line numbers intended for source orientation are missing due to the global list reset. | Fail |
| recording-chapter-2-wide-light.png | The stale-save test and check result are readable. Same missing line-number defect. | Fail |

Overall: FAIL. Browser run also stopped when selecting the first app chapter: the image was lazily loaded outside the viewport after clicking navigation below the stage. Correct active-chapter loading and intrinsic image sizing; restore source line numbers, then recapture in a new folder. No overlap or clipping observed in the five frames; compact and dark states were not reached. These are superseded captures, not final approval.
