# Text-enlargement readiness failure

The font-scaling assertion stopped this run before enlarged-text capture. Its exact exit status and source provenance are in `docs-browser-checks.json` and check receipt `3a766783-cdd5-4bf7-a179-90e10238d2ce`. This incomplete set is not a passing visual review.

A focused reproduction of the same 320px-to-800px sequence showed root text at 32px while inherited paragraph/link/strong/cell sizes temporarily remained 17px/14.4px. After style settling they became 34px/28.8px without a source or CSS change. The harness now waits for each measured font size to reach twice its baseline before asserting and capturing, instead of assuming two animation frames are enough. `diagnostic-storage.png` retains the diagnostic's starting full-page capture; it is not passing evidence.
