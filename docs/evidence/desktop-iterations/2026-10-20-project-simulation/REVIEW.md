# Project simulation — initial review

Task: replace the landing-page toy task with a substantial visual, interactive project simulation.

Opened all three saved PNGs directly from this folder at original resolution. WebKit 27.2, 1440×1000 viewport, light appearance; full component captures retain their actual 1104×1292 dimensions. Capture times, source hashes and image hashes are in browser-checks.json.

| Capture | Observation | Verdict |
| --- | --- | --- |
| home-intro-wide-light.png | Heading, revised example link and introduction are readable. The simulation follows the hero. | Pass |
| simulation-stage-0-wide-light.png | Connections attach to node edges and converge clearly at launch; labels and task details are readable. Status legend dots are all gray because the base selector overrides their status colors. The map should precede the longer explanatory text so it is prominent sooner. | Fail |
| simulation-stage-1-wide-light.png | Completed access and in-progress email are clear. Search says Ready even though the displayed dependency and narrative require completed email. The assignment task has an inconsistent prerequisite. Legend colors also need correction. | Fail |

The browser run stopped on the search-state assertion. Correct the scenario's task dependency and legend styles, bring the map before the explanation, and recapture into a new folder. No passing final visual verdict is claimed. These are headless website captures, not native OS-input or assistive-technology evidence.
