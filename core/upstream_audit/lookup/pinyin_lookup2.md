# Audit: libpinyin/src/lookup/pinyin_lookup2.cpp

Source: https://github.com/libpinyin/libpinyin/blob/master/src/lookup/pinyin_lookup2.cpp

Purpose
-------
Extract the lookup algorithm (Viterbi / beam-like dynamic programming + candidate generation),
scoring formulas, heap/beam mechanics, and training update heuristics used by upstream
`PinyinLookup2` so we can port behavior to Rust with deterministic parity for tests.

This document captures:
- the data structures used for candidates and steps,
- the per-step generation and pruning (top-N by heap) semantics,
- the exact scoring formulas and interpolation,
- tie-break rules when merging candidate entries,
- the training heuristics used to update user bigram/unigram data,
- a parity checklist and recommended tests.

Top-level behavior summary
--------------------------
`PinyinLookup2` performs a constrained dynamic programming over phonetic-key matrix positions
(steps). For each step it maintains:
- a mapping (hash table) from a lookup key (phrase token) to an index into an array of candidate entries for that step (`LookupStepIndex`),
- an array of `lookup_value_t` entries (`LookupStepContent`) representing candidates for that step.

Processing flow:
1. Pre-populate step 0 with prefix tokens (single-token starts) using `populate_prefixes`.
2. For each step i from 0..nstep-2:
   - Build a candidate list from the current step's `LookupStepContent`.
   - Use a max-heap to select the top `nbeam` candidates (`get_top_results`).
   - For each top candidate, run `search_bigram2` and `search_unigram2` which in turn call
     `bigram_gen_next_step` / `unigram_gen_next_step` to produce candidate entries for later steps.
   - `save_next_step` inserts or merges new candidates into the `next_step` arrays using explicit tie-break rules.
3. After filling steps, `final_step` selects the best final candidate (preferring longer aggregate phrase length, then higher probability) and backtraces to produce `MatchResult`.

Key constants & parameters
--------------------------
- nbeam = 32
  - The maximum number of top candidates extracted from a step to expand forward.
- Scoring uses natural log (`log()`) to accumulate additive scores (`m_poss`).
- Interpolation: a per-instance `bigram_lambda` is provided; `unigram_lambda` = `1 - bigram_lambda`.
  - Combined probability is (bigram_lambda * bigram_poss + unigram_lambda * unigram_poss) before multiplying by pronunciation probability and taking log.

Primary data types (conceptual)
-------------------------------
- lookup_value_t
  - `m_handles[2]` — the two phrase token handles used to track prior & current tokens.
  - `m_length` — accumulated character length for the candidate.
  - `m_poss` — accumulated score (log-space) for the candidate.
  - `m_last_step` — index of the prior step (for backtracing)
- LookupStepContent: array/list of `lookup_value_t` for a particular step position
- LookupStepIndex: hash table mapping `lookup_key_t` (token) -> index into LookupStepContent for dedup/merge logic
- `steps_index` and `steps_content`: arrays (length nstep) of the per-step index/table and content array.

Candidate generation & pruning details
--------------------------------------
- `populate_candidates` clones the `LookupStepContent` into a GPtrArray `candidates`.
- `get_top_results` builds a max-heap over candidate pointers with comparator `lookup_value_less_than` (compares `m_poss`) and repeatedly pops to collect up to `nbeam` top candidates.
  - This implements a local per-step beam extraction: only the top N candidates are expanded by ngram lookup, limiting combinatorial explosion.
- For each top candidate, both `search_bigram2` and `search_unigram2` attempt to extend the candidate into new phrase tokens for subsequent steps:
  - `search_bigram2` merges system and user bigrams (via `merge_single_gram`), iterates available continuations and for each found (token, bigram_poss) calls `bigram_gen_next_step`.
  - `search_unigram2` iterates phrase index token ranges and calls `unigram_gen_next_step`.
- Each gen_next_step computes `pinyin_poss` via `compute_pronunciation_possibility(matrix, start, end, keys, phrase_item)`. If `pinyin_poss` nearly zero, candidate is skipped.
- Calculation formulas:
  - Unigram:
    - elem_poss = phrase_item.get_unigram_frequency() / phrase_index.get_phrase_index_total_freq()
    - next.m_poss = cur.m_poss + log(elem_poss * pinyin_poss * unigram_lambda)
  - Bigram:
    - unigram_poss = phrase_item.get_unigram_frequency() / phrase_index.get_phrase_index_total_freq()
    - combine = bigram_lambda * bigram_poss + unigram_lambda * unigram_poss
    - next.m_poss = cur.m_poss + log(combine * pinyin_poss)
