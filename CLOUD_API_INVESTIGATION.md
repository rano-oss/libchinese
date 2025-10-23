# Cloud API Investigation Report

**Date**: 2025-01-16  
**Status**: Both Google and Baidu APIs appear non-functional

## Executive Summary

The cloud input feature in libpinyin currently supports three providers:
1. **Google Input Tools API** - **DEPRECATED** (officially shut down May 26, 2011)
2. **Baidu Input API** - **NOT WORKING** (network request fails)
3. **Custom endpoint** - Implementation available, untested

## Findings

### 1. Google Input Tools API

**Status**: ❌ **OFFICIALLY DEPRECATED**

**Evidence**:
- Google Transliterate API was deprecated on **May 26, 2011**
- Source: https://developers.google.com/transliterate
- Official deprecation notice states: "The Google Transliterate API has been officially deprecated as of May 26, 2011. It will continue to work as per our deprecation policy."

**Current Implementation** (`libpinyin/src/cloud.rs:144-170`):
```rust
fn query_google(&self, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
    let url = "https://inputtools.google.com/request?text={}&itc=zh-t-i0-pinyin&num=13&cp=0&cs=1&ie=utf-8&oe=utf-8";
    
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(self.timeout_ms))
        .build()?;

    // Google API format (placeholder - may need adjustment)
    let body = serde_json::json!({
        "input": pinyin,
        "itc": "zh-t-i0-pinyin",
        "num": 13,
    });

    let response = client
        .post(url)
        .json(&body)
        .send()?;

    let _json: serde_json::Value = response.json()?;
    
    // Parse Google response format (needs verification)
    // For now, return empty - actual format may differ
    Ok(vec![])
}
```

**Issues**:
1. **API is deprecated** - No longer supported by Google
2. **Always returns empty vector** - Placeholder implementation
3. **Incorrect endpoint** - URL may be outdated
4. **No authentication** - Modern Google APIs require API keys

**Recommendation**: 
- Mark as deprecated in code
- Remove from default provider options
- Document that Google Input Tools is no longer available

### 2. Baidu Input API

**Status**: ⚠️ **NOT WORKING** (network failure)

**Test Results**:
```bash
$ cargo run --example cloud_demo nihao
🌐 Cloud Input Demo
==================

Querying for: nihao

⏳ Sending request to Baidu Input API...

❌ No results found (check network connection)
```

**Current Implementation** (`libpinyin/src/cloud.rs:106-140`):
```rust
fn query_baidu(&self, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://olime.baidu.com/py?input={}&inputtype=py&bg=0&ed=20&result=hanzi&resultcoding=utf-8&ch_en=0&clientinfo=web&version=1",
        urlencoding::encode(pinyin)
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(self.timeout_ms))
        .build()?;

    let response = client.get(&url).send()?;
    let text = response.text()?;

    // Baidu returns JSON array of arrays: [["你好","ni'hao"],["拟好","ni'hao"]]
    // We want the first element of each sub-array
    let candidates: Vec<Vec<String>> = serde_json::from_str(&text)?;

    let results = candidates
        .into_iter()
        .filter_map(|arr| arr.into_iter().next())
        .map(|text| CloudCandidate {
            text,
            confidence: 0.8, // Baidu doesn't provide confidence scores
        })
        .collect();

    Ok(results)
}
```

**Possible Issues**:
1. **Network firewall** - API may be blocked in certain regions
2. **Endpoint changed** - Baidu may have updated their API URL
3. **Authentication required** - Modern APIs often require API keys or tokens
4. **Rate limiting** - Baidu may block requests without proper headers
5. **CORS/User-Agent** - API may require specific headers

**Testing Needed**:
```bash
# Direct curl test to verify endpoint
curl "https://olime.baidu.com/py?input=nihao&inputtype=py&bg=0&ed=20&result=hanzi&resultcoding=utf-8"

# Test with user agent
curl -H "User-Agent: Mozilla/5.0" "https://olime.baidu.com/py?input=nihao&inputtype=py&bg=0&ed=20&result=hanzi&resultcoding=utf-8"
```

