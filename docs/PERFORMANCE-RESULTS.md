# First storage measurements

Date: 2026-11-07. Apple M4 Mac, 16 GiB RAM. Release storage build, two Cargo jobs. These results do not prove a visible viewport or cold-start target.

Fixture: 100,000 pages, 299,997 text blocks, and one database with 1,000,000 records. The database file is about 1.5 GiB. The probe checks row counts and uses the production Store methods. Each repeated case has 30 measured samples after warm-up. The OS cache is not controlled.

Command:

```sh
cargo run --release -p moss-core --example storage_bench -- --path /tmp/moss-storage-full.sqlite
# Reuse the fixture after a code change:
cargo run --release -p moss-core --example storage_bench -- --reuse --path /tmp/moss-storage-full.sqlite
```

| Case | p50 | p95 | Maximum |
| --- | ---: | ---: | ---: |
| Read one page | 0.253 ms | 1.692 ms | 2.616 ms |
| Read three page blocks | 0.305 ms | 0.578 ms | 2.250 ms |
| Repeat page + blocks immediately after those reads | 0.014 ms | 0.020 ms | 0.021 ms |
| First 50 rows, title sort | 0.076 ms | 0.104 ms | 0.196 ms |
| First 50 rows, status filter and title sort | 0.110 ms | 0.127 ms | 0.217 ms |
| 50 rows at offset 500,000 | 17.253 ms | 24.642 ms | 27.109 ms |
| Save three blocks and update full-text index | 0.633 ms | 6.026 ms | 12.921 ms |

Loading all 100,000 sidebar page objects took 337.566 ms in one sample. The app currently uses that query at startup. It must use a bounded projection before the 300 ms full-app launch limit can pass.

The repeated page + blocks case reads data that the two preceding calls just loaded. Its low time is expected. It must not be presented as an uncached result.

## Fixes and remaining limits

- The original store open scanned all live records to purge expired Trash. Its first full-scale sample was 679.105 ms. Partial indexes reduced a later repeat store-open sample to 1.860 ms. This excludes Rune, sidebar loading, text layout, GPU work, and display presentation.
- Full-text updates previously deleted entries through an unindexed key. They now use an indexed key and exact match. The one-time FTS migration on the existing 1.5 GiB fixture took 14,958.193 ms as part of store open. Fresh workspaces use the new schema. Migration is a separate unresolved launch case.
- Deep offset paging exceeds the 20 ms large-database limit in these storage-only samples. Use indexed cursor paging before full-scale release.
- JSON property queries can scan records. The status-filter sample has many matching rows near the start of title order and is a favorable case. Test sparse matches, computed fields, multiple sorts, complex filters, and large text/media before making a scale claim.
- Writes can exceed one frame. Move durable saves and index work off the UI thread while retaining unsaved data and reporting failures.
- Use a bounded page projection for the sidebar and bounded block reads for the first document viewport. Full document loading is still eager.
- Add input-to-visible-frame measurements. Include disk-cache conditions, first and subsequent opens, real media previews, and adverse data shapes. The 8.8 ms, 20 ms and 300 ms product targets remain unverified.

## Bounded navigation probe after the UI batch

Same Mac and full fixture, 7 November 2026: 100,000 pages, 299,997 blocks and 1,000,000 records. This is a release storage probe while development work continued. OS cache and other machine work were not controlled. The figures are not visible-frame or uncached-disk results.

The sidebar now requests 50 siblings with keyset cursors and 30 independent search suggestions. Returned child flags exclude deleted and record pages. Page children use the same bounded API. Partial indexes cover live-page sibling and suggestion order. Cursor queries separate equal timestamps from later or earlier ones so both keys have explicit bounds.

| Repeated storage case | p50 | p95 | Maximum |
| --- | ---: | ---: | ---: |
| First 50 sidebar roots | 0.220 ms | 0.349 ms | 0.381 ms |
| 50 roots after a keyset cursor at page 25,000 | 0.172 ms | 0.281 ms | 0.319 ms |
| 30 workspace-wide suggestions | 0.060 ms | 0.166 ms | 0.169 ms |

