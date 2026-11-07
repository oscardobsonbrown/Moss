# Performance contract: working draft

Status: requirement record. No Rotion benchmark has run. App implementation has not started.

## Confirmed requirement

On this Mac, opening ordinary content must take **less than 8.8 ms**, including disk reads. Test a workspace with **100,000 pages** and **one database containing 1 million rows**.

For the single-million-row database case, the real first viewport must be ready for input **within 20 ms**, including disk reads. This replaces the original 8.8 ms limit for that case. Ordinary content keeps the 8.8 ms target.

This content-open rule applies while Rotion is running. It includes uncached disk reads.

Cold start has a separate **300 ms** target, from launch request to the real first viewport ready for input, at the same workspace size on this Mac.

After Quit, startup restores only the last active workspace window and its tabs. Measure the 300 ms target against that restored window's first real viewport. Other workspace windows remain closed.

The result must be the real first viewport, ready for input. Text and database values must be correct. A blank shell, spinner, or placeholder does not pass. Content outside the viewport can load as needed.

On a pointer-triggered tab switch, content appears immediately while only the active-tab indicator animates. This animation must not delay content readiness. Keyboard-triggered tab switches are fully instant. All database view switches are instant.

This target must not become a cache-only target. A test must include data that is not already held in the app's memory.

Remote embeds are exempt from the 8.8 ms target. Their network work must not block local content or input.

Map tiles are also exempt. Saved locations, markers, and controls must be ready within 8.8 ms. Tiles may load in the background. Cached tiles must be available offline. Map tile work must not block local work or input.

Local images, PDFs, and videos may show a saved preview within 8.8 ms while full-resolution content loads in the background. The preview must show real file content, and controls must be ready for input. This is a readiness rule for local media, not an exemption from the timing target.

## Measurements still to define

Observed test hardware: Apple M4, model Mac16,1, 16 GiB RAM. OS and display details will be recorded with each test run.

- Exact start event and end event. Record input receipt, disk work, data readiness, layout, frame submission, presentation, and input readiness separately where possible.
- Data distribution across databases, page lengths, relation depth, formula cost, group counts, and media sizes.
- Display refresh rate, window dimensions, backing scale, app build mode, and Rune revision.
- App restart, OS cache state, background load, and first-open conditions.
- Test runs, repetitions, and handling of OS scheduling delays. Do not silently discard slow results.
- Memory and disk-space limits.

## Reporting rules

- Report the worst result and all target misses. A fast average does not prove that every tested open passes.
- Report percentiles as supporting data. Do not use a percentile to replace the confirmed target without a user decision.
- Separate a running-app test from a restarted-app test. Distinguish app-memory, OS-cache, and physical-disk conditions.
- Include filtered, sorted, grouped, linked, and formula-backed views. Do not test only a plain table.
- Record correctness and input readiness. A fast picture with stale values or blocked input fails.
- For local images, PDFs, and videos, measure real preview readiness and input readiness against the 8.8 ms target. Report full-resolution readiness separately. Define preview generation, missing-preview behavior, and audio readiness before final acceptance tests.
- A screenshot sequence or a low-frame-rate screen recording cannot establish an 8.8 ms result. Use app instrumentation and a suitable presentation measurement.

## Technical limit

The target is not an established guarantee. macOS can delay a process, and disk or rendering work can exceed this limit. The prototype must provide evidence. A missed requirement must cause a design decision, a revised implementation, or an explicit change from the user. It must not disappear from the report.
