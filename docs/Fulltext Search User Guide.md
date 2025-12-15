# Full Text Search Guide (Fuzzy Search Functions)

GraphLite provides comprehensive fuzzy text search capabilities including fuzzy matching, similarity scoring, and hybrid search using **Levenshtein distance algorithms**. This guide covers all available fuzzy search functions.

---

## ⚠️ Important: Fuzzy Search vs Pattern Matching vs Tantivy

GraphLite has **three types** of text search capabilities:

| Type | Algorithm | Indexing | Documentation |
|------|-----------|----------|---------------|
| **Fuzzy Search** (this guide) | Levenshtein distance | No index | You are here |
| **Pattern Matching** (string) | SUBSTRING, REVERSE | No index | [PATTERN_MATCHING_GUIDE.md](../PATTERN_MATCHING_GUIDE.md) |
| **Full-Text Search** (Tantivy) | Inverted index | Tantivy index | [FULLTEXT_PATTERN_MATCHING_SPEC.md](../FULLTEXT_PATTERN_MATCHING_SPEC.md) |

**This guide covers:** Fuzzy search functions that use Levenshtein distance for typo-tolerant matching **without** requiring indexes.

**For pattern matching** (prefix, suffix, wildcard): See [PATTERN_MATCHING_GUIDE.md](../PATTERN_MATCHING_GUIDE.md)
**For Tantivy-indexed search**: See [FULLTEXT_PATTERN_MATCHING_SPEC.md](../FULLTEXT_PATTERN_MATCHING_SPEC.md)

---

**Related Documentation:**
- [Getting Started With Fulltext.md](Getting%20Started%20With%20Fulltext.md) - Step-by-step fuzzy search tutorial
- [Fuzzy Search Functions Reference.md](Fuzzy%20Search%20Functions%20Reference.md) - Technical algorithm details
- [PATTERN_MATCHING_GUIDE.md](../PATTERN_MATCHING_GUIDE.md) - Prefix/suffix/wildcard patterns
- [FULLTEXT_PATTERN_MATCHING_SPEC.md](../FULLTEXT_PATTERN_MATCHING_SPEC.md) - Tantivy-based functions

## Overview

GraphLite's **fuzzy search** functionality uses **Levenshtein distance** for:

- **Fuzzy Matching**: Find approximate matches with configurable edit distance
- **Similarity Scoring**: Calculate normalized similarity scores between strings
- **Substring Search**: Fuzzy and exact substring matching
- **Hybrid Search**: Combine multiple search strategies with configurable weights
- **Keyword Matching**: Boolean AND/OR keyword search
- **Relevance Ranking**: Score and rank search results

All functions use case-insensitive matching and support Unicode characters.

**Note:** These functions operate directly on property values **without using indexes**. For indexed full-text search, see the Tantivy-based functions.

## Core Algorithm: Levenshtein Distance

All fuzzy matching functions are built on the Levenshtein distance algorithm, which calculates the minimum number of single-character edits (insertions, deletions, or substitutions) needed to transform one string into another.

**Example**: The edit distance between "kitten" and "sitting" is 3:
1. kitten → sitten (substitution: k → s)
2. sitten → sittin (substitution: e → i)
3. sittin → sitting (insertion: g)

## Available Functions

### Fuzzy Search Functions (Levenshtein-based)

### 1. FT_FUZZY_MATCH

Returns true if two strings are similar within a specified edit distance threshold.

**Syntax**:
```gql
FT_FUZZY_MATCH(string1, string2, max_distance)
```

**Parameters**:
- `string1`: First string to compare
- `string2`: Second string to compare
- `max_distance`: Maximum allowed edit distance (integer)

**Returns**: Boolean

**Use Cases**:
- Filtering results with typo tolerance
- Deduplication with approximate matching
- Input validation with fuzzy comparison

**Examples**:

```gql
-- Find papers where title fuzzy matches "machine learning" within 2 edits
MATCH (p:Paper)
WHERE FT_FUZZY_MATCH(p.title, 'machine learning', 2)
RETURN p.title;

-- Strict matching (only 1 character difference allowed)
MATCH (p:Paper)
WHERE FT_FUZZY_MATCH(p.abstract, 'neural network', 1)
RETURN p.title;

-- Lenient matching (allows more typos)
MATCH (p:Paper)
WHERE FT_FUZZY_MATCH(p.abstract, 'deep learning', 3)
RETURN p.title;
```

**Performance Characteristics**:
- Time Complexity: O(m × n) where m, n are string lengths
- Space Complexity: O(m × n) for dynamic programming matrix
- Best for: Short to medium strings (< 1000 characters)

---

### 2. FT_SIMILARITY_SCORE

