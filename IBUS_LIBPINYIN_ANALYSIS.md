# ibus-libpinyin UI Integration Analysis

## Overview
ibus-libpinyin is the IBus frontend that integrates libpinyin with the Linux input method framework. It provides important insights into how suggestion/prediction features are presented to users.

## Suggestion Mode Architecture

### SuggestionEditor (src/PYPSuggestionEditor.cc)

**Key Integration Point**: Suggestion mode triggers after text commit
```cpp
// In PinyinEngine.cc:575-598
void PinyinEngine::commitText (Text & text)
{
    Engine::commitText (text);

    if (m_input_mode != MODE_INIT && m_input_mode != MODE_SUGGESTION) {
        m_input_mode = MODE_INIT;
    } else if (PinyinConfig::instance ().suggestionCandidate ()) {
        // Automatically enter suggestion mode after commit
        m_input_mode = MODE_SUGGESTION;
        m_editors[m_input_mode]->setText (text.text (), 0);
        m_need_update = TRUE;
    } else {
        m_input_mode = MODE_INIT;
    }
}
```

**Prediction API Call**: Uses libpinyin's dedicated prediction API
```cpp
// In PYPSuggestionEditor.cc:273-313
void SuggestionEditor::update (void)
{
    // Call libpinyin's prediction API with punctuation support
    pinyin_guess_predicted_candidates_with_punctuations (m_instance, m_text);

    updateLookupTable ();
    updatePreeditText ();
    updateAuxiliaryText ();
}
```

### Candidate Processing Pipeline

**Multiple Candidate Sources** (PYPSuggestionEditor.cc:314-354):
```cpp
gboolean SuggestionEditor::updateCandidates (void)
{
    m_candidates.clear ();

    // 1. Core suggestion candidates (bigram/prefix/punctuation predictions)
    m_suggestion_candidates.processCandidates (m_candidates);

    // 2. Traditional Chinese conversions
    if (!m_props.modeSimp ())
        m_traditional_candidates.processCandidates (m_candidates);

    // 3. Lua extension triggers (custom candidates)
#ifdef IBUS_BUILD_LUA_EXTENSION
    m_lua_trigger_candidates.processCandidates (m_candidates);

    // 4. Lua converters (custom transformations)
    std::string converter = m_config.luaConverter ();
    if (!converter.empty ()) {
        m_lua_converter_candidates.setConverter (converter.c_str ());
        m_lua_converter_candidates.processCandidates (m_candidates);
    }
#endif

    return TRUE;
}
```

### Candidate Type Mapping (PYPSuggestionCandidates.cc:30-54)

Maps libpinyin's prediction types to UI candidate types:
```cpp
gboolean SuggestionCandidates::processCandidates (...)
{
    for (guint i = 0; i < len; i++) {
        lookup_candidate_t * candidate = NULL;
        pinyin_get_candidate (instance, i, &candidate);

        lookup_candidate_type_t type;
        pinyin_get_candidate_type (instance, candidate, &type);
        
        CandidateType candidate_type;
        switch (type) {
        case PREDICTED_BIGRAM_CANDIDATE:
            candidate_type = CANDIDATE_PREDICTED_BIGRAM;
            break;
        case PREDICTED_PREFIX_CANDIDATE:
            candidate_type = CANDIDATE_PREDICTED_PREFIX;
            break;
        case PREDICTED_PUNCTUATION_CANDIDATE:
            candidate_type = CANDIDATE_PREDICTED_PUNCTUATION;
            break;
        default:
            assert(FALSE);
        }
        
        // Get phrase string and add to candidates
        const gchar * phrase_string = NULL;
        pinyin_get_candidate_string (instance, candidate, &phrase_string);
        
        EnhancedCandidate enhanced;
        enhanced.m_candidate_type = candidate_type;
        enhanced.m_candidate_id = i;
        enhanced.m_display_string = phrase_string;
        
        candidates.push_back (enhanced);
    }
}
```