- After building `next_step` candidate (`lookup_value_t next_step`), `save_next_step` is called to insert into the step's content or merge with an existing entry for the same next_key.

save_next_step tie-break & merge semantics
-----------------------------------------
When inserting a new candidate into a step:
- If the next step hash has no entry for next_key, append the candidate and insert mapping token -> (content index).
- If a prior candidate exists for this next_key (value -> step_index), then retrieve the existing `orig_next_value` and compare:
  - If `orig_next_value->m_length > next_step->m_length` => keep existing (existing longer is preferred).
  - Else if `orig_next_value->m_length == next_step->m_length`:
    - If `orig_next_value->m_poss < next_step->m_poss` => replace (prefer higher probability).
  - Otherwise (if new candidate has longer length), replace.
- In simpler terms: prefer candidates with greater total phrase length (`m_length`) first; if lengths tie, prefer higher `m_poss`.

final_step selection & backtrace
--------------------------------
- `final_step` inspects the content array at the last position (`last_step_pos`) and chooses a starting `max_value` as:
  - "the element with minimal `m_length`?": reading the code it selects `max_value` initially to the first element and then:

    ```
    if (cur_value->m_length < max_value->m_length ||
       (cur_value->m_length == max_value->m_length && cur_value->m_poss > max_value->m_poss))
         max_value = cur_value
    ```

  - That is: prefer **smaller** `m_length`? This seems counterintuitive but note the logic: during generation `m_length` was defined as `cur_step->m_length + phrase_length`. In `save_next_step` we preferred larger `m_length` to keep more coverage earlier; `final_step` has reversed comparison likely to prioritize shortest total length when backtracing or may be used in conjunction with other semantics. This is an important subtlety — see parity note below.
- After picking `max_value`, `final_step` backtraces via `m_last_step` and reconstructs the `MatchResult` by looking up prior tokens from `steps_index` and `steps_content`.
- Because `save_next_step` already enforces per-key uniqueness with length-first preference, the final scan is a selection among "winners" and uses its own length/poss ordering.

Training heuristics: `train_result2`
-----------------------------------
- Training updates user bigram/unigram data (interactive learning) using heuristic seeding:
  - Constants used in function:
    - initial_seed = 23 * 3
    - expand_factor = 2
    - unigram_factor = 7
    - pinyin_factor = 1
    - ceiling_seed = 23 * 15 * 64
  - For each token in `result` that should be trained:
    - For bigram train (if `last_token` exists), load user's SingleGram (or create new), compute `seed`:
      - if token absent: `seed = initial_seed`
      - else: `seed = max(freq, initial_seed) * expand_factor`, then `seed = min(seed, ceiling_seed)`
    - Update user single-gram:
      - `user.total_freq += seed`
      - `user.set_freq(token, freq + seed)` (freq is prior bigram freq)
      - store user bigram back to persistent backend (`m_user_bigram->store(last_token, user)`)
    - Compute next_pos for pronunciation update and call:
      - `increase_pronunciation_possibility(matrix, i, next_pos, keys, phrase_item, seed * pinyin_factor)`
    - Update phrase index unigram frequency:
      - `m_phrase_index->add_unigram_frequency(token, seed * unigram_factor)`
- The training approach increases both unigram and bigram counts with a policy that scales with pre-existing frequency and caps growth. This deterministic policy leads to deterministic changes for interactive learning tests.

Beam & pruning implications
---------------------------
- A local per-step beam of top `nbeam` candidates is used (`get_top_results`), but `save_next_step` may insert multiple candidates per next key and the next step arrays are only pruned by per-key uniqueness and subsequent expansions. This effectively bounds the candidate explosion, but the global search is not a strict n-best beam across the whole lattice.
- The combination of:
  - per-step heap extraction (top-nbeam),
  - `save_next_step` per-key dedup/merge,
  - and `final_step` selection
  produce the deterministic candidate set.

Numeric & log-space specifics
-----------------------------
- All probability combinations are multiplied and then passed into `log()` once (i.e., log(elem_poss * pinyin_poss * lambda) rather than summing logs); ensure parity by replicating order of operations.
- Checks against `FLT_EPSILON` / `DBL_EPSILON` are used to filter near-zero probabilities.
- Use natural log consistently.

