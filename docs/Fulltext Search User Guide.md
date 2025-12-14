# User Guide: Fuzzy and Hybrid Search in GraphLite

## Overview

GraphLite provides powerful **fuzzy search** for typo-tolerant matching and **hybrid search** that combines full-text search with vector similarity. This guide demonstrates a complete workflow from schema creation to complex queries.

---

## Quick Start: Complete Workflow

### Step 1: Create Graph Schema

First, let's create a schema with nodes that have both text and vector properties:

General setup:

-- Build the project

```bash
./scripts/build_all.sh
./target/release/graphlite gql --path ./test_db -u admin -p admin
```

-- Set up database

```gql
CREATE SCHEMA papers_schema;
SESSION SET SCHEMA papers_schema;

CREATE GRAPH papers_graph;
SESSION SET GRAPH papers_graph;

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

### Step 2: Insert Data with Vector Properties

```gql
-- Insert sample papers with embeddings
INSERT (:Paper {
  title: "Graph Neural Networks for Molecular Property Prediction", 
  abstract: "Graph neural networks (GNNs) have revolutionized molecular property prediction in drug discovery. This paper introduces a novel GNN architecture that achieves state-of-the-art results on benchmark datasets.",
  embedding: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]
}),
(:Paper {
  title: "Attention Mechanisms in Transformer Architectures",
  abstract: "Attention mechanisms form the core of transformer models used in natural language processing. We analyze different attention variants and their impact on model performance.",
  embedding: [0.2, 0.3, 0.1, 0.4, 0.6, 0.5, 0.8, 0.7]
}),
(:Paper {
  title: "Machine Learning for Healthcare Diagnostics",
  abstract: "Machine learning techniques are transforming healthcare diagnostics. Our study applies deep learning to medical imaging with promising results for early disease detection.",
  embedding: [0.3, 0.1, 0.2, 0.5, 0.4, 0.7, 0.6, 0.8]
}),
(:Paper {
  title: "Federated Learning: Privacy-Preserving ML",
  abstract: "Federated learning enables collaborative model training without sharing raw data. This paper proposes a novel aggregation method that improves convergence rates.",
  embedding: [0.4, 0.5, 0.6, 0.7, 0.8, 0.1, 0.2, 0.3]
}),
(:Paper {
  title: "Quantum Machine Learning Algorithms",
  abstract: "Quantum computing offers new paradigms for machine learning. We introduce quantum variants of classical algorithms and demonstrate speedups on specific problem classes.",
  embedding: [0.5, 0.6, 0.4, 0.8, 0.7, 0.2, 0.3, 0.1]
});

-- Insert authors
INSERT (:Author {name: "Alice Chen", affiliation: "Stanford University"}),
(:Author {name: "Bob Smith", affiliation: "MIT"}),
(:Author {name: "Carol Davis", affiliation: "Google Research"}),
(:Author {name: "David Wilson", affiliation: "Harvard University"});
```

### Step 3: Create Relationships

```gql
-- Create authorship relationships
MATCH (p1:Paper {title: "Graph Neural Networks for Molecular Property Prediction"}), (a1:Author {name: "Alice Chen"}) INSERT (p1)-[:WROTE]->(a1);
MATCH (p1:Paper {title: "Graph Neural Networks for Molecular Property Prediction"}), (a2:Author {name: "Bob Smith"}) INSERT (p1)-[:WROTE]->(a2);

MATCH (p2:Paper {title: "Attention Mechanisms in Transformer Architectures"}), (a3:Author {name: "Carol Davis"}) INSERT (p2)-[:WROTE]->(a3);

MATCH (p3:Paper {title: "Machine Learning for Healthcare Diagnostics"}), (a1:Author {name: "Alice Chen"}) INSERT (p3)-[:WROTE]->(a1);
MATCH (p3:Paper {title: "Machine Learning for Healthcare Diagnostics"}), (a4:Author {name: "David Wilson"}) INSERT (p3)-[:WROTE]->(a4);

MATCH (p4:Paper {title: "Federated Learning: Privacy-Preserving ML"}), (a2:Author {name: "Bob Smith"}) INSERT (p4)-[:WROTE]->(a2);
MATCH (p4:Paper {title: "Federated Learning: Privacy-Preserving ML"}), (a3:Author {name: "Carol Davis"}) INSERT (p4)-[:WROTE]->(a3);