Calculates a normalized Levenshtein-based similarity score between two strings, ranging from 0.0 (completely different) to 1.0 (identical).

**Syntax**:
```gql
FT_SIMILARITY_SCORE(string1, string2)
```

**Parameters**:
- `string1`: First string to compare
- `string2`: Second string to compare

**Returns**: Number (0.0 to 1.0)

**Formula**:
```
similarity = 1.0 - (levenshtein_distance / max_length)
where max_length = max(length(string1), length(string2))
```

**Important Notes**:
- **WARNING: Works best for strings of similar length**
- Penalizes length differences heavily (e.g., "cat" vs "catastrophe" = low score despite prefix match)
- Uses max length for normalization, not average or sum
- For substring matching, use FUZZY_SEARCH or CONTAINS_FUZZY instead

**Use Cases**:
- Comparing strings of similar length (titles, codes, identifiers)
- Finding near-duplicates in uniform-length fields
- Deduplication when string lengths are comparable
- **Not ideal for**: Prefix matching, comparing short vs long strings

**Examples**:

```gql
-- Compare paper titles of similar length
MATCH (p:Paper)
WHERE length(p.title) BETWEEN 30 AND 50
RETURN p.title,
       FT_SIMILARITY_SCORE(p.title, 'Machine Learning for Healthcare') AS score
ORDER BY score DESC
LIMIT 10;

-- Find near-duplicate titles (similar lengths)
MATCH (p1:Paper), (p2:Paper)
WHERE p1.id < p2.id
  AND abs(length(p1.title) - length(p2.title)) < 10  -- Similar lengths
  AND FT_SIMILARITY_SCORE(p1.title, p2.title) > 0.8
RETURN p1.title AS title1,
       p2.title AS title2,
       FT_SIMILARITY_SCORE(p1.title, p2.title) AS similarity;

-- Compare fixed-length codes or identifiers
MATCH (d:Document)
WHERE FT_SIMILARITY_SCORE(d.product_code, 'ABC-12345') > 0.7
RETURN d.product_code,
       FT_SIMILARITY_SCORE(d.product_code, 'ABC-12345') AS similarity
ORDER BY similarity DESC;
```

**Interpretation**:
- **1.0**: Identical strings
- **0.8-0.9**: Very similar (minor typos, similar lengths)
- **0.6-0.7**: Moderately similar (several differences, similar lengths)
- **< 0.5**: Significantly different
- **Low scores with length mismatch**: Expected behavior (e.g., "test" vs "testing for bugs" will score low)

---

### 3. FT_CONTAINS_FUZZY

Returns true if the text contains the query as a fuzzy substring within the specified edit distance.

**Syntax**:
```gql
FT_CONTAINS_FUZZY(text, query, max_distance)
```

**Parameters**:
- `text`: Text to search in
- `query`: Substring to search for
- `max_distance`: Maximum edit distance allowed

**Returns**: Boolean

**Algorithm**:
1. First checks for exact substring match
2. If no exact match, uses sliding window to check all substrings of length equal to query
3. Returns true if any substring is within edit distance threshold

**Use Cases**:
- Fuzzy substring search in long documents
- Finding mentions with typo tolerance
- Flexible keyword matching

**Examples**:

```gql
-- Find documents containing "machine learning" (allowing 2 typos)
MATCH (d:Document)
WHERE FT_CONTAINS_FUZZY(d.content, 'machine learning', 2)
RETURN d.title;

-- Find papers mentioning "neural network" with typo tolerance
MATCH (p:Paper)
WHERE FT_CONTAINS_FUZZY(p.abstract, 'neural network', 1)
RETURN p.title;

-- Multiple fuzzy conditions (AND logic)
MATCH (p:Paper)
WHERE FT_CONTAINS_FUZZY(p.abstract, 'deep', 1)
  AND FT_CONTAINS_FUZZY(p.abstract, 'learning', 1)
RETURN p.title;
```

**Performance**:
- Time Complexity: O(n × m²) where n is text length, m is query length
- Best for: Queries shorter than 50 characters
- Optimization: Exact match is checked first (O(n) fast path)

---

### 4. FT_FUZZY_SEARCH

Returns a relevance score for how well a query matches text, optimized for ranking search results.

**Syntax**:
```gql
FT_FUZZY_SEARCH(text, query)
```

**Parameters**:
- `text`: Text to search in
- `query`: Search query

**Returns**: Number (0.0 to 1.0)

**Algorithm**:
1. Checks for exact substring match (returns 1.0 immediately)
2. Uses sliding window to find best fuzzy match across all substrings
3. Returns highest similarity score found

**Use Cases**:
- Ranking search results
- Finding most relevant documents
- Implementing search engines

