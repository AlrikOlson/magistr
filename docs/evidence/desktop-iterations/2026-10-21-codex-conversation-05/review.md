# Conversation iteration 05

Opened and inspected all three original PNGs at full frame and readable detail on 2026-10-21. Capture metadata and source hashes are in browser-checks.json.

| Capture | Task and observations | Verdict |
| --- | --- | --- |
| conversation-start-wide-light.png | Opening request and stationary transport remain readable. | Pass |
| stream-paused-wide-light.png | Partial reply survives inspection and restores focus correctly. | Pass |
| stream-final-paused-wide-light.png | Attempt to play the selected final message. The UI instead shows the first assistant reply: Play treated a seek to the last entry as completed playback. | Fail |

Overall failed. The browser endpoint assertion detected the wrong timeline entry. Playback completion belongs to the timeline controller and must be distinct from selecting a fully rendered still. Correct that state ownership before final capture. Other profiles were not reached in this run.
