# Interrupted capture run

This run was interrupted when the configured Magistr connection ended its owner session. The server returned `engine_unavailable` with an unknown outcome. Read-back found 52 desktop captures and no completed check receipt or final capture manifest. The browser process subsequently exited.

These files are retained as interrupted evidence. They are **not reviewed or passing evidence**, and do not replace iteration 02 or the subsequent complete run. The rendered site did not change between these runs. The capture harness now checkpoints its manifest after each image so an interrupted run retains provenance.