**Examples**:

```gql
-- Rank papers by relevance to query
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'neural networks') > 0.5
RETURN p.title,
       FT_FUZZY_SEARCH(p.abstract, 'neural networks') AS relevance
ORDER BY relevance DESC
LIMIT 20;

-- Multi-term search
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'machine learning algorithms') > 0.6
RETURN p.title,
       FT_FUZZY_SEARCH(p.abstract, 'machine learning algorithms') AS score
ORDER BY score DESC;

-- Combine with other filters
MATCH (p:Paper)
WHERE p.year >= 2020
  AND FT_FUZZY_SEARCH(p.abstract, 'deep learning') > 0.7
RETURN p.title, p.year,
       FT_FUZZY_SEARCH(p.abstract, 'deep learning') AS relevance
ORDER BY relevance DESC, p.year DESC;
```

**Score Interpretation**:
- **1.0**: Exact match found
- **0.8-0.9**: Very close match (1-2 character differences)
- **0.6-0.7**: Moderate match (several differences)
- **< 0.5**: Weak match

---

### 5. FT_HYBRID_SEARCH

Combines exact matching, fuzzy substring matching, and overall similarity into a single weighted score.

**Syntax**:
```gql
FT_HYBRID_SEARCH(text, query)
FT_HYBRID_SEARCH(text, query, exact_weight, fuzzy_weight, similarity_weight)
```

**Parameters**:
- `text`: Text to search in
- `query`: Search query
- `exact_weight`: Weight for exact substring matching (optional, default: 0.4)
- `fuzzy_weight`: Weight for fuzzy substring matching (optional, default: 0.4)
- `similarity_weight`: Weight for overall similarity (optional, default: 0.2)

**Returns**: Number (0.0 to 1.0)

**Algorithm**:
1. **Exact Score**: 1.0 if query is exact substring, 0.0 otherwise
2. **Fuzzy Score**: Best fuzzy substring match using sliding window
3. **Similarity Score**: Overall Levenshtein-based similarity
4. **Combined**: `(exact × w1 + fuzzy × w2 + similarity × w3) / (w1 + w2 + w3)`

**Use Cases**:
- Advanced search engines
- Multi-strategy relevance ranking
- Balancing precision and recall

**Examples**:

```gql
-- Default weights (0.4 exact, 0.4 fuzzy, 0.2 similarity)
MATCH (p:Paper)
WHERE FT_HYBRID_SEARCH(p.abstract, 'machine learning') > 0.5
RETURN p.title,
       FT_HYBRID_SEARCH(p.abstract, 'machine learning') AS score
ORDER BY score DESC;

-- Custom weights: prioritize exact matches
MATCH (p:Paper)
WHERE FT_HYBRID_SEARCH(p.abstract, 'neural networks', 0.7, 0.2, 0.1) > 0.6
RETURN p.title,
       FT_HYBRID_SEARCH(p.abstract, 'neural networks', 0.7, 0.2, 0.1) AS score
ORDER BY score DESC;

-- Balanced weights for exploration
MATCH (p:Paper)
WHERE FT_HYBRID_SEARCH(p.abstract, 'deep learning', 0.33, 0.33, 0.34) > 0.4
RETURN p.title,
       FT_HYBRID_SEARCH(p.abstract, 'deep learning', 0.33, 0.33, 0.34) AS score
ORDER BY score DESC;

-- Similarity-focused (emphasize overall text similarity)
MATCH (p:Paper)
WHERE FT_HYBRID_SEARCH(p.abstract, 'AI research', 0.2, 0.2, 0.6) > 0.3
RETURN p.title,
       FT_HYBRID_SEARCH(p.abstract, 'AI research', 0.2, 0.2, 0.6) AS score
ORDER BY score DESC;
```

**Weight Tuning Recommendations**:
- **High Precision**: (0.7, 0.2, 0.1) - Favor exact matches
- **Balanced**: (0.4, 0.4, 0.2) - Default, works well for most cases
- **High Recall**: (0.2, 0.4, 0.4) - More lenient, finds more results
- **Similarity-Focused**: (0.2, 0.2, 0.6) - Emphasize overall text similarity (note: still Levenshtein-based, not semantic)

---

### 6. FT_KEYWORD_MATCH

Matches text against multiple keywords using OR logic (returns true if ANY keyword matches).

**Syntax**:
```gql
FT_KEYWORD_MATCH(text, keyword1, keyword2, ...)
```

**Parameters**:
- `text`: Text to search in
- `keyword1, keyword2, ...`: Variable number of keywords (minimum 1)

**Returns**: Boolean

**Use Cases**:
- Multi-keyword filtering with OR logic
- Category-based search
- Tag matching

**Examples**:

