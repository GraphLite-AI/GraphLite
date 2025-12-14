# Getting Started With Fulltext

A step-by-step tutorial for learning GraphLite's text search capabilities.

**Related Documentation:**
- [Full Text Search Guide New.md](Full%20Text%20Search%20Guide%20New.md) - Comprehensive function reference
- [Fuzzy Search Functions Reference.md](Fuzzy%20Search%20Functions%20Reference.md) - Technical deep dive into algorithms

## Overview

GraphLite provides powerful **fuzzy search** for typo-tolerant matching and **hybrid search** that combines multiple text-matching strategies (exact substring matching, fuzzy substring matching, and overall text similarity). This tutorial demonstrates a complete workflow from schema creation to complex queries.

---

## Quick Start: Complete Workflow

### Step 1: Create Graph Schema

First, let's create a schema with nodes that have text properties for search:

General setup:

-- Build the project

```bash
./scripts/build_all.sh
./target/release/graphlite gql --path ./test_db -u admin -p secret
```

-- Set up database

```gql
CREATE SCHEMA papers_schema;
SESSION SET SCHEMA papers_schema;

CREATE GRAPH papers_graph;
SESSION SET GRAPH papers_graph;
```

### Step 2: Insert Data

```gql
-- Insert sample papers
INSERT (:Paper {
  title: "Graph Neural Networks for Molecular Property Prediction",
  abstract: "Graph neural networks (GNNs) have revolutionized molecular property prediction in drug discovery. This paper introduces a novel GNN architecture that achieves state-of-the-art results on benchmark datasets."
}),
(:Paper {
  title: "Attention Mechanisms in Transformer Architectures",
  abstract: "Attention mechanisms form the core of transformer models used in natural language processing. We analyze different attention variants and their impact on model performance."
}),
(:Paper {
  title: "Machine Learning for Healthcare Diagnostics",
  abstract: "Machine learning techniques are transforming healthcare diagnostics. Our study applies deep learning to medical imaging with promising results for early disease detection."
}),
(:Paper {
  title: "Federated Learning: Privacy-Preserving ML",
  abstract: "Federated learning enables collaborative model training without sharing raw data. This paper proposes a novel aggregation method that improves convergence rates."
}),
(:Paper {
  title: "Quantum Machine Learning Algorithms",
  abstract: "Quantum computing offers new paradigms for machine learning. We introduce quantum variants of classical algorithms and demonstrate speedups on specific problem classes."
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
## Add one line why should this query doesn't return results?
**Reason**: The phrase "mashine lurning" requires 2 edits ("mashine"→"machine" and "lurning"→"learning"), but FUZZY_MATCH checks the entire phrase which needs 4+ edits total to match any abstract.

#### Output (6a)

```bash
No results found
```

**Working alternative**: Match individual words instead of the full phrase:
```gql
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'mashine', 1)
  AND CONTAINS_FUZZY(p.abstract, 'lurning', 2)
RETURN p.title;
-- Returns: Machine Learning for Healthcare Diagnostics, Quantum Machine Learning Algorithms
```

```gql
-- Lenient fuzzy matching (larger edit distance for more typos)
MATCH (p:Paper)
WHERE FUZZY_MATCH(p.abstract, 'artifical inteligence', 3)
RETURN p.title;
```
## Add one line why should this query doesn't return results?
**Reason**: None of the paper abstracts contain the words "artificial" or "intelligence" (or similar). The dataset focuses on "machine learning", "neural networks", "quantum computing", etc.

#### Output (6b)

```bash
No results found
```

**Working alternative**: Search for terms that actually exist in the dataset:
```gql
MATCH (p:Paper)
WHERE FUZZY_MATCH(p.abstract, 'machine learning', 2)
RETURN p.title;
-- Returns papers containing "machine learning"
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
## Add one line why should this query doesn't return results?
**Reason**: "lurning" needs 2 edits to become "learning" (substitute u→e, add a), but max_distance is only 1, so no substring matches.

#### Output (6d)

```bash
No results found
```

