# Fuzzy Search Functions Reference

**Document Version:** 1.0
**Date:** December 2025
**Status:** Technical Reference - Algorithm and Implementation Details

**Related Documentation:**
- [Getting Started With Fulltext.md](Getting%20Started%20With%20Fulltext.md) - Step-by-step tutorial for beginners
- [Full Text Search Guide New.md](Full%20Text%20Search%20Guide%20New.md) - Comprehensive function reference

---

## Overview

GraphLite provides a suite of text search and fuzzy matching functions built on the Levenshtein distance algorithm. These functions enable approximate string matching, similarity scoring, and relevance-based search queries.

**Location:** `graphlite/src/functions/text_search_functions.rs`

**Functions Covered:**
- `FUZZY_MATCH` - Boolean threshold-based matching
- `FT_SIMILARITY_SCORE` - Normalized similarity scoring (best for strings of similar length)
- `FUZZY_SEARCH` - Search relevance ranking
- `CONTAINS_FUZZY` - Fuzzy substring detection
- `HYBRID_SEARCH` - Multi-strategy combined search
- `WEIGHTED_SEARCH` - Custom weighted search
- `KEYWORD_MATCH` - Multi-keyword OR matching
- `KEYWORD_MATCH_ALL` - Multi-keyword AND matching

---

## Table of Contents