```gql
-- Find papers mentioning any programming language
MATCH (p:Paper)
WHERE FT_KEYWORD_MATCH(p.abstract, 'Python', 'Java', 'JavaScript', 'C++')
RETURN p.title;

-- Match any of several related terms
MATCH (p:Paper)
WHERE FT_KEYWORD_MATCH(p.content, 'machine learning', 'deep learning', 'AI', 'neural network')
RETURN p.title;

-- Combine with other conditions
MATCH (p:Paper)
WHERE p.year > 2020
  AND FT_KEYWORD_MATCH(p.tags, 'NLP', 'computer vision', 'reinforcement learning')
RETURN p.title, p.year;
```

**Behavior**:
- Case-insensitive matching
- Checks for exact substring matches
- Returns true on first match (short-circuits)
- NULL keywords are ignored

---

### 7. FT_KEYWORD_MATCH_ALL

Matches text against multiple keywords using AND logic (returns true only if ALL keywords match).

**Syntax**:
```gql
FT_KEYWORD_MATCH_ALL(text, keyword1, keyword2, ...)
```

**Parameters**:
- `text`: Text to search in
- `keyword1, keyword2, ...`: Variable number of keywords (minimum 1)

**Returns**: Boolean

**Use Cases**:
- Precise multi-term filtering
- Requirement-based search
- Conjunction queries

**Examples**:

```gql
-- Find papers containing all specified terms
MATCH (p:Paper)
WHERE FT_KEYWORD_MATCH_ALL(p.abstract, 'machine', 'learning', 'deep')
RETURN p.title;

-- Strict multi-keyword filter
MATCH (p:Paper)
WHERE FT_KEYWORD_MATCH_ALL(p.content, 'neural', 'network', 'training')
RETURN p.title;

-- Combine with fuzzy matching
MATCH (p:Paper)
WHERE FT_KEYWORD_MATCH_ALL(p.abstract, 'machine', 'learning')
  AND FT_CONTAINS_FUZZY(p.abstract, 'algorithm', 2)
RETURN p.title;
```

**Behavior**:
- Case-insensitive matching
- All keywords must be present as substrings
- Returns false if any keyword is missing
- NULL keywords are ignored

---

### 8. FT_WEIGHTED_SEARCH

Calculates a weighted search score with explicit control over exact, fuzzy, and similarity components.

**Syntax**:
```gql
FT_WEIGHTED_SEARCH(text, query, exact_weight, fuzzy_weight, similarity_weight)
```

**Parameters**:
- `text`: Text to search in
- `query`: Search query
- `exact_weight`: Weight for exact matching (0.0-1.0)
- `fuzzy_weight`: Weight for fuzzy matching (0.0-1.0)
- `similarity_weight`: Weight for similarity (0.0-1.0)

**Returns**: Number (0.0 to 1.0)

**Note**: This function is identical to HYBRID_SEARCH with explicit weights. Use WEIGHTED_SEARCH when you want to make weight configuration explicit.

**Use Cases**:
- Fine-tuned search ranking
- A/B testing different weight configurations
- Domain-specific search optimization

**Examples**:

```gql
-- Text-focused search (70% exact, 20% fuzzy, 10% similarity)
MATCH (p:Paper)
WHERE FT_WEIGHTED_SEARCH(p.abstract, 'neural network', 0.7, 0.2, 0.1) > 0.6
RETURN p.title,
       FT_WEIGHTED_SEARCH(p.abstract, 'neural network', 0.7, 0.2, 0.1) AS score
ORDER BY score DESC;

-- Compare different weighting strategies
MATCH (p:Paper)
RETURN p.title,
       FT_WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.7, 0.2, 0.1) AS exact_focused,
       FT_WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.33, 0.33, 0.34) AS balanced,
       FT_WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.1, 0.3, 0.6) AS similarity_focused
ORDER BY balanced DESC
LIMIT 10;

-- Performance-optimized with pre-filter
MATCH (p:Paper)
WHERE FT_CONTAINS_FUZZY(p.abstract, 'learning', 2)
  AND FT_WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.6, 0.3, 0.1) > 0.7
RETURN p.title,
       FT_WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.6, 0.3, 0.1) AS score
ORDER BY score DESC;
```

## Real-World Usage Patterns

### Pattern 1: Typo-Tolerant Search

```gql
-- Find documents even with misspellings
MATCH (d:Document)
WHERE FT_CONTAINS_FUZZY(d.content, 'machne lerning', 2)
RETURN d.title, d.content;
```

### Pattern 2: Ranked Search Results

```gql
-- Rank by relevance
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'deep learning') > 0.5
RETURN p.title,
       FT_FUZZY_SEARCH(p.abstract, 'deep learning') AS relevance
ORDER BY relevance DESC
LIMIT 20;
```