MATCH (p5:Paper {title: "Quantum Machine Learning Algorithms"}), (a4:Author {name: "David Wilson"}) INSERT (p5)-[:WROTE]->(a4);
```

### Step 5: Basic Fuzzy Search

Fuzzy search helps find documents despite typos or misspellings:

```gql
-- Find papers about "neural networks" with typo tolerance
MATCH (p:Paper)
WHERE fuzzy_search(p.abstract, 'nural network') > 0.5
RETURN p.title, 
       fuzzy_search(p.abstract, 'nural network') AS fuzzy_score
ORDER BY fuzzy_score DESC;

-- Results will include papers with "neural networks" despite the misspelling
```

#### Output

```bash
| title                                          | fuzzy_score |
|------------------------------------------------|-------------|
| Graph Neural Networks for Molecular Property...| 0.92.....   |
```

### Step 6: Fuzzy Search with Options

Control fuzzy search behavior with different options:

```gql
-- Strict fuzzy matching (small edit distance)
MATCH (p:Paper)
WHERE FUZZY_MATCH(p.abstract, 'mashine lurning', 1)
RETURN p.title;
```

#### Output (6a)

```bash
No results found
```

```gql
-- Lenient fuzzy matching (larger edit distance for more typos)
MATCH (p:Paper)
WHERE FUZZY_MATCH(p.abstract, 'artifical inteligence', 3)
RETURN p.title;
```

#### Output (6b)

```bash
No results found
```

```gql
-- Fuzzy substring matching (word contains fuzzy match)
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'lurning', 2)
RETURN p.title;
```

#### Output (6c)

```bash
┌─────────────────────────────────────────────┐
│ p.title                                     │
╞═════════════════════════════════════════════╡
│ Machine Learning for Healthcare Diagnostics │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Federated Learning: Privacy-Preserving ML   │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Quantum Machine Learning Algorithms         │
└─────────────────────────────────────────────┘
```

```gql
-- Multiple terms with AND logic (requires both terms with fuzzy matching)
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'mashine', 1) 
  AND CONTAINS_FUZZY(p.abstract, 'lurning', 1)
RETURN p.title;
```

#### Output (6d)

```bash
No results found
```

```gql
-- Similarity scoring with threshold
MATCH (p:Paper)
WHERE SIMILARITY_SCORE(p.abstract, 'artifical inteligence') > 0.6
RETURN p.title,
       SIMILARITY_SCORE(p.abstract, 'artifical inteligence') AS similarity
ORDER BY similarity DESC;
```

#### Output (6e)

```bash
No results found
```

### Step 7: Proximity with Fuzzy Search

Combine proximity and fuzzy search for flexible matching:

```gql
-- Find documents containing "mashine" and "lurning" with fuzzy matching
-- (No direct proximity operator - use CONTAINS_FUZZY for each term)
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'mashine', 2) 
  AND CONTAINS_FUZZY(p.abstract, 'lurning', 2)
RETURN p.title;
```

#### Output (7a)

```bash
┌─────────────────────────────────────────────┐
│ p.title                                     │
╞═════════════════════════════════════════════╡
│ Machine Learning for Healthcare Diagnostics │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Quantum Machine Learning Algorithms         │
└─────────────────────────────────────────────┘
```

```gql
-- Complex query: fuzzy OR logic for multiple terms
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'neural', 1) 
  OR CONTAINS_FUZZY(p.abstract, 'network', 1)
  OR CONTAINS_FUZZY(p.abstract, 'deep', 1)
RETURN p.title;
```

#### Output (7b)

```bash
┌─────────────────────────────────────────────────────────┐
│ p.title                                                 │
╞═════════════════════════════════════════════════════════╡
│ Graph Neural Networks for Molecular Property Prediction │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Machine Learning for Healthcare Diagnostics             │
└─────────────────────────────────────────────────────────┘
```

```gql
-- Multi-term fuzzy search with scoring
MATCH (p:Paper)
WHERE SIMILARITY_SCORE(p.abstract, 'neural network deep learning') > 0.4
RETURN p.title,
       SIMILARITY_SCORE(p.abstract, 'neural network deep learning') AS score
ORDER BY score DESC;
```

#### Output (7c)

```bash
No results found
```

```gql
-- Finding papers with specific technical terms (fuzzy AND logic)
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'graph', 1)
  AND CONTAINS_FUZZY(p.abstract, 'neural', 1)
  AND CONTAINS_FUZZY(p.abstract, 'network', 1)
RETURN p.title;
```

#### Output (7d)

```bash
┌─────────────────────────────────────────────────────────┐
│ p.title                                                 │
╞═════════════════════════════════════════════════════════╡
│ Graph Neural Networks for Molecular Property Prediction │
└─────────────────────────────────────────────────────────┘
```

```gql
-- Hybrid search with multiple fuzzy terms
MATCH (p:Paper)
WHERE HYBRID_SEARCH(p.abstract, 'neural networks deep learning') > 0.3
RETURN p.title,
       HYBRID_SEARCH(p.abstract, 'neural networks deep learning') AS hybrid_score,
       SIMILARITY_SCORE(p.abstract, 'neural networks') AS similarity_score
