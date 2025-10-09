# Audit: libpinyin/src/storage/ngram.cpp

Source: https://github.com/libpinyin/libpinyin/blob/master/src/storage/ngram.cpp

Purpose
-------
Extract the algorithmic behavior, data formats and important constants used by the upstream n-gram / single-gram / bigram handling so we can reproduce equivalent behavior in Rust with high fidelity. This file summarizes the runtime semantics (lookup/merge/search), the on-disk/in-memory layout expectations visible in the code, and the training/test artifacts to port for parity validation.

High-level responsibilities observed
-----------------------------------
- SingleGram: compact container that stores a total frequency followed by an array of (token, freq) items. Provides APIs to read total frequency, retrieve token freq, insert/set/remove items, search for ranges and iterate all items.
- Bigram: wrapper around many SingleGram instances and a persistence layer (attach/store/load). The code demonstrates writing SingleGram to a simple attached DB.
- Merge semantics: `merge_single_gram` merges two SingleGram objects (system + user) into a merged SingleGram by summing frequencies of identical tokens and concatenating remaining items while preserving token order.
- Lookup/Generation usage: lookup code in `pinyin_lookup2.cpp` uses single-gram data to:
  - compute bigram conditional probabilities (as freq / total_freq),
  - generate per-candidate probabilities (bigram or unigram mixed with pinyin matching probability),
  - apply thresholds (compare to DBL_EPSILON/FLT_EPSILON) to ignore zero/near-zero probabilities.

Observed data layout & structures
--------------------------------
- SingleGram binary layout:
  - 32-bit total frequency stored at the beginning
  - Followed by zero or more `SingleGramItem` entries:
    - `phrase_token_t m_token` (type depends on upstream; typically u32/int)
    - `guint32 m_freq` (frequency count)
  - This layout is implemented as a contiguous memory blob accessed through `MemoryChunk`.
- Items are stored in ascending order by token id (`token_less_than` is used).
- The `get_length` function computes number of items by pointer arithmetic on begin/end addresses.

Important functions & semantics
------------------------------
- `SingleGram::get_total_freq(guint32 & total)`:
  - Reads the total frequency stored at offset 0 (u32).
- `SingleGram::set_total_freq(guint32 total)`:
  - Writes the total frequency.
- `SingleGram::get_freq(phrase_token_t token, guint32 & freq)`:
  - Binary search (lower_bound) among items; returns freq if found.
- `SingleGram::insert_freq(token, freq)`:
  - Inserts a new `SingleGramItem` at the correct sorted position. If token exists, insertion returns false (no duplicate insertion).
- `SingleGram::set_freq(token, freq)`:
  - Updates an existing item's frequency; returns false if not found.
- `SingleGram::remove_freq(token, freq)`:
  - Finds and removes an item, returning its freq.
- `SingleGram::search(PhraseIndexRange * range, BigramPhraseArray array)`:
  - Returns items whose token is within [range_begin, range_end) — used to iterate continuations (for bigram generation).
  - Populates `m_freq` as frequency normalized by `total_freq` to get a probability-like value (float).
- `SingleGram::retrieve_all(BigramPhraseWithCountArray array)`:
  - Appends all items with token and both raw count and normalized frequency = m_freq / total_freq.

- `merge_single_gram(merged, system, user)`:
  - If either side NULL, copy the other.
  - Otherwise:
    - Initialize merged chunk with new total = system_total + user_total.
    - Merge two sorted arrays by token (like a typical merge in merge-sort).
    - For equal tokens, sum the frequencies into a single `SingleGramItem`.
  - Resulting merged list remains sorted and contains summed frequencies.

- `PinyinLookup2` usage (integration points):
  - When generating bigram candidates, the merged SingleGram is searched to produce `bigram_poss` = freq / total_freq (a float).
  - For unigram, `elem_poss` is taken from `PhraseItem.get_unigram_frequency()` and normalized by phrase_index total freq.
  - `unigram_gen_next_step` computes:
    - `next_step.m_poss = cur_step->m_poss + log(elem_poss * pinyin_poss * unigram_lambda)`
  - `bigram_gen_next_step` computes:
    - `next_step.m_poss = cur_step->m_poss + log((bigram_lambda * bigram_poss + unigram_lambda * unigram_poss) * pinyin_poss)`
  - These expressions show the order of combining phrase frequency, conditional (bigram) probability, and pinyin-pronunciation probability; then taking `log()` to accumulate scores in log-space.

Thresholds and numeric considerations
-------------------------------------
- When probabilities are extremely small, the C++ code compares to `DBL_EPSILON` / `FLT_EPSILON` to avoid numerical underflow and to skip zero-prob candidates.
- Probabilities are floats (gfloat, which maps to float) and sometimes doubles; be mindful to preserve precision where upstream uses double.
- `get_poss` expressions multiply several probabilities and then take `log` — ensure in Rust we follow the same multiplicative combination before converting to log space (or consistently compute in log-space with correct transforms).

Training & tests present in repo (to port)
------------------------------------------
- `tests/storage/test_ngram.cpp`:
  - Uses `SingleGram` to insert and retrieve frequencies, validate ordering, check save/load via `Bigram::attach` and `bigram.store`.
  - Test expectations:
    - After insert/insert_freq/set_freq/remove_freq, the frequencies and totals match expected values.
    - `search` returns items with normalized frequency values (m_freq / total_freq) in ascending token order within the given token range.
  - This test is an excellent parity candidate because it exercises the binary layout semantics and the merge/store semantics.