### Pattern 3: Multi-Strategy Search

```gql
-- Combine exact and fuzzy matching
MATCH (p:Paper)
WHERE FT_KEYWORD_MATCH_ALL(p.title, 'machine', 'learning')
  OR FT_CONTAINS_FUZZY(p.abstract, 'machine learning', 2)
RETURN p.title;
```

### Pattern 4: Similarity-Based Deduplication

```gql
-- Find near-duplicate papers (titles have similar lengths)
MATCH (p1:Paper), (p2:Paper)
WHERE p1.id < p2.id
  AND abs(length(p1.title) - length(p2.title)) < 15  -- Similar lengths
  AND FT_SIMILARITY_SCORE(p1.title, p2.title) > 0.85
RETURN p1.title AS original,
       p2.title AS duplicate,
       FT_SIMILARITY_SCORE(p1.title, p2.title) AS similarity
ORDER BY similarity DESC;
```

### Pattern 5: Author Expertise Ranking

```gql
-- Find authors by research area with fuzzy matching
MATCH (author:Author)-[:WROTE]->(paper:Paper)
WHERE FT_HYBRID_SEARCH(paper.abstract, 'quantum computing') > 0.6
RETURN author.name,
       COUNT(paper) AS papers,
       AVG(FT_HYBRID_SEARCH(paper.abstract, 'quantum computing')) AS avg_relevance
GROUP BY author.name
ORDER BY avg_relevance DESC, papers DESC
LIMIT 10;
```

### Pattern 6: Complex Boolean Search

```gql
-- (neural OR deep) AND (learning OR network) with fuzzy matching
MATCH (p:Paper)
WHERE (FT_CONTAINS_FUZZY(p.abstract, 'neural', 1) OR FT_CONTAINS_FUZZY(p.abstract, 'deep', 1))
  AND (FT_CONTAINS_FUZZY(p.abstract, 'learning', 1) OR FT_CONTAINS_FUZZY(p.abstract, 'network', 1))
RETURN p.title;
```

### Pattern 7: Threshold-Based Filtering

```gql
-- Use different thresholds for different fields
-- Note: Works best when field lengths are similar to query length
MATCH (p:Paper)
WHERE FT_SIMILARITY_SCORE(p.title, 'neural networks') > 0.8
   OR FT_FUZZY_SEARCH(p.abstract, 'neural networks') > 0.6  -- Better for variable-length text
RETURN p.title,
       FT_SIMILARITY_SCORE(p.title, 'neural networks') AS title_score,
       FT_FUZZY_SEARCH(p.abstract, 'neural networks') AS abstract_score
ORDER BY title_score DESC, abstract_score DESC;
```

## Performance Optimization

### 1. Pre-filtering

Use fast filters before expensive fuzzy operations:

```gql
-- BAD: Fuzzy search on all documents
MATCH (d:Document)
WHERE FT_FUZZY_SEARCH(d.content, 'machine learning') > 0.7
RETURN d.title;

-- GOOD: Pre-filter with exact match first
MATCH (d:Document)
WHERE d.content CONTAINS 'machine' OR d.content CONTAINS 'learning'
  AND FT_FUZZY_SEARCH(d.content, 'machine learning') > 0.7
RETURN d.title;
```

### 2. Threshold Selection

Higher thresholds = fewer results, better performance:

```gql
-- Lower threshold (0.5) = more computation
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'artificial intelligence') > 0.5
RETURN p.title;

-- Higher threshold (0.8) = faster, more precise
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'artificial intelligence') > 0.8
RETURN p.title;
```

### 3. Limit Results Early

Use LIMIT to stop processing early:

```gql
-- Stop after finding 10 matches
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'neural networks') > 0.7
RETURN p.title
ORDER BY FT_FUZZY_SEARCH(p.abstract, 'neural networks') DESC
LIMIT 10;
```

### 4. Choose Right Function

Use the simplest function that meets your needs:

| Requirement | Best Function | Complexity |
|-------------|---------------|------------|
| Exact boolean filter | KEYWORD_MATCH | O(n) |
| Fuzzy boolean filter | CONTAINS_FUZZY | O(n × m²) |
| Similarity ranking (similar lengths) | FT_SIMILARITY_SCORE | O(m × n) |
| Substring ranking | FUZZY_SEARCH | O(n × m²) |
| Multi-strategy ranking | HYBRID_SEARCH | O(n × m²) |

## Common Pitfalls

### 1. Threshold Too Low

```gql
-- BAD: 0.3 threshold returns too many irrelevant results
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'AI') > 0.3
RETURN p.title;

-- GOOD: 0.7 threshold filters to relevant results
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'AI') > 0.7
RETURN p.title;
```