ORDER BY hybrid_score DESC;
```

#### Output (7e)

```bash
No results found
```

### Step 8: Hybrid Search (Text + Vector)

Combine fuzzy text search with semantic vector search:

```gql
-- Define a query embedding (in practice, this comes from an embedding model)
-- Note: You'll need to generate or obtain actual embeddings
LET query_embedding = [0.15, 0.25, 0.35, 0.45, 0.55, 0.65, 0.75, 0.85];

-- Basic hybrid search: fuzzy text match AND vector similarity
-- (Note: Based on tests, WEIGHTED_SEARCH handles text matching internally)
MATCH (p:Paper)
WHERE WEIGHTED_SEARCH(p.abstract, 'helthcare AI', 0.6, 0.2, 0.2) > 0.5
RETURN p.title,
       WEIGHTED_SEARCH(p.abstract, 'helthcare AI', 0.6, 0.2, 0.2) AS hybrid_score
ORDER BY hybrid_score DESC
LIMIT 10;
```

#### Output (8a)

```bash
┌────────────────────────────────────────────────────────┐
│ query_embedding                                        │
╞════════════════════════════════════════════════════════╡
│ VECTOR[0.15, 0.25, 0.35, 0.45, 0.55, 0.65, 0.75, 0.85] │
└────────────────────────────────────────────────────────┘
No results found
```

```gql
-- Using HYBRID_SEARCH (simpler interface, automatic weighting)
MATCH (p:Paper)
WHERE HYBRID_SEARCH(p.abstract, 'helthcare AI') > 0.4
RETURN p.title,
       HYBRID_SEARCH(p.abstract, 'helthcare AI') AS score
ORDER BY score DESC;
```

#### Output (8c)

```bash
No results found
```

```gql
-- Weighted search with different weight configurations
MATCH (p:Paper)
RETURN p.title,
       -- Heavy text weight (70% exact, 20% fuzzy, 10% similarity)
       WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.7, 0.2, 0.1) AS text_focused,
       -- Balanced weights
       WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.33, 0.33, 0.34) AS balanced,
       -- Semantic-focused (lower text weights)
       WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.2, 0.2, 0.6) AS semantic_focused
ORDER BY balanced DESC;
```

#### Output (8d)

```bash
┌─────────────────────────────────────────────────────────┐
│ p.title                                                 │
╞═════════════════════════════════════════════════════════╡
│ Graph Neural Networks for Molecular Property Prediction │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Attention Mechanisms in Transformer Architectures       │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Machine Learning for Healthcare Diagnostics             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Federated Learning: Privacy-Preserving ML               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Quantum Machine Learning Algorithms                     │
└─────────────────────────────────────────────────────────┘
```

```gql
-- Practical example with real queries
MATCH (p:Paper)
WHERE HYBRID_SEARCH(p.abstract, 'neural networks drug discovery') > 0.3
RETURN p.title,
       p.abstract,
       HYBRID_SEARCH(p.abstract, 'neural networks drug discovery') AS relevance
ORDER BY relevance DESC
LIMIT 5;
```

#### Output (8e)

```bash
No results found
```

### Step 9: Advanced Hybrid Search with Graph Traversal

Find authors who write about topics similar to a query:

```gql
-- Find authors who write about topics similar to a query
MATCH (author:Author)-[:WROTE]->(paper:Paper)
WHERE HYBRID_SEARCH(paper.abstract, 'quantom computting and machine lurning') > 0.3
RETURN author.name,
       author.affiliation,
       COUNT(paper) AS relevant_papers,
       AVG(HYBRID_SEARCH(paper.abstract, 'quantom computting and machine lurning')) AS avg_score
GROUP BY author.name, author.affiliation
ORDER BY avg_score DESC, relevant_papers DESC
LIMIT 20;
```

#### Output (9a)

```bash
No results found
```

```gql
-- Find experts in specific domains
MATCH (author:Author)-[:WROTE]->(paper:Paper)
WHERE CONTAINS_FUZZY(paper.abstract, 'neural', 1)
  AND CONTAINS_FUZZY(paper.abstract, 'network', 1)
RETURN author.name,
       author.affiliation,
       COUNT(DISTINCT paper) AS neural_network_papers
