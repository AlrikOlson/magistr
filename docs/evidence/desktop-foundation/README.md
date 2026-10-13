# Desktop foundation evidence

Recorded 12 October 2026. The [architecture decision](../../architecture/desktop-foundation.md)
selects one Tauri Rust host and OS WebView. These are actual disposable native
fixture measurements and captures; they do not certify a production release.

- [build.json](build.json): compiler, installed toolchains, machine/OS/WebKit, exact GPUI commit, source and release-binary SHA-256.
- [comparison-source.tar.gz](comparison-source.tar.gz): exact matched finalist sources, shared workload, lockfile, predeclared protocol and native observer. This archive is historical evidence; GPUI is absent from active manifests and commands.
- [performance/comparison.json](performance/comparison.json): five alternating launch samples and three alternating active runs per finalist. Per-run folders contain all raw pixel/input/motion, process/RSS/CPU, accessibility and stream records.
- [cpu-bounds.json](cpu-bounds.json): conservative late-helper bounds for historical streaming CPU. Idle PID sets were stable. Exact start/end streaming snapshots are retained by the selected validation.
- [journeys/tauri/journeys.json](journeys/tauri/journeys.json) and [journeys/gpui/journeys.json](journeys/gpui/journeys.json): native journey results, including failures; their companion AX/IME records preserve actual values.
- [diagnostics/unavailable-ime-automation.json](diagnostics/unavailable-ime-automation.json): failed unattended activation despite the reported Japanese source. The final selected journey covers 15 core checks and records IME automation as unavailable. Earlier successful native Japanese conversion remains in [journeys/tauri/ime.json](journeys/tauri/ime.json) and [diagnostics/first-use-ime.json](diagnostics/first-use-ime.json); it is not a passing unattended activation gate.
- [voiceover/tauri/walk.json](voiceover/tauri/walk.json), [voiceover/gpui/walk.json](voiceover/gpui/walk.json) and [navigation.json](voiceover/gpui/navigation.json): keys, actual reader PID, accessible focus and OCR of the actual VoiceOver caption panel. Representative caption images are in [captured assets](../../assets/desktop-foundation); original image names in raw transcripts identify captures, not generated references. Not every intermediate PNG is retained.
- [selected-validation/build.json](selected-validation/build.json): the selected source/binary identity after the composer correction and losing dependency removal, with a fresh selected release workload.
- [manifest.json](manifest.json): SHA-256 for every retained artifact and captured image. The focused verifier checks hashes and measured contracts without turning missed budgets into passes.

The GPUI editor in the source archive was derived from the upstream input example
at c66312b200d113694fca08c1d468669028a09a0e, copyright Zed Industries,
Apache-2.0. Its [license](LICENSE-APACHE.txt) is retained. The remaining first-party
fixture and observer use this repository's license; the icon is the existing
Magistr icon from apps/website. Framework dependencies retain their own notices.

To reproduce the original comparison in a disposable directory, unpack the
archive without changing its internal desktop-foundation paths. Populate
/private/tmp/magistr-gpui-foundation-c66312b with the exact Zed commit named above,
including its workspace crates (the archived GPUI manifest records that source
path). Build both archived fixtures with cargo +1.99.0 and the archived lockfile,
then run the archived measure.py prepare and compare commands. This is an
explicit research reproduction outside the active application build.
The retained source predates the selected composer fix and the CPU endpoint
audit: consult the bounds and selected validation rather than assuming these
are equivalent revisions.

Use the [active proof instructions](../../../benchmarks/desktop-foundation/README.md)
for current Tauri journeys and measurements. Native input and screenshots are
limited to fixture bundle identities. VoiceOver was restored to off, the previous
input source was restored, and protected AppleScript settings remained off.
Filesystem-cache-cold, Linux/Windows runtime and installable/signing evidence
were unavailable and have no passing result.