### 2. Wrong Edit Distance

```gql
-- BAD: Max distance 5 allows too many false matches
MATCH (p:Paper)
WHERE FT_FUZZY_MATCH(p.title, 'machine learning', 5)
RETURN p.title;

-- GOOD: Max distance 2 allows minor typos only
MATCH (p:Paper)
WHERE FT_FUZZY_MATCH(p.title, 'machine learning', 2)
RETURN p.title;
```

### 3. Not Using ORDER BY

```gql
-- BAD: Results not ranked by relevance
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'neural networks') > 0.5
RETURN p.title;

-- GOOD: Results ranked by relevance
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'neural networks') > 0.5
RETURN p.title,
       FT_FUZZY_SEARCH(p.abstract, 'neural networks') AS score
ORDER BY score DESC;
```

## Function Selection Guide

| Use Case | Recommended Function | Why |
|----------|---------------------|-----|
| Exact match with typo tolerance | FUZZY_MATCH | Boolean result, configurable threshold |
| Ranking search results | FUZZY_SEARCH | Optimized for relevance scoring |
| String similarity (similar lengths) | FT_SIMILARITY_SCORE | Simple, normalized score |
| Fuzzy substring search | CONTAINS_FUZZY | Boolean, works in long text |
| Multi-strategy search | HYBRID_SEARCH | Combines multiple approaches |
| OR keyword search | KEYWORD_MATCH | Fast, multiple keywords |
| AND keyword search | KEYWORD_MATCH_ALL | Precise multi-term filter |
| Custom weighted search | WEIGHTED_SEARCH | Explicit weight control |

## Technical Details

### Complexity Analysis

| Function | Time Complexity | Space Complexity |
|----------|----------------|------------------|
| FUZZY_MATCH | O(m × n) | O(m × n) |
| FT_SIMILARITY_SCORE | O(m × n) | O(m × n) |
| CONTAINS_FUZZY | O(k × m²) where k = text length | O(m²) |
| FUZZY_SEARCH | O(k × m²) | O(m²) |
| HYBRID_SEARCH | O(k × m²) | O(m²) |
| KEYWORD_MATCH | O(k × t) where t = keywords | O(1) |
| KEYWORD_MATCH_ALL | O(k × t) | O(1) |
| WEIGHTED_SEARCH | O(k × m²) | O(m²) |

Where:
- m, n = string lengths being compared
- k = text length for substring search
- t = number of keywords

### Unicode Support

All functions support Unicode:

```gql
-- Works with non-ASCII characters
MATCH (p:Paper)
WHERE FT_FUZZY_MATCH(p.title, 'Künstliche Intelligenz', 2)
RETURN p.title;

-- Works with special characters and symbols
MATCH (d:Document)
WHERE FT_CONTAINS_FUZZY(d.content, 'machine learning!', 2)
RETURN d.title;
```

### NULL Handling

All functions handle NULL values gracefully:

```gql
-- FUZZY_MATCH returns false for NULL
FT_FUZZY_MATCH(NULL, 'test', 2)  -- Returns: false
FT_FUZZY_MATCH('test', NULL, 2)  -- Returns: false

-- FT_SIMILARITY_SCORE returns NULL
FT_SIMILARITY_SCORE(NULL, 'test')  -- Returns: NULL
FT_SIMILARITY_SCORE('test', NULL)  -- Returns: NULL

-- FUZZY_SEARCH returns NULL
FT_FUZZY_SEARCH(NULL, 'test')  -- Returns: NULL
```

## Best Practices

### 1. Always Use ORDER BY for Scoring Functions

```gql
-- Score functions should order results
MATCH (p:Paper)
WHERE FT_FUZZY_SEARCH(p.abstract, 'machine learning') > 0.6
RETURN p.title,
       FT_FUZZY_SEARCH(p.abstract, 'machine learning') AS score
ORDER BY score DESC;
```

### 2. Choose Appropriate Thresholds

- **Edit Distance**: 1-2 for short strings, 2-4 for longer strings
- **Similarity Score**: 0.7-0.8 for similar, 0.9+ for near-identical
- **Fuzzy/Hybrid Search**: 0.6-0.7 for relevant, 0.8+ for highly relevant

### 3. Combine with Other Filters

```gql
-- Use metadata filters first
MATCH (p:Paper)
WHERE p.year >= 2020
  AND p.citations > 100
  AND FT_FUZZY_SEARCH(p.abstract, 'deep learning') > 0.7
RETURN p.title;
```

### 4. Use LIMIT for Large Datasets

