# Capture harness failure

The run stopped after its first image because the newly extracted report writer called itself recursively. The check recorded a `RangeError` and exit code 1 (receipt `dac3716a-f249-44d8-801e-c1997e92653b`). The final report still contains the captured image's source hashes and metadata. This is failed evidence, not a completed visual review. The helper was corrected before the next run; no rendering source changed.
