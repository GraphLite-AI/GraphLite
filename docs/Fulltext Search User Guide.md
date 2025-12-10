# User Guide: Fuzzy and Hybrid Search in GraphLite

## Overview

GraphLite provides powerful **fuzzy search** for typo-tolerant matching and **hybrid search** that combines full-text search with vector similarity. This guide demonstrates a complete workflow from schema creation to complex queries.

---

## Quick Start: Complete Workflow

### Step 1: Create Graph Schema

First, let's create a schema with nodes that have both text and vector properties:

```gql
-- Create Paper nodes with abstract text and embedding vector
CREATE VERTEX Paper(
    id INT PRIMARY KEY,
    title STRING,
    abstract STRING,
    embedding LIST<DOUBLE>
);

-- Create Author nodes
CREATE VERTEX Author(
    id INT PRIMARY KEY,
    name STRING,
    affiliation STRING
);
```

### Step 2: Create Text and Vector Indexes

```gql
-- Create relationships
CREATE DIRECTED EDGE WROTE(FROM Paper, TO Author) WITH REVERSE_EDGE = "WRITTEN_BY";
-- Create inverted index for full-text search on paper abstracts
CREATE TEXT INDEX papers_abstract_idx 
ON Paper (abstract) 
OPTIONS (analyzer='english');

-- Create vector index for semantic search
CREATE VECTOR INDEX papers_embedding_idx 
ON Paper (embedding) 
TYPE DISKANN 
OPTIONS (dimension=384, metric='cosine');
```

Index Types Used:

TEXT INDEX - For fast full-text search (supports fuzzy matching)

VECTOR INDEX - For semantic similarity search using DISKANN algorithm

### Step 3: Insert Sample Data

```gql
-- Insert sample papers with embeddings
INSERT INTO Paper VALUES
(1, "Graph Neural Networks for Molecular Property Prediction", 
 "Graph neural networks (GNNs) have revolutionized molecular property prediction in drug discovery. This paper introduces a novel GNN architecture that achieves state-of-the-art results on benchmark datasets.",
 [0.1, 0.2, 0.3, ...]),
(2, "Attention Mechanisms in Transformer Architectures",
 "Attention mechanisms form the core of transformer models used in natural language processing. We analyze different attention variants and their impact on model performance.",
 [0.2, 0.3, 0.1, ...]),
(3, "Machine Learning for Healthcare Diagnostics",
 "Machine learning techniques are transforming healthcare diagnostics. Our study applies deep learning to medical imaging with promising results for early disease detection.",
 [0.3, 0.1, 0.2, ...]),
(4, "Federated Learning: Privacy-Preserving ML",
 "Federated learning enables collaborative model training without sharing raw data. This paper proposes a novel aggregation method that improves convergence rates.",
 [0.4, 0.5, 0.6, ...]),
(5, "Quantum Machine Learning Algorithms",
 "Quantum computing offers new paradigms for machine learning. We introduce quantum variants of classical algorithms and demonstrate speedups on specific problem classes.",
 [0.5, 0.6, 0.4, ...]);

-- Insert authors
INSERT INTO Author VALUES
(101, "Alice Chen", "Stanford University"),
(102, "Bob Smith", "MIT"),
(103, "Carol Davis", "Google Research"),
(104, "David Wilson", "Harvard University");

-- Create authorship relationships
INSERT INTO WROTE VALUES
(1, 101), (1, 102),  -- Paper 1 by Alice and Bob
(2, 103),            -- Paper 2 by Carol
(3, 101), (3, 104),  -- Paper 3 by Alice and David
(4, 102), (4, 103),  -- Paper 4 by Bob and Carol
(5, 104);            -- Paper 5 by David
```

### Step 4: Basic Fuzzy Search

Fuzzy search helps find documents despite typos or misspellings:

```gql
-- Find papers about "neural networks" with typo tolerance
MATCH (p:Paper)
WHERE text_search(p.abstract, 'nural network', {
    fuzzy: true,
    fuzzy_distance: 2
}) > 0
RETURN p.title, 
       text_search(p.abstract, 'nural network', {fuzzy: true}) AS fuzzy_score
ORDER BY fuzzy_score DESC;

-- Results will include papers with "neural networks" despite the misspelling
```

#### Output

```
| title                                          | fuzzy_score |
|------------------------------------------------|-------------|
| Graph Neural Networks for Molecular Property...| 8.75        |
| Attention Mechanisms in Transformer Archit...  | 3.21        |
```

### Step 5: Fuzzy Search with Options

Control fuzzy search behavior with different options:

```gql
-- Strict fuzzy matching (small edit distance)
MATCH (p:Paper)
WHERE text_search(p.abstract, 'mashine lurning', {
    fuzzy: true,
    fuzzy_distance: 1,        -- Only 1 character edit allowed
    require_all_terms: true   -- All terms must match
}) > 0.5
RETURN p.title;

-- Lenient fuzzy matching
MATCH (p:Paper)
WHERE text_search(p.abstract, 'artifical inteligence', {
    fuzzy: true,
    fuzzy_distance: 3,        -- Up to 3 character edits
    require_all_terms: false  -- Any term can match (OR logic)
}) > 0
RETURN p.title;

-- Fuzzy with stemming disabled (exact word matching)
MATCH (p:Paper)
WHERE text_search(p.abstract, 'learning algorithms', {
    fuzzy: true,
    stemming: false,          -- Don't stem "learning" to "learn"
    fuzzy_distance: 2
}) > 0
RETURN p.title;
```