GROUP BY author.name, author.affiliation
ORDER BY neural_network_papers DESC
LIMIT 15;
```

#### Output (9b)

```bash
No results found
```

```gql
-- Find collaboration networks
MATCH (a1:Author)-[:WROTE]->(p:Paper)<-[:WROTE]-(a2:Author)
WHERE a1.name < a2.name
  AND HYBRID_SEARCH(p.abstract, 'graph neural networks') > 0.35
RETURN a1.name AS author1,
       a2.name AS author2,
       p.title AS collaborative_paper,
       HYBRID_SEARCH(p.abstract, 'graph neural networks') AS relevance
ORDER BY relevance DESC;
```

#### Output (9c)

```bash
No results found
```

```gql
-- Author expertise profiling
MATCH (author:Author)-[:WROTE]->(paper:Paper)
RETURN author.name,
       author.affiliation,
       COUNT(CASE WHEN CONTAINS_FUZZY(paper.abstract, 'machine', 1) THEN 1 END) AS machine_learning_papers,
       COUNT(CASE WHEN CONTAINS_FUZZY(paper.abstract, 'quantum', 1) THEN 1 END) AS quantum_papers,
       COUNT(CASE WHEN CONTAINS_FUZZY(paper.abstract, 'neural', 1) THEN 1 END) AS neural_papers,
       COUNT(paper) AS total_papers
GROUP BY author.name, author.affiliation
ORDER BY total_papers DESC;
```

#### Output (9d)

```bash
No results found
```

### Step 10: Boolean Operations with Fuzzy Search

Combine fuzzy search with boolean logic:

```gql
-- Fuzzy AND logic (must contain both terms with fuzzy matching)
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'mashine', 1)
  AND CONTAINS_FUZZY(p.abstract, 'lurning', 1)
RETURN p.title;
```

#### Output (10a)

```bash
No results found
```

```gql
-- Fuzzy OR logic (contains either term with fuzzy matching)
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'neural', 1)
  OR CONTAINS_FUZZY(p.abstract, 'quantom', 1)
RETURN p.title;
```

#### Output (10b)

```bash
┌─────────────────────────────────────────────────────────┐
│ p.title                                                 │
╞═════════════════════════════════════════════════════════╡
│ Graph Neural Networks for Molecular Property Prediction │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Quantum Machine Learning Algorithms                     │
└─────────────────────────────────────────────────────────┘
```

```gql
-- Complex boolean combinations
MATCH (p:Paper)
WHERE (CONTAINS_FUZZY(p.abstract, 'graph', 1) AND CONTAINS_FUZZY(p.abstract, 'network', 1))
  OR (CONTAINS_FUZZY(p.abstract, 'deep', 1) AND CONTAINS_FUZZY(p.abstract, 'learning', 1))
RETURN p.title;
```

#### Output (10c)

```bash
┌─────────────────────────────────────────────────────────┐
│ p.title                                                 │
╞═════════════════════════════════════════════════════════╡
│ Graph Neural Networks for Molecular Property Prediction │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Machine Learning for Healthcare Diagnostics             │
└─────────────────────────────────────────────────────────┘
```

```gql
-- NOT logic (exclusion)
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'mashine', 1)
  AND NOT CONTAINS_FUZZY(p.abstract, 'statistics', 1)
RETURN p.title;
-- Finds papers about "machine" (with typos) but NOT about "statistics"
```

#### Output (10d)

```bash
┌─────────────────────────────────────────────────────────┐
│ p.title                                                 │
╞═════════════════════════════════════════════════════════╡
│ Graph Neural Networks for Molecular Property Prediction │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Quantum Machine Learning Algorithms                     │
└─────────────────────────────────────────────────────────┘
```

```gql
-- Multiple exclusion criteria
MATCH (p:Paper)
WHERE HYBRID_SEARCH(p.abstract, 'learning algorithms') > 0.4
  AND NOT CONTAINS_FUZZY(p.abstract, 'quantum', 1)
  AND NOT CONTAINS_FUZZY(p.abstract, 'biology', 1)
RETURN p.title,
       HYBRID_SEARCH(p.abstract, 'learning algorithms') AS score
ORDER BY score DESC;
```

#### Output (10e)

```bash
No results found
```

```gql
-- Parentheses for complex logic grouping
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'neural', 1)
  AND (CONTAINS_FUZZY(p.abstract, 'network', 1) OR CONTAINS_FUZZY(p.abstract, 'networks', 1))
  AND NOT CONTAINS_FUZZY(p.abstract, 'convolutional', 1)