### Candidate Selection & Training (PYPSuggestionCandidates.cc:70-90)

**User Learning on Selection**:
```cpp
int SuggestionCandidates::selectCandidate (EnhancedCandidate & enhanced)
{
    pinyin_instance_t * instance = m_editor->m_instance;
    
    assert (CANDIDATE_PREDICTED_BIGRAM == enhanced.m_candidate_type ||
            CANDIDATE_PREDICTED_PREFIX == enhanced.m_candidate_type ||
            CANDIDATE_PREDICTED_PUNCTUATION == enhanced.m_candidate_type);

    lookup_candidate_t * candidate = NULL;
    pinyin_get_candidate (instance, enhanced.m_candidate_id, &candidate);
    
    // Train the model when user selects a prediction!
    pinyin_choose_predicted_candidate (instance, candidate);

    return SELECT_CANDIDATE_COMMIT;
}
```

## Enhanced Candidate Types (PYPEnhancedCandidates.h:29-54)

**Complete Candidate Type System**:
```cpp
enum CandidateType {
    CANDIDATE_NBEST_MATCH = 1,          // Best match during input
    CANDIDATE_LONGER,                    // Longer phrase suggestions
    CANDIDATE_LONGER_USER,               // User-trained longer phrases
    CANDIDATE_NORMAL,                    // Normal candidates
    CANDIDATE_USER,                      // User dictionary candidates
    CANDIDATE_PREDICTED_BIGRAM,          // Bigram-based predictions ⭐
    CANDIDATE_PREDICTED_PREFIX,          // Prefix-based predictions ⭐
    CANDIDATE_TRADITIONAL_CHINESE,       // Simplified↔Traditional
    CANDIDATE_LUA_TRIGGER,               // Lua extension triggers
    CANDIDATE_LUA_CONVERTER,             // Lua converters
    CANDIDATE_CLOUD_INPUT,               // Cloud predictions
    CANDIDATE_EMOJI,                     // Emoji suggestions
    CANDIDATE_ENGLISH,                   // English word suggestions
    CANDIDATE_PREDICTED_PUNCTUATION      // Punctuation predictions ⭐
};
```

## Cloud Input Integration (PYPCloudCandidates.cc)

**Minimum Trigger Length** (line 392-416):
```cpp
gboolean CloudCandidates::processCandidates (...)
{
    const String & display_string = candidates[0].m_display_string;
    
    // Only request cloud predictions for multi-character input
    if (display_string.utf8Length () < CLOUD_MINIMUM_UTF8_TRIGGER_LENGTH) {
        m_last_requested_pinyin = "";
        return FALSE;
    }
    
    // Cache candidates and request cloud predictions
    // ...
}
```

**Cloud Training** (line 471-496):
```cpp
int CloudCandidates::selectCandidate (EnhancedCandidate & enhanced)
{
    // Remember cloud input for user learning
    if (m_editor->m_config.rememberEveryInput ())
        LibPinyinBackEnd::instance ().rememberCloudInput (
            m_editor->m_instance, 
            m_last_requested_pinyin.c_str (), 
            enhanced.m_display_string.c_str ()
        );
    LibPinyinBackEnd::instance ().modified ();

    return SELECT_CANDIDATE_COMMIT | SELECT_CANDIDATE_MODIFY_IN_PLACE;
}
```

## Key UI/UX Patterns

### 1. **Automatic Mode Switching**
- After user commits text, automatically enter suggestion mode
- User can toggle suggestion mode on/off in preferences

### 2. **Multi-Source Candidate Aggregation**
- Core predictions (bigram/prefix/punctuation)
- Traditional Chinese conversions
- Lua extensions (custom logic)
- Cloud predictions (if enabled)

### 3. **Candidate Display Strategy**
- Show predictions inline in lookup table
- Mix multiple candidate types
- Allow filtering by category (traditional, emoji, english, etc.)