There were 30 measured samples after warm-up. A single initial read of 50 roots plus 30 suggestions took 1.895 ms; the OS cache was uncontrolled. Store open took 2.064 ms after the indexes existed. The first open that created the two indexes on the existing fixture took 3,124.550 ms. That migration case fails the 300 ms launch target before Rune starts and still needs a migration strategy.

The same run measured deep database OFFSET reads at p95 43.353 ms, with a 43.483 ms maximum. Saving three blocks and updating FTS reached 27.268 ms. These remain unresolved paths. Do not compare these variable shared-machine samples as a controlled before/after experiment, and do not report the 8.8 ms, 20 ms or 300 ms app targets as achieved.

Reproduction on the retained task fixture:

```sh
cargo run --release -p moss-core --example storage_bench --jobs 2 -- --reuse --path /tmp/moss-storage-full-20261107.sqlite
```

## Indexed page navigation study, 8 November 2026

A read-only storage probe used the scratch fixture with one million records. It read 101 rows from `bench-database` in ID order. After three warm-up reads, 20 samples at offset 500,000 had a 15.806 ms median, 22.704 ms p95 and 23.544 ms maximum. The same SQL at offset zero had a 0.036 ms median and 0.044 ms p95. These are warm SQLite reads, not app-frame or uncached measurements.

For the same fixture, an indexed ID seek had a 0.027 ms median and 0.030 ms p95. An ascending Title/ID seek had a 0.029 ms median and 0.034 ms p95. Query plans showed a range seek on the matching index. The tested descending Title order with ascending ID ties used a temporary sort and had a 31.638 ms median. The probes ran on a shared machine and are not a controlled acceptance benchmark.

Next and Previous now use a cursor for ID order and ascending Title order, with the active filter and search included in the cursor identity. A changed query or schema, a missing anchor, or a changed Title anchor rejects the cursor. Tests compare both directions with offset results, including duplicate titles and filters. Pages are live queries, not a database snapshot: changes to other rows between requests can change their membership. Direct offset jumps, descending Title, other property sorts and grouped views retain the offset path. Their deep-page cost is not fixed by this change. These results do not establish the 8.8 ms, 20 ms or 300 ms product targets.

### Table viewport baseline, 2026-11-08

These results use a disposable local database with 100,000 pages and 1,000,000 records. The Table has eight properties; the first 1,200 records have long titles and values. The release build uses a 1,440 × 900 point window at scale 2. Each Table page contains at most 999 records. This is a paged viewport test, not a test of continuous scrolling through one million logical rows. SQLite and OS cache state were not controlled. The running development app shared the machine.

The original page-icon lookup scanned the million-row database for each viewport. Thirty 100-record openings had a 3,254 ms median and 4,940 ms maximum. A sample and SQLite query plan identified the scan. The lookup now starts with requested IDs and uses the record primary key. After that change, the 999-record page still took a median 85.9 ms to open and its headless wheel frames had a 12.4 ms median. The Table now mounts a bounded set of rows, including grouped and summary rows.

After both changes, 29 warm headless opens of 100 records had p50 2.98 ms, p95 3.95 ms, p99/max 6.88 ms. For 999 records, the values were 5.53, 6.13 and 6.66 ms. A filtered and sorted 100-record view measured 3.47, 3.94 and 3.99 ms. These times include the local query and a Rune headless frame, but exclude process startup, GPU work and presentation. One initial process/State/document frame took 56.45 ms. A jump to offset 500,000 plus a headless frame had p50 16.71 ms, p95 19.18 ms and p99/max 677.87 ms across 30 samples; one exceeded 20 ms. The next-page cursor action was not measured here.

A real wheel traversal reached the bottom of the 999-record page. Plain rows took 311 headless frames: p50 0.75 ms, p95 6.69 ms, p99 7.55 ms, max 7.84 ms; none exceeded 8.33 ms. Wrapped, frozen rows took 1,660 frames: p50 0.23 ms, p95 2.27 ms, p99 2.64 ms, max 3.44 ms; none exceeded 8.33 ms. These are headless wall times, not native presentation times. The complete headless benchmark peaked at 74,203,136 bytes RSS and 27,574,728 bytes footprint. These are process peaks, not per-frame measurements.