Subtleties and important parity notes
------------------------------------
1. The `save_next_step` tie-breaker and the `final_step` selection use `m_length` differently; upstream logic may reflect subtle semantics about how length is counted (token length vs. phrase length). When porting, ensure the definition of `m_length` used in Rust matches upstream exactly (it is set to `cur_step->m_length + phrase_length`).
2. The exact value of `pinyin_poss` computed by `compute_pronunciation_possibility` is crucial — it multiplies the ngram probability. Port that function (and its heuristics) in parity tests.
3. Merging system+user gram data uses integer arithmetic and exact merging order; replicate that to ensure identical `bigram_poss` values.
4. Beam size and heap comparator must match exactly; comparator uses `m_poss` and max-heap semantics. Implementation must use same tie-breaker for equal `m_poss` if determinism required.
5. Training heuristics use several integer constants; tests that assert post-training counts should depend on those constants.

Pseudocode (behavioral) — best-match generation
-----------------------------------------------
```
nstep = matrix.size()
init steps_index[0..nstep-1] as empty hash tables
init steps_content[0..nstep-1] as empty lists

populate_prefixes(steps_index, steps_content, prefixes)

for i in 0..nstep-2:
    // candidates are pointers to step content entries
    candidates = clone pointers from steps_content[i]
    topresults = heap_top_n(candidates, nbeam)        // uses m_poss comparator

    for each top_value in topresults:
        // attempt to expand by unigram & bigram
        search_bigram2(top_value, i, m, ranges)    // loads bigram arrays and calls bigram_gen_next_step
        search_unigram2(top_value, i, m, ranges)   // loops phrase index ranges and calls unigram_gen_next_step

// final selection
last_array = steps_content[nstep-1]
if last_array empty -> fail
choose max_value among last_array by:
    prefer smaller m_length OR (if equal) prefer larger m_poss  // note subtle comparison upstream
backtrace from max_value via m_last_step to produce sequence of tokens
```

Parity checklist & tests to port
-------------------------------
To validate parity for lookup behavior, port the following upstream tests and data:
1. `tests/lookup/test_pinyin_lookup.cpp` / `tests/lookup/*` — these tests exercise the full lookup pipeline. Port canonical small test models (phrase_index, singlegram/bigram entries, and a small pinyin matrix) and expected top-N results.
2. `tests/storage/test_ngram.cpp` — already audited earlier; ensures `bigram_poss` and `elem_poss` values are consistent.
3. Tests for `compute_pronunciation_possibility` and `increase_pronunciation_possibility` (pronunciation model) — verify pinyin_poss numeric outputs.
4. Deterministic test case: start from a tiny phrase index and a simple matrix where exact top candidate ordering is known; assert the top result and the sequence produced by `get_best_match` / `final_step`.

Files to inspect next in upstream for full parity
------------------------------------------------
- `pinyin_phrase3.*` (phrase index item layout, unigram frequency accessors)
- `compute_pronunciation_possibility` (pronunciation probability computation)
- `facade_chewing_table2.h` / `facade_phrase_index` implementation (phrase index ranges, token->phrase mapping)
- The SingleGram / Bigram implementations and their persistence (we audited `ngram.cpp` earlier).
- Upstream lookup tests under `tests/lookup/` to extract authoritative input→expected outputs.

Implementation guidance for Rust port
------------------------------------
- Recreate `lookup_value_t` as a small struct with fields: handles (u32,u32), length (usize), poss (f64 or f32, match upstream usage), last_step (isize).
- Use a Vec<Vec<LookupValue>> for steps_content and Vec<HashMap<u32, usize>> for steps_index, keeping a single canonical index per token per step to implement dedup/merge.
- Implement `get_top_results` using a binary heap that yields the top nbeam entries by `m_poss`.
- Match arithmetic in `bigram_gen_next_step` and `unigram_gen_next_step` exactly: compute elem_poss/bigram_poss as floats (freq / total_freq), compute combined probability then multiply by pinyin_poss, then take ln() and add to `cur.m_poss`.
- Reproduce `save_next_step` merge rules: keep entry with longer `m_length`, or if equal, with higher `m_poss`.
- Port training seed constants exactly and adopt the same integer handling and capping logic when implementing `train_result2` behavior for user-learning tests.

Conclusion
----------
`pinyin_lookup2.cpp` implements a deterministic, per-step beam-limited dynamic expansion with careful per-key merging and log-space scoring that mixes pronunciation and n-gram probabilities by multiplication-before-log. To reach parity:
- port the exact scoring formulas and ordering rules,
- port or reproduce `pinyin_poss` and `bigram/unigram` numeric sources,
- reproduce training seed constants and merge semantics when training parity is required,
- port upstream lookup tests and use them as the canonical assertion suite.

End of audit.