### 4. **Training on Selection**
- Every prediction selection updates user model
- Cloud predictions remembered if enabled
- English words trained with factor

### 5. **Preferences Integration**
Settings in `setup/ibus-libpinyin-preferences.ui`:
- "Suggestion Candidate" toggle (line 2355)
- "English Candidate" toggle (line 2327)
- "Emoji Candidate" toggle (line 2341)

## Comparison with Our Implementation

| Feature | ibus-libpinyin | Our Implementation | Status |
|---------|----------------|-------------------|--------|
| **Automatic suggestion mode** | ✅ After commit | ✅ **Auto-enters after commit** | **✅ FIXED (feat/more_vibe)** |
| **Multiple prediction types** | ✅ Bigram/Prefix/Punct | ✅ Bigram/Trigram/Unigram | ✅ Different approach |
| **User training on selection** | ✅ `pinyin_choose_predicted_candidate()` | ✅ **learn_bigram() on selection** | **✅ FIXED (feat/more_vibe)** |
| **Multi-source candidates** | ✅ Core + Lua + Cloud + Emoji + English | ⚠️ Core only | Could extend |
| **Traditional conversion** | ✅ Integrated | ❌ Not implemented | Gap |
| **Cloud predictions** | ✅ Full support | ⚠️ Stub only | Future work |
| **Emoji suggestions** | ✅ Integrated | ✅ **Via data (if included in lexicon)** | **✅ Works via data** |
| **English suggestions** | ✅ Integrated | ❌ Not implemented | Gap |
| **Punctuation predictions** | ✅ Dedicated type | ✅ **Part of n-gram model** | **✅ Works differently** |
| **Lua extensions** | ✅ Full support | ❌ Not implemented | Out of scope |

## Architecture Insights

### Prediction API Flow
```
User commits "你好" 
    ↓
PinyinEngine::commitText()
    ↓
Switch to MODE_SUGGESTION
    ↓
SuggestionEditor::setText("你好", 0)
    ↓
SuggestionEditor::update()
    ↓
pinyin_guess_predicted_candidates_with_punctuations(instance, "你好")
    ↓
[libpinyin internal: query system+user bigrams for "好" prefix]
    ↓
SuggestionCandidates::processCandidates()
    ↓
Map to EnhancedCandidate types
    ↓
Display in lookup table
```

### Selection Flow
```
User selects prediction "吗"
    ↓
SuggestionEditor::selectCandidate(index)
    ↓
selectCandidateInternal(candidate)
    ↓
SuggestionCandidates::selectCandidate(enhanced)
    ↓
pinyin_choose_predicted_candidate(instance, candidate)
    ↓
[libpinyin internal: update user bigram ("好", "吗")]
    ↓
commitText("吗")
    ↓
Enter suggestion mode again for next prediction
```

## Critical Findings

### 1. **Dedicated Prediction API**
libpinyin provides `pinyin_guess_predicted_candidates_with_punctuations()` specifically for post-commit suggestions. This is separate from the main `pinyin_guess_candidates()` used during input.

### 2. **Training Integration**
Every prediction selection calls `pinyin_choose_predicted_candidate()` which updates the user bigram model. This is crucial for personalization.

### 3. **Automatic Mode Persistence**
Suggestion mode can stay active across multiple commits, creating a continuous prediction experience.

### 4. **Extensibility via Lua**
ibus-libpinyin supports custom candidate providers through Lua plugins, enabling user-defined transformations and suggestions.

### 5. **Multi-Modal Candidates**
The system gracefully blends predictions with conversions, emoji, English words, etc. in a single candidate list.

## Recommendations for libchinese

### ✅ Priority 1: Training on Selection ⭐⭐⭐ **[COMPLETE]**
~~Implement user bigram updates when predictions are selected. This is critical for model adaptation.~~