**Working alternative**: Increase max_distance to 2 for "lurning":
```gql
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'mashine', 1)
  AND CONTAINS_FUZZY(p.abstract, 'lurning', 2)
RETURN p.title;
-- Returns: Machine Learning for Healthcare Diagnostics, Quantum Machine Learning Algorithms
```

```gql
-- Levenshtein similarity scoring with threshold
MATCH (p:Paper)
WHERE LEVENSHTEIN_SIMILARITY(p.abstract, 'artifical inteligence') > 0.6
RETURN p.title,
       LEVENSHTEIN_SIMILARITY(p.abstract, 'artifical inteligence') AS similarity
ORDER BY similarity DESC;
```
## Add one line why should this query doesn't return results?
**Reason**: The abstracts don't contain "artificial intelligence" or similar terms, so similarity scores are very low (<0.2), all below the 0.6 threshold.

**Note**: LEVENSHTEIN_SIMILARITY uses formula `1.0 - (edit_distance / max_length)` and works best when comparing strings of similar length.

#### Output (6e)

```bash
No results found
```

**Working alternative**: Use terms that exist in the dataset:
```gql
MATCH (p:Paper)
WHERE LEVENSHTEIN_SIMILARITY(p.abstract, 'machine learning') > 0.6
RETURN p.title,
       LEVENSHTEIN_SIMILARITY(p.abstract, 'machine learning') AS similarity
ORDER BY similarity DESC;
-- Returns papers with high similarity to "machine learning"
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
-- Multi-term Levenshtein similarity search
MATCH (p:Paper)
WHERE LEVENSHTEIN_SIMILARITY(p.abstract, 'neural network deep learning') > 0.4
RETURN p.title,
       LEVENSHTEIN_SIMILARITY(p.abstract, 'neural network deep learning') AS score
ORDER BY score DESC;
```
## Add one line why should this query doesn't return results?
**Reason**: LEVENSHTEIN_SIMILARITY compares the entire query string against the abstract; the long phrase has low overall similarity (< 0.4) even though individual words match. Works best for strings of similar length.

#### Output (7c)

```bash
No results found
```

**Working alternative**: Use FUZZY_SEARCH which finds best substring matches:
```gql
MATCH (p:Paper)
WHERE FUZZY_SEARCH(p.abstract, 'neural network') > 0.7
   OR FUZZY_SEARCH(p.abstract, 'deep learning') > 0.7
RETURN p.title,
       FUZZY_SEARCH(p.abstract, 'neural network') AS nn_score,
       FUZZY_SEARCH(p.abstract, 'deep learning') AS dl_score
ORDER BY nn_score DESC, dl_score DESC;
-- Returns papers with strong matches for either phrase
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
       LEVENSHTEIN_SIMILARITY(p.abstract, 'neural networks') AS levenshtein_score
ORDER BY hybrid_score DESC;
```
## Add one line why should this query doesn't return results?
**Reason**: The long query phrase gets low scores on all three components (exact=0, fuzzy<0.3, similarity<0.3), resulting in combined score < 0.3 threshold.

#### Output (7e)

```bash
No results found
```

**Working alternative**: Use shorter, focused queries that match the data:
```gql
MATCH (p:Paper)
WHERE HYBRID_SEARCH(p.abstract, 'neural networks') > 0.3
RETURN p.title,
       HYBRID_SEARCH(p.abstract, 'neural networks') AS hybrid_score
ORDER BY hybrid_score DESC;
-- Returns: Graph Neural Networks for Molecular Property Prediction
```

### Step 8: Multi-Strategy Hybrid Search

Use HYBRID_SEARCH and WEIGHTED_SEARCH to combine multiple text-matching strategies with configurable weights.

**What HYBRID_SEARCH Does**:
HYBRID_SEARCH combines three text-based strategies:
1. **Exact substring match** (1.0 if query appears as substring, 0.0 otherwise)
2. **Fuzzy substring match** (best sliding window match using Levenshtein distance)
3. **Overall text similarity** (Levenshtein-based similarity between entire strings)

