# Desktop foundation comparison protocol

Declared before either prototype is measured, 12 October 2026.

Both candidates retain the same generated corpus: 1,000 roadmap items, 160 rich
messages containing Unicode, headings, code, links and requirements, and an
8,000-line diff. Eight independent Rust threads each emit 1,250 updates at 125 Hz
(10,000 events / ten seconds). No production records, providers or stores are
opened. Both adapters virtualize long lists and keep the whole corpus.

Use installed Rust 1.99.0, release opt-level 3, thin LTO and one codegen unit.
Builds run before measurement. Alternate candidates on the same Apple machine (Mac17,6),
M5 Max (18 CPU / 40 GPU cores, 48 GB), macOS 26.5.2, 1,728 × 1,117 logical display
at 2×. Report framework pins, WebKit/OS version and each process included.
Keep observer cost and unrelated machine load explicit. Run cold process launches
with warm filesystem caches; an OS-reboot cold-cache experiment is unavailable.

Budgets chosen before measurement:

| Measure | Target | Reason |
| --- | --- | --- |
| Launch to usable window p95 | 2 seconds | Frequent project opening should feel immediate |
| Idle total CPU | 1% of one core | A waiting agent workspace should not keep the CPU busy |
| App plus WebView RSS | 512 MiB | Leaves room for providers, language servers and editors |
| Input to visible result p95 | 50 ms | Direct interaction target |
| Scroll/resize frame work p95 | 16.7 ms | One frame at 60 Hz |
| Lost/reordered updates | 0 | Updates carry independent per-stream sequence numbers |

Distinguish draw/DOM callbacks from compositor presentation and visible pixels.
Do not call a timer's throughput UI throughput. Count accepted updates and
acknowledged rendered sequence numbers separately. Record delayed updates above
50 ms and high-water memory. Native keyboard, copy/selection, multilingual/IME,
screen-reader navigation and five product journeys accompany performance.
Any missing measurement remains unavailable; it is not a zero or a pass.