RETURN p.title;
```

#### Output (10f)

```bash
┌─────────────────────────────────────────────────────────┐
│ p.title                                                 │
╞═════════════════════════════════════════════════════════╡
│ Graph Neural Networks for Molecular Property Prediction │
└─────────────────────────────────────────────────────────┘
```

```gql
-- Combining exact and fuzzy matching
MATCH (p:Paper)
WHERE p.title CONTAINS 'Learning'
  AND CONTAINS_FUZZY(p.abstract, 'mashine', 2)
RETURN p.title;
```

#### Output (10g)

```bash
┌─────────────────────────────────────────────┐
│ p.title                                     │
╞═════════════════════════════════════════════╡
│ Machine Learning for Healthcare Diagnostics │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Quantum Machine Learning Algorithms         │
└─────────────────────────────────────────────┘
```

```gql
-- KEYWORD_MATCH: OR logic (matches any of the keywords)
MATCH (p:Paper)
WHERE KEYWORD_MATCH(p.abstract, 'Python', 'JavaScript', 'Java')
RETURN p.title;
```

#### Output (10h)

```bash
No results found
```

```gql
-- KEYWORD_MATCH_ALL: AND logic (matches all keywords)
MATCH (p:Paper)
WHERE KEYWORD_MATCH_ALL(p.abstract, 'Machine', 'Learning', 'Deep')
RETURN p.title;
```

#### Output (10i)

```bash
┌─────────────────────────────────────────────┐
│ p.title                                     │
╞═════════════════════════════════════════════╡
│ Machine Learning for Healthcare Diagnostics │
└─────────────────────────────────────────────┘
```

### Step 11: Performance-optimized Hybrid Search

Use score thresholds and early filtering for better performance:

```gql
MATCH (p:Paper)
WHERE WEIGHTED_SEARCH(p.abstract, 'federated lurning privasy', 0.7, 0.2, 0.1) > 0.6
RETURN p.title,
       WEIGHTED_SEARCH(p.abstract, 'federated lurning privasy', 0.7, 0.2, 0.1) AS final_score
ORDER BY final_score DESC
LIMIT 15;
```

#### Output (11a)

```bash
No results found
```

```gql
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'federated', 2)
  AND CONTAINS_FUZZY(p.abstract, 'lurning', 2)
  AND CONTAINS_FUZZY(p.abstract, 'privasy', 2)
RETURN p.title,
       WEIGHTED_SEARCH(p.abstract, 'federated learning privacy', 0.7, 0.2, 0.1) AS score
ORDER BY score DESC
LIMIT 15;
```

#### Output (11b)

```bash
No results found
```

## Best Practices for Fuzzy and Hybrid Search

### DO

#### 1. Use appropriate max distance

```gql
-- Short words: smaller distance
WHERE fuzzy_search(name, 'Jhon', {max_distance: 1}) > 0.5

-- Long words/queries: larger distance
WHERE fuzzy_search(abstract, 'artifical inteligence', {max_distance: 3}) > 0.4
```

#### 2. Combine fuzzy with other operators for precision

```gql
WHERE fuzzy_search(content, '"mashine lurning"~5', {
    max_distance: 2,
    require_all: true
}) > 0.6
```

#### 3. Tune hybrid weights based on your domain

```gql
-- Technical papers: weight text higher
(text_score * 0.7 + vector_score * 0.3)

-- Semantic similarity: weight vector higher
(text_score * 0.4 + vector_score * 0.6)
```

#### 4. Use thresholds to improve performance

```gql
WHERE fuzzy_search(content, query, {max_distance: 2}) > 0.5 
  AND vector_similarity(embedding, query_embedding) > 0.7
```

## Troubleshooting Fuzzy and Hybrid Search

### Problem: Fuzzy search returns too many irrelevant results

Solution: Increase score threshold and adjust max distance

```gql
-- Higher precision
WHERE fuzzy_search(content, 'query', {
    max_distance: 1,      -- Stricter matching
    require_all: true     -- All terms must match
}) > 0.7                  -- Higher score threshold
```

### Problem: Hybrid search is slow

Solution: Add early filtering and check indexes

```gql
-- Faster with thresholds
WHERE fuzzy_search(content, $query, {max_distance: 2}) > 0.6  -- Filter first
  AND vector_similarity(embedding, $query_embedding) > 0.7

-- Verify indexes exist
CALL gql.show_indexes();
```

### Problem: Vector similarity doesn't improve results

Solution: Adjust hybrid weights and ensure embeddings are trained on relevant data

```gql
-- Experiment with different weights
LET hybrid_score = (text_score * $text_weight + vector_score * $vector_weight)

-- Start with equal weights, then adjust based on validation
-- For technical text: $text_weight = 0.7, $vector_weight = 0.3
-- For semantic search: $text_weight = 0.3, $vector_weight = 0.7
```