```gql
-- Prevent processing entire dataset
MATCH (p:Paper)
WHERE FT_HYBRID_SEARCH(p.abstract, 'AI research') > 0.5
RETURN p.title
ORDER BY FT_HYBRID_SEARCH(p.abstract, 'AI research') DESC
LIMIT 50;
```

### 5. Weight Tuning for Domain

Test different weights for your specific use case:

```gql
-- A/B test different configurations
MATCH (p:Paper)
RETURN p.title,
       FT_HYBRID_SEARCH(p.abstract, 'neural networks', 0.7, 0.2, 0.1) AS config_a,
       FT_HYBRID_SEARCH(p.abstract, 'neural networks', 0.4, 0.4, 0.2) AS config_b,
       FT_HYBRID_SEARCH(p.abstract, 'neural networks', 0.3, 0.3, 0.4) AS config_c
ORDER BY config_b DESC
LIMIT 20;
```

## Pattern Matching Functions

GraphLite also provides **pattern matching functions** for prefix, suffix, wildcard, regex, and autocomplete operations. These functions use the `FT_` prefix to indicate they are full-text search functions.

### 9. FT_STARTS_WITH

Checks if a string starts with a given prefix.

**Syntax**:
```gql
FT_STARTS_WITH(text, prefix)
```

**Parameters**:
- `text`: String to check
- `prefix`: Prefix to match

**Returns**: Boolean

**Examples**:
```gql
-- Find users whose email starts with "admin"
MATCH (u:Person)
WHERE FT_STARTS_WITH(u.email, 'admin')
RETURN u.name, u.email;

-- Find documents with titles starting with "Machine"
MATCH (d:Document)
WHERE FT_STARTS_WITH(d.title, 'Machine')
RETURN d.title;

-- Case-sensitive prefix matching
MATCH (p:Person)
WHERE FT_STARTS_WITH(p.username, 'alice')
RETURN p.username;
```

### 10. FT_ENDS_WITH

Checks if a string ends with a given suffix.

**Syntax**:
```gql
FT_ENDS_WITH(text, suffix)
```

**Parameters**:
- `text`: String to check
- `suffix`: Suffix to match

**Returns**: Boolean

**Examples**:
```gql
-- Find files ending with .pdf extension
MATCH (d:Document)
WHERE FT_ENDS_WITH(d.name, '.pdf')
RETURN d.name;

-- Find email addresses from gmail.com
MATCH (u:Person)
WHERE FT_ENDS_WITH(u.email, '@gmail.com')
RETURN u.email;

-- Find usernames ending with "_admin"
MATCH (u:Person)
WHERE FT_ENDS_WITH(u.username, '_admin')
RETURN u.username;
```

### 11. FT_WILDCARD

Matches text against wildcard patterns using `*` (zero or more characters) and `?` (exactly one character).

**Syntax**:
```gql
FT_WILDCARD(text, pattern)
```

**Parameters**:
- `text`: String to match
- `pattern`: Wildcard pattern with `*` and `?`

**Returns**: Boolean

**Examples**:
```gql
-- Match files with any extension (*.*)
MATCH (d:Document)
WHERE FT_WILDCARD(d.name, '*.pdf')
RETURN d.name;

-- Match usernames starting with 'bob' (bob*)
MATCH (u:Person)
WHERE FT_WILDCARD(u.username, 'bob*')
RETURN u.username;

-- Match three-letter usernames (???)
MATCH (u:Person)
WHERE FT_WILDCARD(u.username, '???')
RETURN u.username;

-- Match product SKUs (ABC-*)
MATCH (p:Person)
WHERE FT_WILDCARD(p.sku, 'ABC-*')
RETURN p.sku, p.name;

-- Complex pattern (user_*)
MATCH (u:Person)
WHERE FT_WILDCARD(u.username, 'user_*')
RETURN u.username;
```

### 12. FT_REGEX

Matches text against regular expression patterns.

**Syntax**:
```gql
FT_REGEX(text, pattern)
```

**Parameters**:
- `text`: String to match
- `pattern`: Regular expression pattern

**Returns**: Boolean

**Supports**:
- `.` - Match any character
- `*` - Zero or more of preceding
- `+` - One or more of preceding
- `?` - Zero or one of preceding
- `[abc]` - Character class
- `[^abc]` - Negated character class
- `^` - Start anchor
- `$` - End anchor
- `|` - Alternation (OR)
- `()` - Grouping
- `\d` - Digit, `\w` - Word char, `\s` - Whitespace

