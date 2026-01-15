# Issue 1.4 Indexing + Constraints

## Code map

- Catalog + metadata: `crates/query/src/execution/planner.rs` (`Catalog`, `TableInfo`, `IndexInfo`)
- Tuple format + RID: `crates/query/src/execution/seq_scan.rs` (`Tuple` encoding, `Rid`, slotted pages)
- Volcano executor traits + plan nodes: `crates/query/src/execution/operator.rs`, `crates/query/src/execution/planner.rs`, `crates/query/src/logical_plan.rs`
- Storage APIs + page layout: `crates/storage/src/page.rs`, `crates/storage/src/buffer.rs`, `crates/query/src/execution/seq_scan.rs`
- WAL: no durable WAL implementation present; index recovery uses rebuild
- Buffer pool pin/unpin: `crates/storage/src/buffer.rs`

## B+Tree design

### Page types

- **Header page** (`PAGE_TYPE_HEADER`): root page id, key type, key size, unique flag, composite key count, text key size, composite key type list.
- **Internal page** (`PAGE_TYPE_INTERNAL`):
  - header: page type, key count, parent page id, left-most child pointer
  - body: `[key, child]` pairs sorted by key
- **Leaf page** (`PAGE_TYPE_LEAF`):
  - header: page type, key count, parent page id, next leaf pointer
  - body: `[key, rid]` pairs sorted by key

### Key encoding

- `INT/BIGINT/TIMESTAMP`: 8-byte little-endian `i64`
- `TEXT`: fixed-size payload (default 128 bytes) with a 2-byte length prefix and zero padding
- `COMPOSITE`: concatenation of component encodings in column order (each component uses its native encoding)
- Ordering uses `IndexKey` comparisons (numeric or lexical string ordering); composite keys are lexicographic by component and padding does not affect compare.

### Split policy (insert only)

- Insert into a leaf in sorted order.
- On overflow, split by count (`mid = len / 2`), keep left half in-place, move right half to a new leaf.
- Separator key is the first key in the right leaf and is inserted into the parent.
- Internal overflow uses the same split rule; the middle key is promoted to the parent.
- Root split creates a new internal root and updates the header page.

### Invariants

- Leaf links (`next_leaf_page_id`) define a stable in-order traversal.
- Internal nodes keep `children.len() == keys.len() + 1`.
- Keys inside a page are strictly sorted; duplicates are adjacent and deterministic.
- Composite keys preserve column-order lexicographic sorting across leaf scans.

## Uniqueness + constraints

- `TableInfo::insert_tuple` probes unique indexes first.
- If any duplicate is found, insert fails with `ExecutionError::ConstraintViolation` containing table, index name, and key.
- Insert order: **probe -> heap insert -> index insert**.
- If any index insert fails after the heap insert, the tuple slot is marked deleted (slot length = 0).
- `TableInfo::update_tuples` revalidates unique indexes against the new key values, deletes old index entries, and inserts new ones.
- Index delete is supported (`Index::delete`) to support index key movement on updates.

## Update execution

- `LogicalPlan::Update` is executed by `Update`, which delegates to `TableInfo::update_tuples`.
- Updates apply assignments per tuple, write back to the heap (in-place when possible), and repair index entries.

## Recovery strategy

- No WAL for indexes is implemented in this codebase.
- `TableInfo::rebuild_indexes` recreates each index by scanning the heap and re-inserting keys.
- Tests simulate rebuild to ensure constraints still enforce after rebuild.

## Performance evidence

- Test: `index_scan_touches_fewer_pages_than_seq_scan` in `crates/query/src/execution/tests.rs`
  - Dataset: 10,000 rows, equality predicate on indexed column
  - Assertion: indexed scan touches **>5x fewer** buffer pool fetches than seq scan
  - Reproduce: `cargo test -p query index_scan_touches_fewer_pages_than_seq_scan`
- Test: `index_scan_uses_fewer_page_fetches_than_seq_scan` in `crates/query/tests/index_perf.rs`
  - Dataset: 50,000 rows, equality predicate on indexed column
  - Assertion: indexed scan touches **>=20x fewer** buffer pool fetches than seq scan
  - Reproduce: `cargo test -p query --test index_perf`

## Debugging tips

- Use `BPlusTree::iter_all()` to verify in-order traversal.
- Use `BPlusTree::height()` to confirm split behavior.
- Use `BPlusTree::key_types()` to confirm composite key metadata.
- Use `BufferPoolManager::fetch_count()` to compare scan strategies.
