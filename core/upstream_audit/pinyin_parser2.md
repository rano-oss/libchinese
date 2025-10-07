# Audit: libpinyin/src/storage/pinyin_parser2.cpp

Source: https://github.com/libpinyin/libpinyin/blob/master/src/storage/pinyin_parser2.cpp

Purpose
-------
Extract the exact algorithmic behavior, constants, and tie‑breaking rules implemented by the upstream C++ pinyin parser so we can implement a behaviorally identical Rust version and verify parity via tests.

This document summarizes:
- the DP segmentation algorithm and its state,
- the `parse_one_key` behavior (including tone handling),
- the precise tie-breakers used when choosing among candidate transitions,
- the backtrace/finalization strategy,
- special-case handling (apostrophes, incomplete pinyin, options),
- implications and a prioritized test vector list to port.

High-level description
----------------------
The upstream `FullPinyinParser2` performs dynamic programming over input pinyin characters to produce a segmentation into valid "keys" (ChewingKey entries). For each character position it considers candidate substrings up to a maximum syllable length, verifies whether the substring is a valid syllable (via `parse_one_key` / index lookup), and updates a per-position best-state structure according to a deterministic set of tie-breakers.

Important constants
-------------------
- `max_full_pinyin_length = 7`  — maximum substring length to consider for full pinyin (includes tone).
- `max_double_pinyin_length = 3` — maximum for double pinyin.
- `max_chewing_length = 4` — maximum chewing length (historic).
- Beam/limits are handled differently in lookup code — parser uses full DP up to the max lengths above.

DP state (per-position)
-----------------------
The parser stores, per index/step, a `parse_value_t` that contains:
- `ChewingKey m_key` — the key for the matched syllable.
- `ChewingKeyRest m_key_rest` — raw begin/end positions for that syllable.
- `gint16 m_num_keys` — number of segments used to reach this position.
- `gint16 m_parsed_len` — total characters parsed/covered by valid syllables so far.
- `gint16 m_distance` — accumulated distance/penalty from index table entries.
- `gint16 m_last_step` — pointer to previous step index for backtrace (-1 if none).

parse_one_key behavior (core substring matcher)
----------------------------------------------
- Accepts flags in `options` (notably `USE_TONE`, `FORCE_TONE`, `PINYIN_INCOMPLETE`).
- If `USE_TONE` is set: it will check the last char of the substring; if it is `'1'..'5'` it treats that as a tone, reduces the effective parsed length by 1 and records tone position. If `FORCE_TONE` is set and no tone is present, the match is rejected.
- The function calls `search_pinyin_index2(options, m_pinyin_index, m_pinyin_index_len, input, key, distance)`:
  - This performs a table lookup `pinyin -> index entry`, returning a `ChewingKey` and an integer `distance`.
  - If the lookup fails, `parse_one_key` returns false (no match for that substring).
- If matched and `USE_TONE` is set and the tone position equals parsed position, sets `key.m_tone = tone` and increments parsed length accounted for (so tone counted as part of parsed substring).
- `distance` returned from the index entry is an integer penalty value that gets added to the DP state's `m_distance`.
- `parse_one_key` returns whether the substring was recognized as a key (true) or not.

Core DP loop / recurrence
--------------------------
For an input string of length `len` the code:
1. Allocates an array `m_parse_steps` of length `len + 1` of `parse_value_t` and initializes them (`m_last_step = -1`).
2. Sets the base state at end `best_cost` equivalent: `m_parse_steps[len]` is the DP base (0 parsed_len, 0 keys, 0 distance, m_last_step = -1).
3. Iterates positions `i` from 0..len-1:
   - If `input[i]` is `'\'` (apostrophe), propagate `curstep` to `nextstep` (special handling, see below).
   - Otherwise, find the next apostrophe position `next_sep` (the substring is bounded by the next apostrophe).
   - For each substring start `m = i` and end `n` in `m+1 .. min(m + max_full_pinyin_length, next_sep)`, do:
     - Call `parse_one_key(options, key, distance, input + m, onepinyinlen)` where `onepinyinlen = n - m`.
     - If `parsed`:
       - Build `value`:
         - `value.m_num_keys = curstep->m_num_keys + 1`
         - `value.m_parsed_len = curstep->m_parsed_len + onepinyinlen`
         - `value.m_distance = curstep->m_distance + distance`
         - `value.m_last_step = m`
       - Compare `value` against stored `nextstep` at position `n` and possibly replace according to tie-breakers (see below).
4. After processing all positions, call `final_step` to pick the final `curstep` and backtrace using `m_last_step` to produce the ordered sequence of keys.