Default weights: 40% exact, 40% fuzzy, 20% similarity

```gql
-- Basic hybrid search with default weights
MATCH (p:Paper)
WHERE HYBRID_SEARCH(p.abstract, 'healthcare diagnostics') > 0.4
RETURN p.title,
       HYBRID_SEARCH(p.abstract, 'healthcare diagnostics') AS hybrid_score
ORDER BY hybrid_score DESC
LIMIT 10;
```

#### Output (8a)

```bash
┌─────────────────────────────────────────────┐
│ p.title                                     │
╞═════════════════════════════════════════════╡
│ Machine Learning for Healthcare Diagnostics │
└─────────────────────────────────────────────┘
```

```gql
-- Custom weight configuration with WEIGHTED_SEARCH
-- Prioritize exact matches (70%), fuzzy matches (20%), overall similarity (10%)
MATCH (p:Paper)
WHERE WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.7, 0.2, 0.1) > 0.5
RETURN p.title,
       WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.7, 0.2, 0.1) AS score
ORDER BY score DESC;
```

#### Output (8b)

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
-- Compare different weight configurations
MATCH (p:Paper)
RETURN p.title,
       -- Exact-match focused (70% exact, 20% fuzzy, 10% similarity)
       WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.7, 0.2, 0.1) AS exact_focused,
       -- Balanced weights (equal distribution)
       WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.33, 0.33, 0.34) AS balanced,
       -- Similarity-focused (emphasize overall text similarity)
       WEIGHTED_SEARCH(p.abstract, 'machine learning', 0.2, 0.2, 0.6) AS similarity_focused
ORDER BY balanced DESC;
```

#### Output (8c)

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
-- Multi-term search with hybrid scoring
MATCH (p:Paper)
WHERE HYBRID_SEARCH(p.abstract, 'neural networks') > 0.3
   OR HYBRID_SEARCH(p.abstract, 'drug discovery') > 0.3
RETURN p.title,
       HYBRID_SEARCH(p.abstract, 'neural networks') AS nn_score,
       HYBRID_SEARCH(p.abstract, 'drug discovery') AS dd_score
ORDER BY nn_score DESC, dd_score DESC;
```

#### Output (8d)

```bash
┌─────────────────────────────────────────────────────────┐
│ p.title                                                 │
╞═════════════════════════════════════════════════════════╡
│ Graph Neural Networks for Molecular Property Prediction │
└─────────────────────────────────────────────────────────┘
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
## Add one line why should this query doesn't return results?
**Reason**: The query has 5 typos in a 6-word phrase ("quantom computting and machine lurning"); the overall similarity is too low even for papers about quantum and machine learning.

#### Output (9a)

```bash
No results found
```

**Working alternative**: Fix typos or use correct terms:
```gql
MATCH (author:Author)-[:WROTE]->(paper:Paper)
WHERE HYBRID_SEARCH(paper.abstract, 'quantum machine learning') > 0.3
RETURN author.name,
       author.affiliation,
       COUNT(paper) AS relevant_papers,
       AVG(HYBRID_SEARCH(paper.abstract, 'quantum machine learning')) AS avg_score
GROUP BY author.name, author.affiliation
ORDER BY avg_score DESC, relevant_papers DESC;
-- Returns: David Wilson (wrote Quantum Machine Learning Algorithms)
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
## Add one line why should this query doesn't return results?
**Reason**: The WHERE clause filters papers first, but only 1 paper matches (Graph Neural Networks...), and its authors weren't linked in Step 3 relationships.

```bash
No results found
```