1. [Core Algorithm: Levenshtein Distance](#core-algorithm-levenshtein-distance)
2. [FUZZY_MATCH Function](#fuzzy_match-function)
3. [FT_SIMILARITY_SCORE Function](#levenshtein_similarity-function)
4. [FUZZY_SEARCH Function](#fuzzy_search-function)
5. [CONTAINS_FUZZY Function](#contains_fuzzy-function)
6. [Advanced Search Functions](#advanced-search-functions)
7. [Performance Characteristics](#performance-characteristics)
8. [Usage Examples](#usage-examples)
9. [Implementation Details](#implementation-details)

---

## Core Algorithm: Levenshtein Distance

### What is Levenshtein Distance?

The Levenshtein distance (edit distance) measures the minimum number of single-character edits needed to transform one string into another. The three allowed edit operations are:

1. **Insertion** - Add a character
2. **Deletion** - Remove a character
3. **Substitution** - Replace a character

### Implementation

**Code Location:** `text_search_functions.rs:19-61`

```rust
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();

    // Edge cases
    if len1 == 0 { return len2; }
    if len2 == 0 { return len1; }

    // Convert to character vectors (Unicode-aware)
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    // Dynamic programming matrix
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    // Initialize first row and column
    for i in 0..=len1 { matrix[i][0] = i; }
    for j in 0..=len2 { matrix[0][j] = j; }

    // Fill matrix
    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1_chars[i-1] == s2_chars[j-1] { 0 } else { 1 };
            matrix[i][j] = min(
                matrix[i-1][j] + 1,      // deletion
                matrix[i][j-1] + 1,      // insertion
                matrix[i-1][j-1] + cost  // substitution
            );
        }
    }

    matrix[len1][len2]
}
```

### Example Calculation

**Input:** "kitten" → "sitting"

**Step-by-step transformations:**
1. kitten → sitten (substitute 'k' → 's')
2. sitten → sittin (substitute 'e' → 'i')
3. sittin → sitting (insert 'g')

**Distance:** 3 edits

**Matrix visualization:**
```
      ""  s  i  t  t  i  n  g
""     0  1  2  3  4  5  6  7
k      1  1  2  3  4  5  6  7
i      2  2  1  2  3  4  5  6
t      3  3  2  1  2  3  4  5
t      4  4  3  2  1  2  3  4
e      5  5  4  3  2  2  3  4
n      6  6  5  4  3  3  2  3
```

**Result:** Distance = 3

### Similarity Score Calculation

**Code Location:** `text_search_functions.rs:63-73`

```rust
fn similarity_score(s1: &str, s2: &str) -> f64 {
    let distance = levenshtein_distance(s1, s2) as f64;
    let max_len = s1.len().max(s2.len()) as f64;

    if max_len == 0.0 {
        return 1.0;  // Both empty strings are identical
    }

    1.0 - (distance / max_len)
}
```

**Formula:**
```
similarity = 1.0 - (edit_distance / max_string_length)

Where:
- similarity = 1.0 means identical strings
- similarity = 0.0 means completely different
```

**Example:**
```
similarity_score("kitten", "sitting")
= 1.0 - (3 / 7)
= 1.0 - 0.428
= 0.571
```

---

## FT_FUZZY_MATCH Function

### Overview

**Signature:** `FT_FUZZY_MATCH(str1, str2, max_distance)`

**Purpose:** Returns `true` if two strings are similar within a specified edit distance threshold.

**Returns:** Boolean (`true` or `false`)

**Code Location:** `text_search_functions.rs:79-141`

### How It Works

1. Converts both strings to lowercase (case-insensitive comparison)
2. Calculates Levenshtein distance between them
3. Returns `true` if `distance <= max_distance`

### Implementation

```rust
impl Function for FuzzyMatchFunction {
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let s1 = context.get_argument(0)?.as_string()?;
        let s2 = context.get_argument(1)?.as_string()?;
        let max_dist = context.get_argument(2)?.as_number()? as usize;

        let distance = levenshtein_distance(
            &s1.to_lowercase(),
            &s2.to_lowercase()
        );

        Ok(Value::Boolean(distance <= max_dist))
    }
}
```

### Usage Examples

**Example 1: Find similar product names**
```sql
MATCH (p:Product)
WHERE FT_FUZZY_MATCH(p.name, 'iPhone', 2)
RETURN p.name, p.price
```

**Results:**
```
name       | price
-----------|-------
iPhone     | 999   (distance: 0)
iPhane     | 899   (distance: 1 - one substitution)
iPhons     | 950   (distance: 1 - one substitution)
iPbone     | 850   (distance: 2 - two substitutions)
```

**Example 2: Fuzzy name matching in fraud detection**
```sql
MATCH (customer:Customer)
WHERE FT_FUZZY_MATCH(customer.name, 'John Smith', 3)
RETURN customer.name, customer.id

-- Matches:
-- "John Smith"  (exact)
-- "Jon Smith"   (1 char difference)
-- "John Smyth"  (1 char difference)
-- "Jhon Smit"   (2 char differences)
```

**Example 3: Address matching with tolerance**
```sql
MATCH (addr:Address)
WHERE FT_FUZZY_MATCH(addr.street, '123 Main Street', 5)
RETURN addr.street

-- Matches variations with up to 5 character differences
```

### Null Handling

```sql
FT_FUZZY_MATCH(null, 'test', 2)     -- Returns: false
FT_FUZZY_MATCH('test', null, 2)     -- Returns: false
FT_FUZZY_MATCH('test', 'test', null) -- Returns: false
```

### Performance Notes

- **Time Complexity:** O(m × n) where m, n are string lengths
- **Space Complexity:** O(m × n) for distance matrix
- **Optimization:** Case conversion happens once before distance calculation

---

## FT_SIMILARITY_SCORE Function

### Overview

**Signature:** `FT_SIMILARITY_SCORE(str1, str2)`

**Purpose:** Returns a normalized similarity score from 0.0 (completely different) to 1.0 (identical) based on Levenshtein edit distance.

**Returns:** Number (0.0 to 1.0)

**Code Location:** `text_search_functions.rs:147-201`

**IMPORTANT:** Works best for strings of **similar length**. Penalizes length differences heavily because it normalizes by the longer string's length.

### How It Works

1. Calculates Levenshtein distance between strings
2. Normalizes by dividing by the length of the **longer** string (not average or sum)
3. Inverts to get similarity: `1.0 - (distance / max_length)`

**Formula:**
```
similarity = 1.0 - (levenshtein_distance / max(length(str1), length(str2)))
```

**Length Sensitivity Example:**
```
FT_SIMILARITY_SCORE('cat', 'catastrophe') = 0.27
  - Edit distance: 8
  - Max length: 11 (catastrophe)
  - Score: 1.0 - (8/11) = 0.27 (low despite perfect prefix match)

FT_SIMILARITY_SCORE('cat', 'bat') = 0.67
  - Edit distance: 1
  - Max length: 3
  - Score: 1.0 - (1/3) = 0.67 (high, strings have similar length)
```

### Implementation

```rust
impl Function for SimilarityScoreFunction {
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let s1 = context.get_argument(0)?.as_string()?;
        let s2 = context.get_argument(1)?.as_string()?;

        let score = similarity_score(
            &s1.to_lowercase(),
            &s2.to_lowercase()
        );

        Ok(Value::Number(score))
    }
}
```

### Usage Examples

**Example 1: Rank products by similarity (similar-length product names)**
```sql
-- Best used when product names have similar lengths
MATCH (p:Product)
WHERE abs(length(p.name) - length('iPhone')) < 5  -- Filter to similar lengths
RETURN
    p.name,
    FT_SIMILARITY_SCORE(p.name, 'iPhone') AS similarity
ORDER BY similarity DESC
LIMIT 10
```

**Results:**
```
name           | similarity
---------------|------------
iPhone         | 1.000  (exact match)
iPhone X       | 0.875  (1 char diff, 8 total)
iPhones        | 0.857  (1 char diff, 7 total)
iPhone 13      | 0.778  (2 char diff, 9 total)
Samsung        | 0.143  (6 char diff, 7 total)
```

**Example 2: Find duplicate customer records (names of similar length)**
```sql
MATCH (c1:Customer), (c2:Customer)
WHERE c1.id < c2.id
  AND abs(length(c1.name) - length(c2.name)) < 5  -- Similar name lengths
  AND FT_SIMILARITY_SCORE(c1.name, c2.name) > 0.8
RETURN
    c1.name AS name1,
    c2.name AS name2,
    FT_SIMILARITY_SCORE(c1.name, c2.name) AS similarity

-- Finds potential duplicates with >80% similarity
```

**Example 3: Fuzzy grouping for similarly-formatted tags**
```sql
MATCH (tag:Tag)
WHERE abs(length(tag.name) - length('machine-learning')) < 8
WITH tag, FT_SIMILARITY_SCORE(tag.name, 'machine-learning') AS sim
WHERE sim > 0.7
RETURN tag.name, sim
ORDER BY sim DESC

-- Groups similar tags together (works best for tags with consistent formatting)
```

### Mathematical Properties

**Symmetry:**
```sql
FT_SIMILARITY_SCORE(a, b) = FT_SIMILARITY_SCORE(b, a)
```

**Identity:**
```sql
FT_SIMILARITY_SCORE(a, a) = 1.0
```

**Range:**
```sql
0.0 <= FT_SIMILARITY_SCORE(a, b) <= 1.0
```

### Null Handling

```sql
FT_SIMILARITY_SCORE(null, 'test')  -- Returns: null
FT_SIMILARITY_SCORE('test', null)  -- Returns: null
FT_SIMILARITY_SCORE(null, null)    -- Returns: null
```

### Interpretation Guide

| Score Range | Interpretation | Use Case |
|-------------|----------------|----------|
| 0.9 - 1.0 | Nearly identical | Duplicate detection |
| 0.7 - 0.9 | Very similar | Fuzzy matching |
| 0.5 - 0.7 | Moderately similar | Broad search |
| 0.3 - 0.5 | Somewhat similar | Exploratory |
| 0.0 - 0.3 | Different | Not useful |

---

## FT_FUZZY_SEARCH Function

### Overview

**Signature:** `FT_FUZZY_SEARCH(text, query)`

**Purpose:** Returns a similarity score for how well a query matches within a text. Optimized for search ranking and relevance scoring.

**Returns:** Number (0.0 to 1.0)

**Code Location:** `text_search_functions.rs:696-776`

### How It Works

1. **Exact substring match:** Returns 1.0 immediately if query is an exact substring
2. **Sliding window search:** Tries matching query at every position in the text
3. **Best match selection:** Returns the highest similarity score found

### Key Difference from FT_SIMILARITY_SCORE

| Function | Comparison | Use Case |
|----------|------------|----------|
| `FT_SIMILARITY_SCORE` | Whole string vs whole string | "How similar are these two complete strings (of similar length)?" |
| `FUZZY_SEARCH` | Query within text (substring) | "How well does this query appear within this text?" |

### Implementation

```rust
impl Function for FuzzySearchFunction {
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let text_str = context.get_argument(0)?.as_string()?;
        let query_str = context.get_argument(1)?.as_string()?;

        let text_lower = text_str.to_lowercase();
        let query_lower = query_str.to_lowercase();

        // Exact substring match gets highest score
        if text_lower.contains(&query_lower) {
            return Ok(Value::Number(1.0));
        }

        // Find best match across all substrings
        let query_len = query_lower.len();
        let mut best_score: f64 = 0.0;

        for i in 0..=(text_lower.len() - query_len) {
            let substring = &text_lower[i..i + query_len];
            let distance = levenshtein_distance(substring, &query_lower);
            let score = 1.0 - (distance / query_len as f64);
            best_score = best_score.max(score);
        }

        Ok(Value::Number(best_score))
    }
}
```

### Usage Examples

**Example 1: Document search with ranking**
```sql
MATCH (doc:Document)
LET relevance = FT_FUZZY_SEARCH(doc.content, 'machine learning')
WHERE relevance > 0.7
RETURN
    doc.title,
    doc.content,
    relevance
ORDER BY relevance DESC
LIMIT 20
```

**Example 2: Product description search**
```sql
MATCH (p:Product)
WITH
    p,
    FT_FUZZY_SEARCH(p.description, 'wireless headphones') AS score
WHERE score > 0.8
RETURN p.name, p.description, score
ORDER BY score DESC
```

**Example 3: Multi-field search with aggregation**
```sql
MATCH (article:Article)
WITH
    article,
    FT_FUZZY_SEARCH(article.title, 'graph database') AS title_score,
    FT_FUZZY_SEARCH(article.abstract, 'graph database') AS abstract_score
LET total_score = (title_score * 0.7) + (abstract_score * 0.3)
WHERE total_score > 0.6
RETURN article.title, total_score
ORDER BY total_score DESC
```

### Behavior Examples

**Example: Substring vs Whole String Comparison**

```sql
-- Text: "The quick brown fox jumps over the lazy dog"
-- Query: "brown fox"

FT_SIMILARITY_SCORE(text, query)
-- Compares entire strings
-- Result: ~0.20 (low - different lengths, many unmatched chars)

FT_FUZZY_SEARCH(text, query)
-- Finds "brown fox" inside text
-- Result: 1.0 (perfect substring match)
```

**Example: Typo Tolerance**

```sql
-- Text: "GraphLite is a fast embedded graph database"
-- Query: "embeded"  (typo: missing 'd')

FT_FUZZY_SEARCH(text, query)
-- Slides "embeded" across text
-- Finds "embedded" with distance 1
-- Result: ~0.875 (1 char diff in 8-char word)
```

### Performance Characteristics

- **Time Complexity:** O(t × q²) where t = text length, q = query length
- **Early Exit:** Returns immediately on exact substring match
- **Best Case:** O(t) if exact match found early
- **Worst Case:** O(t × q²) for complete scan with no exact match

### Null Handling

```sql
FT_FUZZY_SEARCH(null, 'query')  -- Returns: null
FT_FUZZY_SEARCH('text', null)   -- Returns: null
FT_FUZZY_SEARCH(null, null)     -- Returns: null
```

---

## FT_CONTAINS_FUZZY Function

### Overview

**Signature:** `FT_CONTAINS_FUZZY(text, query, max_distance)`

**Purpose:** Returns `true` if text contains query as a substring with fuzzy matching tolerance.

**Returns:** Boolean (`true` or `false`)

**Code Location:** `text_search_functions.rs:607-690`

### How It Works

1. **Exact match first:** Returns `true` if query is exact substring
2. **Sliding window:** Slides query-length window across text
3. **Distance check:** Returns `true` if any window matches within max_distance

### Implementation

```rust
impl Function for ContainsFuzzyFunction {
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let text_str = context.get_argument(0)?.as_string()?;
        let query_str = context.get_argument(1)?.as_string()?;
        let max_dist = context.get_argument(2)?.as_number()? as usize;

        let text_lower = text_str.to_lowercase();
        let query_lower = query_str.to_lowercase();

        // Check exact match first
        if text_lower.contains(&query_lower) {
            return Ok(Value::Boolean(true));
        }

        // Fuzzy matching on all substrings
        let query_len = query_lower.len();
        for i in 0..=(text_lower.len().saturating_sub(query_len)) {
            let substring = &text_lower[i..i + query_len];
            if levenshtein_distance(substring, &query_lower) <= max_dist {
                return Ok(Value::Boolean(true));
            }
        }

        Ok(Value::Boolean(false))
    }
}
```

### Usage Examples

**Example 1: Tolerant substring search**
```sql
MATCH (comment:Comment)
WHERE FT_CONTAINS_FUZZY(comment.text, 'awesome', 2)
RETURN comment.text

-- Matches:
-- "This is awesome!"     (exact)
-- "This is awsome!"      (1 typo)
-- "Really awsom work"    (2 typos)
```

**Example 2: Email domain validation with typos**
```sql
MATCH (user:User)
WHERE FT_CONTAINS_FUZZY(user.email, '@gmail.com', 1)
RETURN user.email

-- Matches common typos:
-- "@gmail.com"  (correct)
-- "@gmai.com"   (missing 'l')
-- "@gmial.com"  (transposition)
```

### Comparison with FUZZY_SEARCH

```sql
-- Text: "GraphLite database system"
-- Query: "databse" (typo)

FT_CONTAINS_FUZZY(text, query, 1)
-- Returns: true (found similar substring)

FT_FUZZY_SEARCH(text, query)
-- Returns: 0.875 (similarity score)
```

---

## Advanced Search Functions

### FT_HYBRID_SEARCH

**Signature:** `FT_HYBRID_SEARCH(text, query)` or `FT_HYBRID_SEARCH(text, query, exact_weight, fuzzy_weight, similarity_weight)`

**Purpose:** Combines multiple search strategies with configurable weights.

**Code Location:** `text_search_functions.rs:207-336`

**Default Weights:**
- Exact match: 0.4 (40%)
- Fuzzy match: 0.4 (40%)
- Similarity: 0.2 (20%)

**Algorithm:**
```rust
fn calculate_hybrid_score(
    text: &str,
    query: &str,
    exact_weight: f64,
    fuzzy_weight: f64,
    similarity_weight: f64,
) -> f64 {
    // Strategy 1: Exact substring match
    let exact_score = if text.contains(query) { 1.0 } else { 0.0 };

    // Strategy 2: Fuzzy substring match (sliding window)
    let fuzzy_score = /* best sliding window match */;

    // Strategy 3: Overall similarity
    let similarity = similarity_score(text, query);

    // Weighted combination
    (exact_score * exact_weight +
     fuzzy_score * fuzzy_weight +
     similarity * similarity_weight) / total_weight
}
```

**Example:**
```sql
MATCH (doc:Document)
RETURN
    doc.title,
    FT_HYBRID_SEARCH(doc.content, 'neural networks') AS score
ORDER BY score DESC

-- Automatically balances exact, fuzzy, and similarity matching
```

### FT_WEIGHTED_SEARCH

**Signature:** `FT_WEIGHTED_SEARCH(text, query, exact_weight, fuzzy_weight, similarity_weight)`

**Purpose:** Custom weighted search with explicit weight control.

**Code Location:** `text_search_functions.rs:510-601`

**Example:**
```sql
-- Prioritize exact matches heavily
MATCH (article:Article)
RETURN
    article.title,
    FT_WEIGHTED_SEARCH(
        article.content,
        'GraphLite',
        0.7,  -- 70% weight on exact match
        0.2,  -- 20% weight on fuzzy match
        0.1   -- 10% weight on similarity
    ) AS score
ORDER BY score DESC
```

### FT_KEYWORD_MATCH

**Signature:** `FT_KEYWORD_MATCH(text, 'keyword1', 'keyword2', ...)`

**Purpose:** Match text against multiple keywords using OR logic.

**Code Location:** `text_search_functions.rs:340-429`

**Example:**
```sql
MATCH (product:Product)
WHERE FT_KEYWORD_MATCH(
    product.description,
    'wireless',
    'bluetooth',
    'portable'
)
RETURN product.name

-- Returns products matching ANY of the keywords
```

### FT_KEYWORD_MATCH_ALL

**Signature:** `FT_KEYWORD_MATCH_ALL(text, 'keyword1', 'keyword2', ...)`

**Purpose:** Match text against multiple keywords using AND logic.

**Code Location:** `text_search_functions.rs:435-504`

**Example:**
```sql
MATCH (product:Product)
WHERE FT_KEYWORD_MATCH_ALL(
    product.description,
    'wireless',
    'noise-canceling',
    'headphones'
)
RETURN product.name

-- Returns products matching ALL keywords
```

---

## Performance Characteristics

### Time Complexity

| Function | Best Case | Average Case | Worst Case |
|----------|-----------|--------------|------------|
| `FUZZY_MATCH` | O(m×n) | O(m×n) | O(m×n) |
| `FT_SIMILARITY_SCORE` | O(m×n) | O(m×n) | O(m×n) |
| `FUZZY_SEARCH` | O(t) | O(t×q²) | O(t×q²) |
| `CONTAINS_FUZZY` | O(t) | O(t×q²) | O(t×q²) |
| `HYBRID_SEARCH` | O(t) | O(t×q²) | O(t×q²) |

Where:
- m, n = lengths of two strings being compared
- t = text length
- q = query length

### Space Complexity

All functions: **O(m × n)** for the Levenshtein distance matrix.

### Optimization Opportunities

**Current Optimizations:**
- Early exit on exact match in `FUZZY_SEARCH` and `CONTAINS_FUZZY`
- Case conversion happens once (not per comparison)
- Unicode-aware character handling

**Potential Improvements:**
1. **Early termination in distance calculation**
   - Stop if distance exceeds threshold
   - Reduces worst-case for large strings

2. **Diagonal band optimization**
   - Limit matrix computation to band around diagonal
   - Useful when max_distance is small

3. **More efficient algorithms**
   - Wagner-Fischer with space optimization
   - Myers' bit-parallel algorithm for small alphabets
   - Damerau-Levenshtein for transpositions

4. **Caching**
   - Cache frequently compared string pairs
   - Useful in iterative queries

### Performance Guidelines

**Good Performance:**
```sql
-- Short strings, reasonable threshold
WHERE FT_FUZZY_MATCH(name, 'John', 2)  -- Fast

-- Exact match detection
WHERE FT_FUZZY_SEARCH(title, 'GraphLite')  -- Fast (early exit)
```

**Moderate Performance:**
```sql
-- Medium strings of similar length
WHERE FT_SIMILARITY_SCORE(description, query_string) > 0.7

-- Moderate-length sliding window
WHERE FT_FUZZY_SEARCH(paragraph, 'search query')
```

**Slower Performance:**
```sql
-- Very long strings
WHERE FT_FUZZY_SEARCH(entire_book_content, 'query')  -- O(millions)

-- Many comparisons (all-pairs)
MATCH (a:Article), (b:Article)
WHERE FT_SIMILARITY_SCORE(a.content, b.content) > 0.8  -- O(n²)
```

---

## Usage Examples

### Use Case 1: E-Commerce Product Search

**Scenario:** Search products with typo tolerance

```sql
-- Basic fuzzy product search
MATCH (p:Product)
WHERE FT_FUZZY_SEARCH(p.name, 'wireles headfones') > 0.7
RETURN p.name, p.price
ORDER BY FT_FUZZY_SEARCH(p.name, 'wireles headfones') DESC
LIMIT 10

-- Results:
-- "Wireless Headphones"  score: ~0.93
-- "Wireless Earphones"   score: ~0.85
-- "Bluetooth Headphones" score: ~0.72
```

### Use Case 2: Fraud Detection

**Scenario:** Find duplicate customer accounts with similar names (of similar length)

```sql
MATCH (c1:Customer), (c2:Customer)
WHERE c1.id < c2.id
  AND abs(length(c1.name) - length(c2.name)) < 5  -- Similar name lengths
  AND abs(length(c1.email) - length(c2.email)) < 10  -- Similar email lengths
  AND FT_SIMILARITY_SCORE(c1.name, c2.name) > 0.85
  AND FT_SIMILARITY_SCORE(c1.email, c2.email) > 0.70
RETURN
    c1.name AS name1,
    c2.name AS name2,
    FT_SIMILARITY_SCORE(c1.name, c2.name) AS name_similarity,
    c1.email AS email1,
    c2.email AS email2,
    FT_SIMILARITY_SCORE(c1.email, c2.email) AS email_similarity
ORDER BY name_similarity DESC

-- Finds potential duplicate accounts with similar-length names and emails
```

### Use Case 3: Document Search

**Scenario:** Multi-strategy document relevance ranking

```sql
MATCH (doc:Document)
WITH
    doc,
    FT_FUZZY_SEARCH(doc.title, 'graph databases') AS title_score,
    FT_FUZZY_SEARCH(doc.abstract, 'graph databases') AS abstract_score,
    FT_KEYWORD_MATCH(doc.keywords, 'graph', 'database', 'NoSQL') AS has_keywords
WHERE title_score > 0.6 OR abstract_score > 0.5 OR has_keywords = true
RETURN
    doc.title,
    (title_score * 0.5 + abstract_score * 0.3 +
     (CASE WHEN has_keywords THEN 0.2 ELSE 0.0 END)) AS total_relevance
ORDER BY total_relevance DESC
LIMIT 20
```

### Use Case 4: Social Network - Name Matching

**Scenario:** Find people with similar names for friend suggestions

```sql
MATCH (user:User {id: 'current_user_id'})
MATCH (other:User)
WHERE user.id <> other.id
  AND FT_FUZZY_MATCH(user.name, other.name, 3)
  AND abs(length(user.name) - length(other.name)) < 8  -- Similar name lengths
  AND NOT (user)-[:FRIENDS_WITH]-(other)
RETURN
    other.name,
    other.id,
    FT_SIMILARITY_SCORE(user.name, other.name) AS similarity
ORDER BY similarity DESC
LIMIT 10

-- Friend suggestions based on name similarity
```

### Use Case 5: Data Cleaning

**Scenario:** Standardize city names with variations

```sql
MATCH (address:Address)
WITH DISTINCT address.city AS city_variant
MATCH (standard:StandardCity)
WHERE FT_FUZZY_MATCH(city_variant, standard.name, 2)
  AND abs(length(city_variant) - length(standard.name)) < 8  -- Similar lengths
RETURN
    city_variant AS original,
    standard.name AS standardized,
    FT_SIMILARITY_SCORE(city_variant, standard.name) AS confidence
ORDER BY city_variant, confidence DESC

-- Maps variations to standard names (works best for similar-length variations):
-- "New York" → "New York"
-- "New Yrok" → "New York" (typo, similar length)
-- Note: "NYC" → "New York" may score lower due to length difference
```

---

## Implementation Details

### Case Sensitivity

**All functions are case-insensitive:**

```rust
let text_lower = text.to_lowercase();
let query_lower = query.to_lowercase();
```

**Example:**
```sql
FT_FUZZY_MATCH('iPhone', 'IPHONE', 0)  -- Returns: true
FT_SIMILARITY_SCORE('GraphLite', 'graphlite')  -- Returns: 1.0
```

### Unicode Support

**Character-level operations use Rust's `char` type:**

```rust
let s1_chars: Vec<char> = s1.chars().collect();
let s2_chars: Vec<char> = s2.chars().collect();
```

**Works correctly with:**
- Accented characters: "café" vs "cafe"
- Multi-byte UTF-8: "日本語" vs "日本后"
- Special characters and symbols

**Example:**
```sql
FT_SIMILARITY_SCORE('café', 'cafe')  -- Correctly handles é
FT_FUZZY_SEARCH('Hello World!', 'World')  -- Handles special characters
```

### Null Handling Summary

| Function | Null Behavior |
|----------|---------------|
| `FUZZY_MATCH` | Returns `false` if any input is null |
| `FT_SIMILARITY_SCORE` | Returns `null` if any input is null |
| `FUZZY_SEARCH` | Returns `null` if any input is null |
| `CONTAINS_FUZZY` | Returns `false` if any input is null |
| `HYBRID_SEARCH` | Returns `null` if text or query is null |
| `WEIGHTED_SEARCH` | Returns `null` if text or query is null |
| `KEYWORD_MATCH` | Returns `false` if text is null, ignores null keywords |
| `KEYWORD_MATCH_ALL` | Returns `false` if text is null, ignores null keywords |

### Function Registration

**All functions are registered in the function registry:**

`graphlite/src/functions/mod.rs:261-276`

```rust
// Register fuzzy matching functions
registry.register(
    "FT_FUZZY_MATCH",
    Box::new(text_search_functions::FuzzyMatchFunction::new()),
);
registry.register(
    "FT_SIMILARITY_SCORE",
    Box::new(text_search_functions::SimilarityScoreFunction::new()),
);
registry.register(
    "FT_CONTAINS_FUZZY",
    Box::new(text_search_functions::ContainsFuzzyFunction::new()),
);
registry.register(
    "FT_FUZZY_SEARCH",
    Box::new(text_search_functions::FuzzySearchFunction::new()),
);
// ... additional functions
```

**Note:** All fuzzy search functions use the `FT_` prefix to distinguish them from ISO GQL standard string functions.

### Error Handling

**Type validation:**
```rust
let str_val = context.get_argument(0)?
    .as_string()
    .ok_or_else(|| FunctionError::InvalidArgumentType {
        message: "Argument must be a string".to_string(),
    })?;
```

**Argument count validation:**
```rust
if context.argument_count() < expected {
    return Err(FunctionError::InvalidArgumentCount {
        expected,
        actual: context.argument_count(),
    });
}
```

---

## Comparison with Other Systems

### vs PostgreSQL

**PostgreSQL:**
- `pg_trgm` extension for trigram similarity
- `similarity(text, text)` function
- `levenshtein(text, text)` available in `fuzzystrmatch`

**GraphLite:**
- Built-in, no extensions needed
- Consistent API across all fuzzy functions
- Optimized for graph queries

### vs Elasticsearch

**Elasticsearch:**
- Fuzzy query with `fuzziness` parameter
- Match query with `operator` and `minimum_should_match`
- More query DSL, more complex

**GraphLite:**
- Simpler function-based approach
- Integrates directly with graph queries
- Lower setup complexity

### vs Neo4j

**Neo4j:**
- No built-in fuzzy string matching
- Requires APOC plugin for `apoc.text.levenshteinDistance`
- Plugin dependency required

**GraphLite:**
- Built-in fuzzy functions
- No plugins needed
- Part of core functionality

---

## Best Practices

### 1. Choose the Right Function

**Use `FUZZY_MATCH` when:**
- You need a binary yes/no answer
- You have a specific distance threshold
- Filtering records

**Use `FT_SIMILARITY_SCORE` when:**
- You need to rank results
- Comparing complete strings **of similar length**
- Finding near-duplicates with similar-length strings
- WARNING: Pre-filter by length difference for best results

**Use `FUZZY_SEARCH` when:**
- Searching within text (substring)
- Document/content search
- Query appears in longer text
- Working with strings of varying lengths

### 2. Set Appropriate Thresholds

**FUZZY_MATCH distance:**
- 1-2: Very strict (typos only)
- 3-4: Moderate tolerance
- 5+: Permissive (may have false positives)

**FT_SIMILARITY_SCORE threshold:**
- 0.9-1.0: Near duplicates (strings of similar length)
- 0.7-0.9: Similar items (strings of similar length)
- 0.5-0.7: Loosely related (strings of similar length)
- <0.5: Usually not useful
- WARNING: Combine with length filter: `abs(length(s1) - length(s2)) < threshold`

**FUZZY_SEARCH threshold:**
- 0.85-1.0: High confidence matches
- 0.7-0.85: Good matches
- 0.5-0.7: Possible matches
- <0.5: Low relevance

### 3. Performance Optimization

**Use indexes where possible:**
```sql
-- Create index on frequently searched fields
CREATE INDEX ON :Product(name)

-- Then fuzzy search is faster on indexed values
MATCH (p:Product)
WHERE FT_FUZZY_SEARCH(p.name, 'query') > 0.8
RETURN p
```

**Limit comparison scope:**
```sql
-- BAD: Compares all pairs (O(n²))
MATCH (a:Article), (b:Article)
WHERE FT_SIMILARITY_SCORE(a.title, b.title) > 0.8

-- GOOD: Pre-filter by category and length
MATCH (a:Article {category: 'tech'}), (b:Article {category: 'tech'})
WHERE a.id < b.id
  AND abs(length(a.title) - length(b.title)) < 10  -- Similar lengths
  AND FT_SIMILARITY_SCORE(a.title, b.title) > 0.8
```

**Use exact matches for early filtering:**
```sql
-- First filter with exact keywords, then fuzzy match
MATCH (doc:Document)
WHERE doc.content CONTAINS 'GraphLite'
  AND FT_FUZZY_SEARCH(doc.content, 'graph database') > 0.7
```

### 4. Combine Multiple Strategies

**Multi-field scoring:**
```sql
MATCH (product:Product)
WITH
    product,
    FT_FUZZY_SEARCH(product.name, 'laptop') * 0.6 +
    FT_FUZZY_SEARCH(product.description, 'laptop') * 0.3 +
    (CASE WHEN product.category = 'electronics' THEN 0.1 ELSE 0.0 END)
    AS total_score
WHERE total_score > 0.5
RETURN product.name, total_score
ORDER BY total_score DESC
```

---

## Troubleshooting

### Issue: Slow Performance

**Problem:** Queries using fuzzy functions are slow

**Solutions:**
1. Add indexes on searched fields
2. Pre-filter with exact matches or category filters
3. Use `FUZZY_MATCH` with low threshold instead of `FUZZY_SEARCH`
4. Limit the number of comparisons (avoid Cartesian products)
5. For `FT_SIMILARITY_SCORE`, pre-filter by length difference

### Issue: Too Many/Few Results

**Problem:** Threshold not filtering correctly

**Solutions:**
1. Adjust threshold based on actual data
2. Test with sample queries to calibrate
3. Use FT_SIMILARITY_SCORE to understand score distribution
4. **For FT_SIMILARITY_SCORE:** Always pre-filter by length difference
5. Consider string length when setting thresholds

### Issue: Case Sensitivity Problems

**Problem:** "iPhone" not matching "iphone"

**Solution:** Functions are already case-insensitive by default. If issues persist, check for leading/trailing whitespace:

```sql
-- Trim whitespace first
WHERE FT_FUZZY_MATCH(TRIM(field), TRIM('query'), 2)
```

### Issue: Unicode/Emoji Handling

**Problem:** Unexpected behavior with non-ASCII characters

**Solution:** Functions handle Unicode correctly. If issues occur, verify data encoding:

```sql
-- Check string lengths (char count, not bytes)
RETURN LENGTH(text_field)  -- Character count
```

---

## Future Enhancements

### Potential Additions

1. **Phonetic Matching**
   - Soundex algorithm
   - Metaphone/Double Metaphone
   - Useful for name matching

2. **N-gram Similarity**
   - Trigram/bigram matching
   - Better for very short strings
   - Complements Levenshtein

3. **Weighted Edit Operations**
   - Different costs for insertion/deletion/substitution
   - Keyboard distance-based costs
   - Context-aware weights

4. **Caching Layer**
   - Cache frequent comparisons
   - Significant speedup for repeated queries
   - Memory vs performance trade-off

5. **Approximate String Indexing**
   - BK-tree or similar structure
   - Fast approximate lookups
   - Reduce O(n) scans

---

## References

### Academic Papers

- Levenshtein, Vladimir I. (1966). "Binary codes capable of correcting deletions, insertions, and reversals"
- Wagner, Robert A.; Fischer, Michael J. (1974). "The String-to-String Correction Problem"

### Implementation Resources

- GraphLite source: `graphlite/src/functions/text_search_functions.rs`
- Function registry: `graphlite/src/functions/mod.rs`
- AST validator: `graphlite/src/ast/validator.rs`

### Related Documentation

- GraphLite Function Framework
- GQL Query Language Reference
- Performance Tuning Guide

---

## Changelog

**Version 1.0 (December 2025)**
- Initial documentation
- Comprehensive coverage of all fuzzy functions
- Usage examples and best practices
- Performance characteristics analysis

---

**Document Status:** Production Ready
**Last Updated:** December 2025
**Maintainer:** GraphLite Team