Tie-breaking rules (how a `value` replaces a stored `nextstep`)
---------------------------------------------------------------
When a candidate `value` for `nextstep` is considered, replacement logic is:

1. If `nextstep` has no previous result (i.e., `nextstep->m_last_step == -1`), accept `value`.
2. Else prefer the candidate with **larger `m_parsed_len`** (i.e. more input covered by valid syllables).
3. If `m_parsed_len` ties, prefer **smaller `m_num_keys`** (fewer segments).
4. If both `m_parsed_len` and `m_num_keys` tie, prefer **smaller `m_distance`** (lower accumulated distance).
5. (Disabled/optional behavior in code) There is an additional conditional preference to prefer an `'a'` at clause end in certain cases — present in the original code but wrapped in `#if 0`.

Important: the primary tie-breaker is `m_parsed_len` (maximize parsed coverage). Only after equal coverage does the algorithm prefer fewer keys, then smaller distance.

Backtrace / finalization (`final_step`)
---------------------------------------
- After DP fill, `final_step` scans backwards from `step_len - 1` to find the first `i` such that `i == curstep->m_parsed_len`. That `curstep` is treated as the final best state (this enforces the “maximum parsed length” at the final position).
- It then extracts `num_keys` and performs a standard backtrace by following `m_last_step` pointers to collect the `ChewingKey` and `ChewingKeyRest` items in reverse order.
- The function returns `parsed_len` — the total parsed characters for the chosen segmentation.

Distance semantics
------------------
- `distance` is an integer read from the index entry (`index->m_distance`) via `search_pinyin_index2`.
- It represents a table-specific penalty (e.g., entries that are approximate, lower-confidence, or fuzzy alternatives may carry larger distances).
- The parser sums these integers into `m_distance` and uses `m_distance` only as a late (lowest-priority) tie-breaker.
- Precisely reproducing `distance` semantics requires either:
  - importing the upstream generated index/table (`pinyin_parser_table.h`) and reading `m_distance` values, or
  - generating an equivalent mapping in the Rust trie that assigns the same distances for every syllable used in parity tests.

Special handling and options
----------------------------
- Apostrophes (`'`):
  - Treated as explicit separators. When the DP loop sees a `'` at `input[i]`, it propagates the `curstep` to `nextstep` at `i+1` with `m_parsed_len + 1`, same `m_num_keys` and same `m_distance`, and sets `m_last_step = i`. This ensures apostrophes break segments predictably.
- Incomplete pinyin:
  - Index lookups/flags may allow incomplete pinyin entries depending on `options & PINYIN_INCOMPLETE`. `check_pinyin_options` filters index entries by `IS_PINYIN`, `PINYIN_INCOMPLETE`, and correctness flags.
- Tone handling:
  - If `USE_TONE` is set, the parser identifies tones appended to syllables. `FORCE_TONE` requires tones to be present.
- Multiple schemes:
  - There are different table/scheme variants (HANYU, LUOMA, etc.). The scheme determines which `m_pinyin_index` to use. For parity testing start with the default full pinyin scheme used in upstream tests.

Edge cases and failure modes
----------------------------
- If no valid segmentation is found that covers a character position, the DP may propagate a state with smaller `parsed_len` and the parser may return a `parsed_len` less than input length.
- `final_step` chooses the best `curstep` by matching position index `i` with `curstep->m_parsed_len` to ensure we choose a consistent final state.
- The upstream parser asserts that certain invariants hold (e.g., range lengths) — care is needed in Rust to avoid panics on malformed inputs.

Implications for Rust port (parity checklist)
--------------------------------------------
To achieve parity with upstream behavior in tests, implement the following:

1. DP State and recurrence:
   - Implement a per-position DP array equivalent to `m_parse_steps` with fields: `num_keys`, `parsed_len`, `distance`, `last_step` and the matched token (syllable string or canonical key).
   - Iterate forward and consider candidate substrings bounded by a `max_full_pinyin_length` (7) and next apostrophe.

2. parse_one_key equivalence:
   - For parity tests that depend on `distance`, either:
     - a) Use the upstream generated pinyin table (import it as data), or
     - b) Build a trie that, for each exact syllable used in tests, returns the same `distance` as upstream. This is necessary because `distance` affects tie-breaking in corner cases.
   - Tone extraction: support `USE_TONE` / `FORCE_TONE` semantics if tests include tones.

3. Exact tie-breakers:
   - Primary: maximize `parsed_len`.
   - Secondary: minimize `num_keys`.
   - Tertiary: minimize `distance`.
   - Implement these comparisons exactly and deterministically.

4. Apostrophe handling:
   - Treat `'` as explicit separators that propagate state to the next position with parsed_len incremented by 1 and no additional distance.