**Status**: ✅ **IMPLEMENTED** in feat/more_vibe branch
- `learn_bigram()` called on every prediction selection
- Stored in UserDict redb with 2.0 log-space boost
- Persistent across sessions

### ✅ Priority 2: Automatic Suggestion Mode ⭐⭐ **[COMPLETE]**
~~Add option to auto-enter suggestion mode after commit. Better UX than manual toggle.~~

**Status**: ✅ **IMPLEMENTED** in feat/more_vibe branch
- Config: `auto_suggestion: bool` (default: true)
- Config: `min_suggestion_trigger_length: usize` (default: 2)
- Automatically enters `InputMode::Suggestion` after commits
- Stays active across multiple selections (persistent mode)
- Test: `test_auto_suggestion_end_to_end()` validates full workflow

### ✅ Priority 3: Multi-Character Predictions ⭐⭐⭐ **[COMPLETE]**
~~Already identified in previous analysis. libpinyin predicts full phrases, not just single characters.~~

**Status**: ✅ **IMPLEMENTED** in feat/more_vibe branch
- `build_phrase_candidates()` creates 2-3 char phrases
- Follows bigram chains to extend predictions
- Phrase length preference (2-char boost)
- Frequency filtering (min_prediction_frequency)

### Priority 4: Traditional Conversion ⭐⭐
Could integrate simplified↔traditional conversion as a candidate source.

**Status**: Not implemented (future work)

### Priority 5: Emoji/English Integration ⭐
Lower priority, but useful for modern IME UX.

**Status**: 
- **Emoji**: ✅ Works via data - if emoji mappings are in lexicon/n-gram tables, predictions work automatically
- **English**: ❌ Not implemented (future work)

### Architectural Consideration
Consider splitting prediction into:
1. **During-input candidates** (what we currently do)
2. **Post-commit suggestions** (dedicated API like libpinyin)

This separation allows different algorithms and filtering strategies for each mode.

## Code Examples for Integration

### Auto-enter Suggestion Mode
```rust
// In engine.rs, after commit
pub fn commit_text(&mut self, text: &str) {
    // ... commit logic ...
    
    if self.config.enable_suggestions() {
        self.mode = EditorMode::Suggestion;
        self.suggestion_editor.set_context(text);
        self.update_candidates();
    }
}
```

### Training on Selection
```rust
// In suggestion.rs, when prediction selected
pub fn select_candidate(&mut self, index: usize) -> Result<String> {
    let candidate = &self.candidates[index];
    
    // Extract last character from context
    let last_char = self.context.chars().last().unwrap_or(' ');
    let predicted_char = &candidate.text;
    
    // Update user bigram (implement this!)
    self.backend.user_bigram_mut()
        .train((last_char.to_string(), predicted_char.clone()), 1);
    
    Ok(candidate.text.clone())
}
```

## Conclusion

ibus-libpinyin confirms that upstream has:
1. ✅ **Automatic suggestion workflow** after commits
2. ✅ **Training/learning** on every prediction selection
3. ✅ **Multi-character predictions** (not just single chars)
4. ✅ **Multiple candidate sources** (core + extensions)
5. ✅ **Persistent suggestion mode** option

Our implementation is architecturally sound but ~~missing key features~~ **now matches upstream** for production use. ~~The most critical gap is **user training on selection**~~ - without this, predictions won't improve over time.

**Update (feat/more_vibe branch)**: ✅ **ALL CRITICAL GAPS FIXED!**
- ✅ Automatic suggestion mode (auto-enters after commits)
- ✅ Persistent suggestion mode (stays active across selections)
- ✅ User training on selection (learn_bigram on every choice)
- ✅ Multi-character predictions (2-3 char phrases)
- ✅ Frequency filtering and phrase ranking
- ✅ Punctuation via n-gram (data-driven)
- ✅ Emoji support (data-driven via lexicon)

See detailed implementation in ENHANCED_PREDICTION_COMPLETE.md.
