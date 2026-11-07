# First storage measurements

Date: 2026-11-07. Apple M4 Mac, 16 GiB RAM. Release storage build, two Cargo jobs. These results do not prove a visible viewport or cold-start target.

Fixture: 100,000 pages, 299,997 text blocks, and one database with 1,000,000 records. The database file is about 1.5 GiB. The probe checks row counts and uses the production Store methods. Each repeated case has 30 measured samples after warm-up. The OS cache is not controlled.

Command:

```sh
cargo run --release -p rotion-core --example storage_bench -- --path /tmp/rotion-storage-full.sqlite
# Reuse the fixture after a code change:
cargo run --release -p rotion-core --example storage_bench -- --reuse --path /tmp/rotion-storage-full.sqlite
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