- Training utilities (in other upstream files under utils/training):
  - Tools to compute counts-to-probabilities and estimate interpolation weights are present upstream (e.g., `estimate_interpolation.cpp`, `gen_ngram`).
  - Upstream training typically:
    - Counts collected from corpus,
    - Optionally apply add-k smoothing or other smoothing,
    - Convert counts to probabilities (counts / total),
    - Store conditional probabilities for bigram/trigram as P(w2 | w1) = count(w1,w2) / count(w1) — sometimes with add-k smoothing and a V-like correction for the continuation count.

Porting implications & parity checklist
---------------------------------------
To reach parity with upstream behavior for n-gram and bigram single-gram handling, follow these steps:

1. Reproduce SingleGram binary layout and behavior:
   - Model a single-gram blob that stores total freq (u32) + sorted list of (token:u32, freq:u32).
   - Implement lookup, insert, remove, set, and range search that behave like the C++ functions (binary search, sorted insertion).
   - Implement `retrieve_all` returning both count and normalized frequency.

2. Implement `merge_single_gram` semantics precisely:
   - Produce merged total = system_total + user_total.
   - Merge sorted item lists summing frequencies for identical tokens.
   - Preserve ordering and exact integer arithmetic used upstream (u32 saturation isn't explicitly used upstream; check for overflow guards — upstream uses checks before adding).

3. Implement probability normalization and usage as in `pinyin_lookup2`:
   - `bigram_poss = freq / (gfloat) total_freq`
   - `elem_poss = unigram_frequency / phrase_index_total_freq`
   - Multiply by `pinyin_poss` and interpolation weights, then `log()` to accumulate into candidate `m_poss`.
   - Apply the same thresholds for near-zero probabilities to avoid generating candidate steps with zero possibility.

4. Training conversions:
   - Provide identical counts→prob conversions where necessary for tests:
     - Unigram: p = count / total (add-k smoothing if upstream uses it in certain tools).
     - Bigram: p(w2|w1) = (count(w1,w2) + k) / (count(w1) + k*Vw1) — if upstream training tools use add-k.
   - Port training utilities where parity of numeric values matters (e.g., estimate interpolation weights).

5. Tests to port:
   - `tests/storage/test_ngram.cpp` — port as Rust integration/unit tests verifying:
     - insert/get/remove semantics and token ordering,
     - normalized frequency computations,
     - persistence via Bigram attach/store/load flow (or simulate with an in-memory store).
   - Additional tests that rely on merged single-gram behavior used by lookup (these will surface during lookup parity tests).

Edge cases & caveats
--------------------
- Overflow: upstream uses `guint32` and contains checks in training code for overflow conditions (e.g., comparing `seed > 0 && total_freq > total_freq + seed` before addition). Reproduce similar guards or adopt saturating arithmetic if necessary.
- Floating vs double: upstream mixes float/double (`gfloat`, `gdouble`) depending on context. Tests that compare numeric results might be sensitive to precision. Use the same type where practical or allow small epsilons in assertions.
- DB/persistence format: `Bigram::attach("/tmp/test.db", ATTACH_CREATE|ATTACH_READWRITE)` indicates a simple on-disk format managed by the upstream `Bigram` class. If you need exact file-level compatibility, port or parse the underlying DB format; otherwise, implement equivalent persistence semantics for tests (or mock the store in-memory).
- Continuation counts / Vw1: upstream bigram log-prob training approximates the denominator for add-k smoothing by using the number of distinct continuations for w1. That approximation must be reproduced if training parity is required.

Pseudocode (merge_single_gram)
------------------------------
This pseudocode captures the merge semantics implemented upstream:

```
function merge_single_gram(merged, system, user):
    if system is NULL and user is NULL: return error
    if system is NULL: merged.data = copy(user.data); return
    if user is NULL: merged.data = copy(system.data); return

    system_total = system.total_freq
    user_total = user.total_freq
    merged_total = system_total + user_total
    merged.items = empty

    i = 0; j = 0
    while i < len(system.items) and j < len(user.items):
        if system.items[i].token < user.items[j].token:
            merged.items.append(system.items[i])
            i += 1
        else if system.items[i].token > user.items[j].token:
            merged.items.append(user.items[j])
            j += 1
        else:
            merged_item.token = system.items[i].token
            merged_item.freq = system.items[i].freq + user.items[j].freq
            merged.items.append(merged_item)
            i += 1; j += 1

    append remaining items from system or user
    write merged.total_freq and merged.items into merged chunk
    return success
```

Recommended immediate actions for parity work
-------------------------------------------
1. Port `tests/storage/test_ngram.cpp` to Rust (`core/tests/ngram_tests.rs`) and make it assert the same results as upstream.
2. Implement `SingleGram` in Rust (an in-memory struct wrapping a Vec<(token,u32)> plus total) matching insert/search/remove semantics. Use this to run the ported tests.
3. Implement `merge_single_gram` in Rust and add tests that reproduce the upstream merging behavior (use the same inputs from test code).
4. Integrate with `pinyin_lookup` parity tests later (after segmentation tests) to ensure probabilities are consumed the same way.

References
----------
- `src/storage/ngram.cpp` (upstream implementation under audit)
- `tests/storage/test_ngram.cpp` (unit/functional tests that should be ported)
- `src/lookup/pinyin_lookup2.cpp` (shows how `bigram_poss`, `elem_poss`, and `pinyin_poss` are combined)

End of audit
------------
This file captures the critical runtime semantics and the prioritized checklist to reach parity with respect to the upstream n-gram/bigram/single-gram behavior. The next step is to implement `SingleGram` and `merge_single_gram` in Rust, port the test(s) in `tests/storage/test_ngram.cpp`, and iterate on numeric compatibility (floats vs doubles, and exact normalization behavior).