**Working alternative**: The paper exists but check if WROTE relationships were created:
```gql
-- First verify the paper exists
MATCH (paper:Paper)
WHERE CONTAINS_FUZZY(paper.abstract, 'neural', 1)
  AND CONTAINS_FUZZY(paper.abstract, 'network', 1)
RETURN paper.title;
-- Should return: Graph Neural Networks for Molecular Property Prediction

-- Then check for authors (relationships exist per Step 3)
MATCH (author:Author)-[:WROTE]->(paper:Paper {title: "Graph Neural Networks for Molecular Property Prediction"})
RETURN author.name;
-- Returns: Alice Chen, Bob Smith
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
## Add one line why should this query doesn't return results?
**Reason**: The phrase "graph neural networks" scores low on HYBRID_SEARCH (< 0.35) because it's a 3-word phrase with partial matches in the abstract.

```bash
No results found
```

**Working alternative**: Lower threshold or search for 2-word phrases:
```gql
MATCH (a1:Author)-[:WROTE]->(p:Paper)<-[:WROTE]-(a2:Author)
WHERE a1.name < a2.name
  AND HYBRID_SEARCH(p.abstract, 'neural networks') > 0.3
RETURN a1.name AS author1,
       a2.name AS author2,
       p.title AS collaborative_paper,
       HYBRID_SEARCH(p.abstract, 'neural networks') AS relevance
ORDER BY relevance DESC;
-- Returns: Alice Chen & Bob Smith collaborated on Graph Neural Networks paper
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
## Add one line why should this query doesn't return results?
**Reason**: This query should actually return results if the WROTE relationships exist. Likely the relationships weren't persisted or session state was lost.

```bash
No results found
```

**Working alternative**: Verify relationships exist first:
```gql
-- Check if relationships exist
MATCH (author:Author)-[:WROTE]->(paper:Paper)
RETURN author.name, paper.title
LIMIT 5;
-- If this returns results, the original query should work
-- If not, recreate relationships from Step 3
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
## Add one line why should this query doesn't return results?
**Reason**: Same as query 6d - "lurning" needs 2 edits to match "learning", but max_distance is only 1.

```bash
No results found
```

**Working alternative**: Increase edit distance for "lurning":
```gql
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'mashine', 1)
  AND CONTAINS_FUZZY(p.abstract, 'lurning', 2)
RETURN p.title;
-- Returns: Machine Learning for Healthcare Diagnostics, Quantum Machine Learning Algorithms
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
┌─────────────────────────────────────────────┐
│ p.title                                     │
╞═════════════════════════════════════════════╡
│ Machine Learning for Healthcare Diagnostics │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Quantum Machine Learning Algorithms         │
└─────────────────────────────────────────────┘
```

**Explanation**: Both papers contain "machine" (within 1 edit of "mashine") and neither contains "statistics". The "Graph Neural Networks" paper does NOT contain "machine", so it correctly does not appear in results.

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

## Add one line why should this query doesn't return results?
**Reason**: The phrase "learning algorithms" scores < 0.4 on HYBRID_SEARCH even in papers about learning, because it's a 2-word phrase with partial/fuzzy matches.

#### Output (10e)

```bash
No results found
```

**Working alternative**: Use single words or lower the threshold:
```gql
MATCH (p:Paper)
WHERE FUZZY_SEARCH(p.abstract, 'learning') > 0.7
  AND NOT CONTAINS_FUZZY(p.abstract, 'quantum', 1)
RETURN p.title,
       FUZZY_SEARCH(p.abstract, 'learning') AS score
ORDER BY score DESC;
-- Returns: Machine Learning for Healthcare Diagnostics, Federated Learning papers
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
## Add one line why should this query doesn't return results?
**Reason**: None of the paper abstracts mention programming languages (Python, JavaScript, Java); they focus on ML/AI concepts, not implementation languages.

#### Output (10h)

```bash
No results found
```

**Working alternative**: Search for keywords that exist in the dataset:
```gql
MATCH (p:Paper)
WHERE KEYWORD_MATCH(p.abstract, 'learning', 'neural', 'quantum', 'attention')
RETURN p.title;
-- Returns all papers (each contains at least one of these terms)
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
## Add one line why should this query doesn't return results?
**Reason**: Multiple typos ("lurning" needs 2 edits, "privasy" needs 1 edit), and high exact weight (0.7) penalizes typo-laden queries; combined score < 0.6.

#### Output (11a)

```bash
No results found
```

**Working alternative**: Fix typos or adjust weights to favor fuzzy matching:
```gql
MATCH (p:Paper)
WHERE WEIGHTED_SEARCH(p.abstract, 'federated learning privacy', 0.7, 0.2, 0.1) > 0.4
RETURN p.title,
       WEIGHTED_SEARCH(p.abstract, 'federated learning privacy', 0.7, 0.2, 0.1) AS final_score