The separate native scratch host loaded 999 records and drove the Table's own scroll API. Its trace reported a 60 Hz display, not a validated 120 Hz display. For plain rows, 600 submitted frames had 598 presented and two skipped; present intervals were p50/p95 8.33 ms and max 16.67 ms. For wrapped, frozen rows, 596 were presented and four skipped; intervals were p50 25.00 ms, p95 33.33 ms and max 66.67 ms, with 462 intervals above the trace's 25 ms missed-frame threshold. The native wrapped case therefore fails the motion target. Native trace data does not isolate GPU execution time. Writing frame trace data may also affect pacing; its overhead remains under investigation. A prior native run loaded only 100 rows and is excluded; a headless run with a retained scroll offset failed its assertion and is also excluded. Earlier positive wheel deltas did not reach the bottom and are excluded as traversal evidence.

The Gallery preview batch now uses requested IDs and indexed record lookups; its query plan has a regression test, but Gallery frame timing remains unmeasured. Cold disk behavior, continuous million-row scrolling, wider schemas, and native 120 Hz presentation remain unverified. These small samples do not establish a frame-time guarantee.

A later trace-free native comparison collected 599 drawn wrapped/frozen frames in memory. Update wall time was p50 0.48 ms, p95 1.77 ms, p99 4.34 ms, max 16.30 ms; CPU encoding was p50 0.31 ms, p95 0.53 ms, p99 0.71 ms, max 4.77 ms. Submission gaps were p50 25.35 ms, p95 34.05 ms, p99 38.08 ms, max 330.25 ms, with one skipped frame. Trace writing alone does not account for the slow submission cadence. These gaps are not presentation or GPU timing. The frame hook reported 120 Hz, while the earlier trace header reported 60 Hz; the active display mode needs independent verification.

A separate storage-only rank probe used a disposable clone at `/private/tmp/moss-rank-probe-20261108`. Initial rank construction took 1,257.6 ms for one million records. Warm 100-row windows at the start, middle and end had medians 0.125, 0.107 and 0.137 ms and p95 0.128, 0.110 and 0.138 ms. Each matched the corresponding offset query; 977 rank buckets held at most 1,024 records. OS cache was uncontrolled. These results do not include a UI frame, and first-time preparation is not instant.

A later independent CoreGraphics check reported main display ID 1, 3,600 × 2,338 pixels, and 120 Hz through `CGMainDisplayID`, `CGDisplayCopyDisplayMode`, `CGDisplayModeGetPixelWidth`, `CGDisplayModeGetPixelHeight` and `CGDisplayModeGetRefreshRate`. This confirms the reported display mode at that later check; it does not establish the presentation cadence during the earlier traces.

The initial continuous-window path is limited to unfiltered, ungrouped, unsorted, unwrapped Table views in record-ID order. Other configurations continue to use the bounded page path. While a newly created row is pinned outside the current cache, it appears under “New items” at the end; its exact rank is not implied. A cache replacement defers while an edit is active. The one-million-row list uses leading and trailing spacers and a bounded cache, rather than a million-row UI index; its long-range precision and current native performance require the checks below.

A subsequent release probe used the built rank index and the continuous, plain Table on the same disposable million-record data. At the top, ordinal 500,000 and ordinal 999,999, the loaded IDs matched independent rank-window reads. The corresponding bounded caches held 256, 256 and 128 rows. One input-ready open and headless frame took 4.80 ms; the top, middle and end windows settled in 7.23, 14.93 and 12.66 ms respectively. These are single samples with uncontrolled OS cache, not percentile results. After the first bottom jump, the complete last row and New page control were visible. A UI title edit saved to the correct final record ID and the test restored the original title. The headless process, including two GPU snapshot readbacks, peaked at 107,724,800 bytes RSS and 259,900,088 bytes footprint. The scratch images are `/private/tmp/moss-table-perf-20261108/logical-middle.png` and `/private/tmp/moss-table-perf-20261108/logical-end.png`.