**Examples**:
```gql
-- Match email addresses
MATCH (u:Person)
WHERE FT_REGEX(u.email, '^[a-z]+@[a-z]+\\.com$')
RETURN u.email;

-- Match product SKUs (ABC-123 format)
MATCH (p:Person)
WHERE FT_REGEX(p.sku, '^[A-Z]{3}-[0-9]{3}$')
RETURN p.sku;

-- Match usernames with numbers at end
MATCH (u:Person)
WHERE FT_REGEX(u.username, '.*[0-9]+$')
RETURN u.username;

-- Match admin or moderator roles
MATCH (u:Person)
WHERE FT_REGEX(u.role, '^(admin|moderator)$')
RETURN u.username, u.role;

-- Match file extensions (pdf, png, md)
MATCH (d:Document)
WHERE FT_REGEX(d.name, '\\.(pdf|png|md)$')
RETURN d.name;
```

### 13. FT_PHRASE_PREFIX

Matches phrases with prefix completion on the last word (autocomplete functionality).

**Syntax**:
```gql
FT_PHRASE_PREFIX(text, phrase_prefix)
```

**Parameters**:
- `text`: Text to search in
- `phrase_prefix`: Phrase where the last word is a prefix

**Returns**: Boolean

**Features**:
- Case-insensitive matching
- Tokenizes by whitespace
- Last word must be a prefix match
- All previous words must exact match

**Examples**:
```gql
-- Find documents with titles starting with "Machine Learn"
MATCH (d:Document)
WHERE FT_PHRASE_PREFIX(d.title, 'Machine Learn')
RETURN d.title;
-- Matches: "Machine Learning Basics", "Machine Learning Advanced"

-- Autocomplete for "Deep Lear"
MATCH (d:Document)
WHERE FT_PHRASE_PREFIX(d.title, 'Deep Lear')
RETURN d.title;
-- Matches: "Deep Learning Guide"

-- Single word prefix
MATCH (d:Document)
WHERE FT_PHRASE_PREFIX(d.title, 'Nat')
RETURN d.title;
-- Matches: "Natural Language Processing"

-- Three word phrase prefix
MATCH (d:Document)
WHERE FT_PHRASE_PREFIX(d.title, 'Machine Learning Bas')
RETURN d.title;
-- Matches: "Machine Learning Basics"
```

### Pattern Matching Function Comparison

| Function | Use Case | Example Pattern | Matches |
|----------|----------|-----------------|---------|
| `FT_STARTS_WITH` | Prefix matching | `"admin"` | "admin", "admin123", "administrator" |
| `FT_ENDS_WITH` | Suffix matching | `".pdf"` | "document.pdf", "report.PDF" |
| `FT_WILDCARD` | Wildcard patterns | `"*.pdf"` | Any file ending with .pdf |
| `FT_REGEX` | Complex patterns | `"^[A-Z]{3}-[0-9]{3}$"` | "ABC-123", "XYZ-456" |
| `FT_PHRASE_PREFIX` | Autocomplete | `"Machine Learn"` | "Machine Learning..." |

### Performance Notes

All pattern matching functions use **string-based operations** (Phase 1 implementation):
- **Time Complexity**: O(n) where n is the number of nodes
- **Space Complexity**: O(1) for most functions, O(m) for regex compilation
- **Best For**: Small to medium datasets (< 100K records)
- **Future**: Phase 2 will add Tantivy-indexed implementations for O(log n) performance

### Combining Pattern Matching with Fuzzy Search

```gql
-- Find PDF documents with fuzzy title matching
MATCH (d:Document)
WHERE FT_ENDS_WITH(d.name, '.pdf')
  AND FT_FUZZY_SEARCH(d.title, 'machine learning') > 0.7
RETURN d.name, d.title;

-- Find admin users with email domain filtering
MATCH (u:Person)
WHERE FT_STARTS_WITH(u.username, 'admin')
  AND FT_ENDS_WITH(u.email, '@company.com')
RETURN u.username, u.email;

-- Wildcard pattern with fuzzy content search
MATCH (d:Document)
WHERE FT_WILDCARD(d.name, '*.md')
  AND FT_CONTAINS_FUZZY(d.content, 'documentation', 2)
RETURN d.name;
```

## Summary

GraphLite's text search functions provide powerful, flexible tools for:

- **Fuzzy Matching**: Handle typos and variations using Levenshtein distance
- **Similarity Scoring**: Rank by relevance using normalized edit distance
- **Hybrid Search**: Combine multiple text-matching strategies (exact, fuzzy, similarity) with configurable weights
- **Keyword Matching**: Boolean logic for filtering
- **Pattern Matching**: Prefix, suffix, wildcard, regex, and autocomplete operations
- **Performance**: Optimized algorithms with configurable behavior

All search functions are **text-based** using Levenshtein distance and string algorithms. Choose the right function for your use case, tune thresholds and weights appropriately, and combine with standard GQL filters for optimal results.