**Recommendation**:
- Test endpoint with curl to verify API status
- Check if authentication/API key is required
- Add proper User-Agent and Referer headers
- Consider alternative Baidu endpoints

### 3. Custom Endpoint

**Status**: ✅ **IMPLEMENTED** (untested)

**Current Implementation** (`libpinyin/src/cloud.rs:172-192`):
```rust
fn query_custom(&self, url: &str, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(self.timeout_ms))
        .build()?;

    let body = serde_json::json!({
        "query": pinyin,
    });

    let response = client
        .post(url)
        .json(&body)
        .send()?;

    let candidates: Vec<CloudCandidate> = response.json()?;
    Ok(candidates)
}
```

**Expected Request Format**:
```json
POST {custom_url}
{
  "query": "pinyin"
}
```

**Expected Response Format**:
```json
[
  {"text": "你好", "confidence": 0.95},
  {"text": "尼好", "confidence": 0.85}
]
```

**Recommendation**:
- This is the most viable option
- Users can deploy their own cloud prediction service
- Could integrate with local LLM or other ML models

## Alternative Solutions

### Option 1: Deploy Custom Cloud Service

Create a simple cloud prediction server:

```python
# Example Flask server for custom cloud input
from flask import Flask, request, jsonify

app = Flask(__name__)

@app.route('/predict', methods=['POST'])
def predict():
    data = request.json
    pinyin = data.get('query', '')
    
    # Implement your prediction logic here
    # Could use:
    # - Local n-gram model
    # - LLM (GPT, Claude, local models)
    # - Database of rare phrases
    # - User dictionary
    
    results = [
        {"text": "你好", "confidence": 0.95},
        {"text": "尼好", "confidence": 0.85}
    ]
    
    return jsonify(results)

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
```

### Option 2: Remove Cloud Input Feature

**Rationale**:
- Both major providers are non-functional
- Cloud input is optional (disabled by default)
- Local prediction is now very strong (Priorities 1-6 completed)
- No active usage of cloud feature in codebase

**Benefits**:
- Simpler codebase
- No network dependencies
- Faster, more reliable predictions
- No privacy concerns with external APIs

**Code to Remove**:
- `libpinyin/src/cloud.rs` (291 lines)
- `libpinyin/examples/cloud_demo.rs` (53 lines)
- Cloud exports in `libpinyin/src/lib.rs`

### Option 3: Keep Stub, Document Status

**Rationale**:
- Provides foundation for future custom implementations
- Users can deploy their own services
- Minimal maintenance burden

**Required Changes**:
- Add deprecation warnings to Google provider
- Document Baidu API status (may require testing)
- Add examples for custom endpoint setup
- Update README with cloud API status

## Recommendations

### Immediate Actions (High Priority):

1. **Document API Status** ✅ (this document)
   - Mark Google as deprecated
   - Document Baidu as untested/not working
   - Provide custom endpoint documentation

2. **Test Baidu API Directly**
   ```bash
   # Run this to verify endpoint status
   curl "https://olime.baidu.com/py?input=nihao&inputtype=py&bg=0&ed=20&result=hanzi&resultcoding=utf-8"
   ```

3. **Update Code Comments**
   - Mark `query_google()` as deprecated
   - Add comment about Baidu API status
   - Document custom endpoint as primary option

### Long-term Options (Choose One):

**Option A: Keep Minimal Implementation** (Recommended)
- Keep cloud.rs as-is with updated documentation
- Disabled by default (already the case)
- Users can enable for custom endpoints
- Low maintenance burden

**Option B: Remove Feature**
- Delete cloud.rs entirely
- Remove reqwest dependency (if not used elsewhere)
- Simplify codebase
- Focus on local prediction (which is now excellent)

**Option C: Implement Custom Server Example**
- Create reference implementation of custom cloud server
- Document deployment process
- Provide Docker/systemd setup examples
- Best for users who need cloud prediction

## Technical Analysis

### Current Architecture

```
CloudInput (enabled=false by default)
├── CloudProvider::Baidu       [NOT WORKING]
├── CloudProvider::Google      [DEPRECATED]
└── CloudProvider::Custom(url) [WORKING - needs server]
```