The native host gated each of its top, middle and end phases on the matching loaded record ID, then scrolled within each region. It submitted 246 frames in 5.81 seconds; CPU submission gaps had p50 16.66 ms, p95 22.76 ms, p99 25.40 ms and max 174.80 ms, with 245 of 245 above 8.33 ms. The trace header reported 60 Hz and all presentation timestamps were null. Rune documents null as a Metal-reported skipped drawable; the reason and window focus state were not established. This run does not establish actual presentation cadence or the 120 Hz target. Its process peak was 134,709,248 bytes RSS and 273,761,240 bytes footprint. The trace and log are `/private/tmp/moss-table-perf-20261108/logical-gated-final.jsonl` and `/private/tmp/moss-table-perf-20261108/logical-gated-final.log`.

The 38-point fixed-height rows are clipped to that height. Local row grouping avoids cumulative rounding after the large spacer, but f32 coordinates still yield up to 2 points of observed cell-height rounding near the last row; exact pixel geometry at that distance is not established. Wrapping, filter, sort and grouping still use paged views. The native wrapped fallback and continuous native Table have not met the requested frame budget. The frame update/encode timings exclude drawable acquisition and display pacing; slow submission or presentation cannot be attributed to rendering alone.

Rune's trace header captures the display-link rate at startup, when it may still contain a 60 Hz default. The running macOS display link takes its period from CoreVideo callbacks; the frame hook later reported approximately 120 Hz. Drawable acquisition and display pacing occur before the reported update and encode timings, so those timings cannot rule out time spent waiting there. No public frame-rate preference was found in the pinned Rune API.

An optional trace-free comparison used the supported process-only `RUNE_GPU_BACKEND=wgpu` override on the wrapped 999-row host. Across 600 drawn frames, update wall time was p50 0.44 ms, p95 2.59 ms, p99 3.44 ms and max 24.70 ms; CPU encoding was p50 1.20 ms, p95 1.62 ms, p99 2.35 ms and max 60.06 ms. Submission gaps were p50 33.26 ms, p95 36.95 ms, p99 49.62 ms and max 70.13 ms; all 599 exceeded 8.33 ms. The hook reported approximately 60 Hz here, versus approximately 120 Hz in the earlier Metal run. The different reported refresh rates make this an uncontrolled comparison, and submission gaps do not measure GPU completion or presentation. The log is `/private/tmp/moss-table-perf-20261108/native-wrap-wgpu.log`.

A follow-up native probe sampled its own AppKit state and found no active application or key window at frames 1 and 120. It was stopped after 359 trace records; `/private/tmp/moss-table-perf-20261108/logical-foreground-600.jsonl` and its matching `.log` are preserved, but this run is not foreground performance evidence. The host now requests focus through its own Rune window handle for the next controlled run. Profiling is paused while another task uses the foreground app.

A later read-only CoreGraphics/CoreVideo probe in the shell process returned success with zero active displays from `CGGetActiveDisplayList`, reported `CGMainDisplayID` 16, and returned `kCVReturnInvalidArgument` (-6661) with no link from `CVDisplayLinkCreateWithActiveCGDisplays`. Rune uses that creation call before selecting its window monitor. This reproduces the creation error in that process, but does not establish why its display list differed from the earlier main-display check or what happened in earlier runs. The probe created no window and changed no display setting. A valid focused timing run remains pending a verified active display.
CoreGraphics also reported three online displays, with IDs 1 and 16 marked asleep and inactive. Direct `CVDisplayLinkCreateWithCGDisplay` succeeded for each of those IDs, but no tick or presentation was tested. This supports an asleep-display explanation for the current active-display creation failure; it is not evidence for a production change or for the cause of earlier frame timing.