### Step 6: Proximity with Fuzzy Search

Combine proximity and fuzzy search for flexible matching:

```gql
-- Find "machine" and "learning" within 5 words, with typos allowed
MATCH (p:Paper)
WHERE text_search(p.abstract, '"mashine lurning"~5', {
    fuzzy: true,
    fuzzy_distance: 2
}) > 0
RETURN p.title;

-- Complex query: fuzzy proximity with OR logic
MATCH (p:Paper)
WHERE text_search(p.abstract, '("neural network"~3 OR "deep lurning"~4)', {
    fuzzy: true,
    fuzzy_distance: 2,
    require_all_terms: false
}) > 1.0
RETURN p.title, 
       text_search(p.abstract, '("neural network"~3 OR "deep lurning"~4)', 
                  {fuzzy: true}) AS score
ORDER BY score DESC;
```

### Step 7: Hybrid Search (Text + Vector)

Combine fuzzy text search with semantic vector search:

```gql
-- Define a query embedding (in practice, this comes from an embedding model)
LET $query_embedding = [0.15, 0.25, 0.35, ...];  -- Embedding for "AI in healthcare"

-- Hybrid search: fuzzy text match AND semantic similarity
MATCH (p:Paper)
WHERE text_search(p.abstract, 'helthcare AI', {
    fuzzy: true,
    fuzzy_distance: 2
}) > 2.0
  AND vector_similarity(p.embedding, $query_embedding) > 0.7
RETURN p.title,
       text_search(p.abstract, 'helthcare AI', {fuzzy: true}) AS text_score,
       vector_similarity(p.embedding, $query_embedding) AS vector_score,
       (text_search(p.abstract, 'helthcare AI', {fuzzy: true}) * 0.6 +
        vector_similarity(p.embedding, $query_embedding) * 0.4) AS hybrid_score
ORDER BY hybrid_score DESC
LIMIT 10;
```

### Step 8: Advanced Hybrid Search with Graph Traversal

Find authors who write about topics similar to a query:

```gql
-- Hybrid search within graph patterns
LET $query = 'quantom computting and machine lurning';
LET $query_embedding = [0.3, 0.4, 0.25, ...];  -- Embedding for the query

MATCH (author:Author)-[:WROTE]->(paper:Paper)
WHERE text_search(paper.abstract, $query, {
    fuzzy: true,
    fuzzy_distance: 3,
    require_all_terms: false
}) > 1.5
  AND vector_similarity(paper.embedding, $query_embedding) > 0.65
RETURN author.name,
       author.affiliation,
       COUNT(paper) AS relevant_papers,
       AVG(text_search(paper.abstract, $query, {fuzzy: true})) AS avg_text_score,
       AVG(vector_similarity(paper.embedding, $query_embedding)) AS avg_vector_score,
       (AVG(text_search(paper.abstract, $query, {fuzzy: true})) * 0.5 +
        AVG(vector_similarity(paper.embedding, $query_embedding)) * 0.5) AS author_score
ORDER BY author_score DESC, relevant_papers DESC
LIMIT 20;
```

### Step 9: Boolean Operations with Fuzzy Search

Combine fuzzy search with boolean logic:

```gql
-- Fuzzy AND logic (default)
MATCH (p:Paper)
WHERE text_search(p.abstract, 'mashine lurning', {
    fuzzy: true,
    require_all_terms: true  -- Must match both terms (AND)
}) > 0
RETURN p.title;

-- Fuzzy OR logic
MATCH (p:Paper)
WHERE text_search(p.abstract, 'neural OR quantom', {
    fuzzy: true,
    require_all_terms: false  -- Match any term (OR)
}) > 0
RETURN p.title;

-- Fuzzy with exclusion
MATCH (p:Paper)
WHERE text_search(p.abstract, 'mashine -statistics', {
    fuzzy: true
}) > 0
RETURN p.title;
-- Finds papers about "machine" (with typos) but NOT about "statistics"
```

### Step 10: Performance-Optimized Hybrid Search

Use score thresholds and early filtering for better performance:

```gql
-- Optimized hybrid query with early filtering
LET $query_text = 'federated lurning privasy';
LET $query_embedding = [0.4, 0.5, 0.55, ...];

-- First: filter by text score (uses text index)
MATCH (p:Paper)
WHERE text_search(p.abstract, $query_text, {
    fuzzy: true,
    fuzzy_distance: 2
}) > 3.0  -- Text score threshold
WITH p, 
     text_search(p.abstract, $query_text, {fuzzy: true}) AS t_score
-- Then: filter by vector similarity (uses vector index)
WHERE vector_similarity(p.embedding, $query_embedding) > 0.75
RETURN p.title,
       t_score,
       vector_similarity(p.embedding, $query_embedding) AS v_score,
       (t_score * 0.7 + vector_similarity(p.embedding, $query_embedding) * 0.3) AS final_score
ORDER BY final_score DESC
LIMIT 15;
```