5. Final selection/backtrace:
   - Implement `final_step` behavior: find `i` such that `i == parsed_len` scanning from the end backward, then backtrace via `last_step` to reconstruct keys in order.

6. Tests to port (priority):
   - `tests/storage/test_parser2.cpp` — segmentation test vectors:
     - Simple cases: "nihao" -> ["ni","hao"], "zhongguo" -> ["zhong","guo"].
     - Ambiguous inputs where multiple segmentations are possible and tie-breakers decide the result (e.g., inputs that can be `si-ang` vs `s-iang`).
     - Apostrophe cases: inputs with `'` that enforce different splits.
     - Tone cases if present in upstream tests.
   - These test vectors will help ensure the DP and tie-breakers match exactly.

Files / symbols to consult next (for full fidelity)
---------------------------------------------------
- `pinyin_parser_table.h` — generated table mapping pinyin strings → index entries (including `m_distance`), needed if you want entry-level distance parity.
- `pinyin_index_item_t` / `content_table` definitions in the upstream source to map how `m_distance` and `m_table_index` are encoded.
- `pinyin_phrase3.h` and `chewing_key.h` — the data structures for phrase tokens and keys used downstream by lookup.
- Upstream tests: `tests/storage/test_parser2.cpp` and other parser-related tests to extract authoritative input→expected segmentation tuples.

Pseudocode of the DP core (behavioral)
--------------------------------------
This pseudocode mirrors the upstream control flow and tie-breakers:

```text
let n = input.len()
let steps = vec![ParseValue::default(); n+1]
steps[n] = ParseValue { num_keys:0, parsed_len:0, distance:0, last_step:-1 }

for pos in 0..n {
  if input[pos] == '\'' {
    propagate steps[pos] -> steps[pos+1] with parsed_len + 1, last_step = pos
    continue
  }

  let next_sep = index of next '\'' or n
  let try_len_max = min(pos + max_full_pinyin_length, next_sep)
  for end in (pos+1..=try_len_max) {
    if parse_one_key(options, input[pos..end]) returns (key, dist) {
      let cur = steps[pos]
      cand.num_keys = cur.num_keys + 1
      cand.parsed_len = cur.parsed_len + (end - pos)
      cand.distance = cur.distance + dist
      cand.last_step = pos

      // replacement according to upstream tie-breakers:
      if steps[end].last_step == -1 {
        steps[end] = cand
      } else if cand.parsed_len > steps[end].parsed_len {
        steps[end] = cand
      } else if cand.parsed_len == steps[end].parsed_len {
        if cand.num_keys < steps[end].num_keys {
          steps[end] = cand
        } else if cand.num_keys == steps[end].num_keys {
          if cand.distance < steps[end].distance {
            steps[end] = cand
          }
        }
      }
    }
  }
}

// final step: find i from n downwards such that i == steps[i].parsed_len
// then backtrace from steps[i] using last_step to accumulate keys
```

Notes on implementing `distance` parity
--------------------------------------
- If you plan to rely purely on a Trie of syllables built from a textual syllable list, you must ensure the Trie yields `distance` values compatible with upstream. For many tests the `distance` tie-breaker won't be exercised, but there are ambiguous inputs where it matters.
- Best practice for strict parity: either import the upstream `pinyin_parser_table.h` (or an extracted JSON/CSV of entries) into the Rust model, or port the table-generation script to reproduce the same `m_distance` mapping.

Suggested workflow to reach parity
----------------------------------
1. Audit and extract the small set of upstream segmentation test vectors from `tests/storage/test_parser2.cpp`.
2. Implement trie + DP per pseudocode above, using a default `distance = 0` for all syllables initially.
3. Run segmentation tests. For cases failing only in the `distance` tie-breaker, extract required `distance` values from upstream `pinyin_parser_table.h` and add them to the Trie nodes used by those tests.
4. Iterate until segmentation vectors are identical.

Appendix: immediate priority test vectors to port (examples)
------------------------------------------------------------
- "nihao" -> ["ni", "hao"]
- "zhongguo" -> ["zhong", "guo"]
- an ambiguous case that the upstream test contains (port exact string and expected segmentation)
- apostrophe-containing case (e.g., "zhe'yang" style) — port upstream example
- tone-containing case if present in upstream tests (e.g., "ni3hao3" or "ni3hao0" variants)

End of audit
------------
This document provides a behavior-level specification extracted from `pinyin_parser2.cpp`. With the DP recurrence, state layout, and exact tie-breaker ordering reproduced in Rust, we can achieve deterministic parity for segmentation tests. The only remaining fidelity detail for some corner cases is the `distance` table; for those we should import or reproduce the same pinyin index table entries.