ORDER BY final_score DESC;
-- Returns: Federated Learning: Privacy-Preserving ML
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
## Add one line why should this query doesn't return results?
**Reason**: The abstract contains "federated" and "learning" but NOT "privacy" (says "Privacy-Preserving" in title only); the WHERE clause requires all three terms.

#### Output (11b)

```bash
No results found
```

**Working alternative**: Remove the privacy requirement or search both title and abstract:
```gql
MATCH (p:Paper)
WHERE CONTAINS_FUZZY(p.abstract, 'federated', 2)
  AND CONTAINS_FUZZY(p.abstract, 'lurning', 2)
RETURN p.title,
       WEIGHTED_SEARCH(p.abstract, 'federated learning', 0.7, 0.2, 0.1) AS score
ORDER BY score DESC;
-- Returns: Federated Learning: Privacy-Preserving ML

-- Or search title as well:
MATCH (p:Paper)
WHERE (CONTAINS_FUZZY(p.abstract, 'federated', 2) OR CONTAINS_FUZZY(p.title, 'federated', 2))
  AND (CONTAINS_FUZZY(p.abstract, 'learning', 2) OR CONTAINS_FUZZY(p.title, 'learning', 2))
  AND (CONTAINS_FUZZY(p.abstract, 'privacy', 2) OR CONTAINS_FUZZY(p.title, 'privacy', 2))
RETURN p.title;
-- Returns: Federated Learning: Privacy-Preserving ML
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
-- For precise matching: prioritize exact matches
WEIGHTED_SEARCH(text, query, 0.7, 0.2, 0.1)  -- 70% exact, 20% fuzzy, 10% similarity

-- For flexible matching: emphasize fuzzy and similarity
WEIGHTED_SEARCH(text, query, 0.2, 0.5, 0.3)  -- 20% exact, 50% fuzzy, 30% similarity
```

#### 4. Use thresholds to improve performance

```gql
-- Pre-filter with fast exact checks before expensive fuzzy operations
WHERE content CONTAINS 'keyword'
  AND FUZZY_SEARCH(content, query) > 0.7
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
-- Use higher thresholds and pre-filtering
WHERE content CONTAINS 'keyword'  -- Fast pre-filter
  AND FUZZY_SEARCH(content, $query) > 0.6  -- Then fuzzy search

-- Use LIMIT to stop early
MATCH (d:Document)
WHERE FUZZY_SEARCH(d.content, $query) > 0.6
RETURN d.title
ORDER BY FUZZY_SEARCH(d.content, $query) DESC
LIMIT 100;  -- Stop after finding top 100
```

### Problem: Search results not relevant enough

Solution: Adjust hybrid weights and scoring thresholds based on your data

```gql
-- Experiment with different weight configurations
MATCH (d:Document)
RETURN d.title,
       WEIGHTED_SEARCH(d.content, $query, 0.7, 0.2, 0.1) AS exact_focused,
       WEIGHTED_SEARCH(d.content, $query, 0.3, 0.5, 0.2) AS fuzzy_focused,
       WEIGHTED_SEARCH(d.content, $query, 0.2, 0.3, 0.5) AS similarity_focused
ORDER BY exact_focused DESC
LIMIT 10;

-- Start with balanced weights, then adjust based on your use case:
-- For exact term matching: 0.7, 0.2, 0.1 (prioritize exact matches)
-- For typo tolerance: 0.2, 0.6, 0.2 (prioritize fuzzy matching)
-- For conceptual similarity: 0.2, 0.2, 0.6 (prioritize overall similarity)
```