At 2026-11-08 08:01:21 AWST, the shell-process display probe below returned three online IDs `[16, 1, 17]`, zero active IDs, main ID 16, and `asleep=1` for IDs 1 and 16. Active-display link creation returned -6661 and no link; direct link creation for each of those IDs returned 0. It only reads display state, creates and releases transient link objects, and does not start a link or wake a display. Run it from a shell in the same session:

```sh
python3 - <<'PY'
import ctypes, datetime
cg = ctypes.CDLL('/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics')
cv = ctypes.CDLL('/System/Library/Frameworks/CoreVideo.framework/CoreVideo')
cf = ctypes.CDLL('/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation')
print('time', datetime.datetime.now().astimezone().isoformat())
for name in ('CGGetOnlineDisplayList', 'CGGetActiveDisplayList'):
    f = getattr(cg, name)
    f.argtypes = [ctypes.c_uint32, ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint32)]
    f.restype = ctypes.c_int32
    ids, count = (ctypes.c_uint32 * 16)(), ctypes.c_uint32()
    print(name, 'status', f(16, ids, ctypes.byref(count)), 'count', count.value, 'ids', list(ids[:count.value]))
cg.CGMainDisplayID.restype = ctypes.c_uint32
print('CGMainDisplayID', cg.CGMainDisplayID())
for name in ('CGDisplayIsActive', 'CGDisplayIsAsleep', 'CGDisplayIsOnline', 'CGDisplayIsBuiltin'):
    f = getattr(cg, name)
    f.argtypes, f.restype = [ctypes.c_uint32], ctypes.c_uint32
    print(name, [(i, f(i)) for i in (1, 16)])
cv.CVDisplayLinkCreateWithActiveCGDisplays.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
cv.CVDisplayLinkCreateWithActiveCGDisplays.restype = ctypes.c_int32
cv.CVDisplayLinkCreateWithCGDisplay.argtypes = [ctypes.c_uint32, ctypes.POINTER(ctypes.c_void_p)]
cv.CVDisplayLinkCreateWithCGDisplay.restype = ctypes.c_int32
cf.CFRelease.argtypes = [ctypes.c_void_p]
p = ctypes.c_void_p()
print('CVDisplayLinkCreateWithActiveCGDisplays', cv.CVDisplayLinkCreateWithActiveCGDisplays(ctypes.byref(p)), bool(p.value))
if p.value: cf.CFRelease(p)
for i in (1, 16):
    p = ctypes.c_void_p()
    print('CVDisplayLinkCreateWithCGDisplay', i, cv.CVDisplayLinkCreateWithCGDisplay(i, ctypes.byref(p)), bool(p.value))
    if p.value: cf.CFRelease(p)
PY
```

## Table scrolling, 14 November 2026

A release headless test used 3,000 records, eight properties, a 1,440 × 900 point window at 2× scale, and time-based wheel input. It ran 104 measured frames at each speed in the sequence 1,000, 2,000, 3,000, 5,000, 8,000, 13,000, 21,000, 34,000, 55,000, 89,000, 144,000, and 160,000 points per second. It checked visible row coverage, record identity, scroll direction, and the distance moved. The test data and logs were local scratch files; the OS cache and other processes were not controlled.

No loading indicators, missing visible rows, incorrect visible record titles, jumps, or reversals were observed in that run. At speeds through 89,000 points per second, all 104 measured CPU frames stayed below 8.33 ms. At 144,000, 76 frames exceeded 8.33 ms (p50 9.78 ms, p95 18.63 ms); at 160,000, 81 did (p50 10.17 ms, p95 19.10 ms). The scroll can reach a table boundary, so measured mean travel may be below the input speed. These timings are headless CPU measurements, not native presentation or GPU timing, and do not establish sustained 120 Hz.

An isolated experiment reduced Rune's off-screen row preparation. At 160,000 points per second it still exceeded the frame budget in 52 of 104 frames (p50 8.36 ms, p95 11.10 ms). It was not added to the project. An uncached jump in a table larger than the data window can still show placeholder rows while the requested records load; this case is not covered by the no-loading result above.