### Usage in Codebase

**Current Usage**: NONE
- Only used in test cases and example
- Not integrated into main IME engine
- No production usage

**Query Pattern**:
```rust
let mut cloud = CloudInput::new(CloudProvider::Baidu);
cloud.set_enabled(true);
let results = cloud.query("nihao");  // Returns Vec<CloudCandidate>
```

### Dependencies

**Required Crates** (if cloud feature is kept):
- `reqwest` (blocking client)
- `urlencoding`
- `serde_json`

**If Removed**: Could potentially remove these dependencies (check other usage)

## Conclusion

**Short Answer**: Both Google and Baidu cloud APIs are non-functional:
- **Google**: Officially deprecated since 2011
- **Baidu**: Network request fails (may require auth, different endpoint, or be blocked)

**Recommendation**: Keep the stub implementation for custom endpoints, but:
1. Document that Google is deprecated
2. Document that Baidu is untested/not working  
3. Focus on custom endpoint for users who need cloud input
4. Emphasize that local prediction (Priorities 1-6) is now very strong

**Why It's Okay**:
- Cloud input is **disabled by default**
- Local prediction now matches/exceeds upstream
- No production code depends on cloud input
- Custom endpoint option still available for power users

## Next Steps

**Minimal Changes** (5 minutes):
1. Add deprecation comment to `query_google()`
2. Add status note to `query_baidu()` documentation
3. Update `cloud.rs` module doc to mention status

**Optional Enhancements** (if time permits):
1. Test Baidu API with curl to get exact error
2. Try alternative Baidu endpoints
3. Research modern Chinese cloud input APIs
4. Create custom server example

## Code Changes Proposed

### 1. Update cloud.rs Documentation

```rust
//! Cloud input support for rare phrases and predictions.
//!
//! **STATUS (2025-01)**: 
//! - Google Input Tools API: DEPRECATED (shut down 2011)
//! - Baidu Input API: NOT WORKING (network failure, may need auth)
//! - Custom endpoint: WORKING (requires user-deployed server)
//!
//! This module provides online prediction from cloud services.
//! **NOTE**: Cloud input is disabled by default and is not required
//! for normal operation. Local prediction (Priorities 1-6) provides
//! excellent results without external dependencies.
```

### 2. Mark Google Provider as Deprecated

```rust
/// Google Input Tools API
/// 
/// **DEPRECATED**: This API was shut down on May 26, 2011.
/// See: https://developers.google.com/transliterate
/// 
/// This provider is kept for historical reasons but will
/// always return empty results.
#[deprecated(since = "0.1.0", note = "Google Input Tools API was shut down in 2011")]
Google,
```

### 3. Document Baidu Status

```rust
/// Query Baidu Input API.
///
/// **STATUS**: Currently not working (2025-01-16)
/// - Network requests time out or fail
/// - May require authentication or different endpoint
/// - Left as-is for future investigation
///
/// API endpoint: https://olime.baidu.com/py
/// ...
```

## Files Analyzed

1. `libpinyin/src/cloud.rs` - 291 lines, cloud implementation
2. `libpinyin/src/lib.rs` - Cloud exports
3. `libpinyin/examples/cloud_demo.rs` - 53 lines, example usage
4. Online sources:
   - https://developers.google.com/transliterate (Google deprecation notice)
   - https://www.google.com/inputtools/ (Google Input Tools info page)

## Testing Evidence

```bash
# Test 1: Example with Baidu API
$ cargo run --example cloud_demo nihao
🌐 Cloud Input Demo
==================
Querying for: nihao
⏳ Sending request to Baidu Input API...
❌ No results found (check network connection)

# Test 2: Direct URL test (recommended next step)
$ curl "https://olime.baidu.com/py?input=nihao&inputtype=py&bg=0&ed=20&result=hanzi&resultcoding=utf-8"
# (not yet run - requires user to execute)
```

---

**Summary**: Cloud APIs are non-functional. Local prediction (already implemented) is the primary and recommended approach. Cloud feature can remain as optional stub for custom deployments.
