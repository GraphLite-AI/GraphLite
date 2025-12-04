// N-gram index for fuzzy and approximate string matching

use std::collections::{HashMap, HashSet};

/// Fuzzy match result
#[derive(Debug, Clone)]
pub struct FuzzyMatchResult {
    /// Document ID
    pub doc_id: u64,
    /// Similarity score (0.0 to 1.0)
    pub similarity: f64,
}

/// N-gram index for fuzzy string matching
#[derive(Debug, Clone)]
pub struct NGramIndex {
    /// N value (typically 3 for trigrams)
    n: usize,
    /// Mapping from n-gram to document IDs
    ngram_to_docs: HashMap<String, HashSet<u64>>,
    /// Mapping from document ID to original text
    doc_to_text: HashMap<u64, String>,
}

impl NGramIndex {
    /// Create a new N-gram index (default n=3 for trigrams)
    pub fn new() -> Self {
        Self::with_n(3)
    }

    /// Create an N-gram index with specific n value
    pub fn with_n(n: usize) -> Self {
        assert!(n > 0, "n must be greater than 0");
        Self {
            n,
            ngram_to_docs: HashMap::new(),
            doc_to_text: HashMap::new(),
        }
    }

    /// Add a text to the index
    pub fn add(&mut self, doc_id: u64, text: &str) {
        let normalized = text.to_lowercase();
        self.doc_to_text.insert(doc_id, normalized.clone());

        // Generate n-grams with padding
        let padded = format!(
            "{}{}{}",
            " ".repeat(self.n - 1),
            normalized,
            " ".repeat(self.n - 1)
        );

        for ngram in self.generate_ngrams(&padded) {
            self.ngram_to_docs
                .entry(ngram)
                .or_insert_with(HashSet::new)
                .insert(doc_id);
        }
    }

    /// Fuzzy search with edit distance tolerance
    pub fn fuzzy_search(&self, query: &str, max_distance: usize) -> Vec<FuzzyMatchResult> {
        let query = query.to_lowercase();
        let mut results = Vec::new();

        // Get candidate documents that share n-grams
        let candidates = self.get_candidates(&query);

        for doc_id in candidates {
            if let Some(text) = self.doc_to_text.get(&doc_id) {
                let distance = self.levenshtein_distance(&query, text);
                if distance <= max_distance {
                    let similarity = self.calculate_similarity(&query, text, distance);
                    results.push(FuzzyMatchResult { doc_id, similarity });
                }
            }
        }

        // Sort by similarity descending
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// Fuzzy search with similarity threshold
    pub fn fuzzy_search_threshold(
        &self,
        query: &str,
        threshold: f64,
        max_distance: Option<usize>,
    ) -> Vec<FuzzyMatchResult> {
        let query = query.to_lowercase();
        let mut results = Vec::new();

        let max_distance =
            max_distance.unwrap_or_else(|| (query.len().max(10) as f64 * 0.3).ceil() as usize);

        // Get candidate documents
        let candidates = self.get_candidates(&query);

        for doc_id in candidates {
            if let Some(text) = self.doc_to_text.get(&doc_id) {
                let distance = self.levenshtein_distance(&query, text);
                if distance <= max_distance {
                    let similarity = self.calculate_similarity(&query, text, distance);
                    if similarity >= threshold {
                        results.push(FuzzyMatchResult { doc_id, similarity });
                    }
                }
            }
        }

        // Sort by similarity descending
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// Get candidate documents using n-gram overlap
    fn get_candidates(&self, query: &str) -> HashSet<u64> {
        let padded = format!(
            "{}{}{}",
            " ".repeat(self.n - 1),
            query,
            " ".repeat(self.n - 1)
        );

        let query_ngrams = self.generate_ngrams(&padded);
        let mut candidates = HashSet::new();

        for ngram in query_ngrams {
            if let Some(docs) = self.ngram_to_docs.get(&ngram) {
                candidates.extend(docs.iter());
            }
        }

        candidates
    }

    /// Generate n-grams from text
    fn generate_ngrams(&self, text: &str) -> Vec<String> {
        let mut ngrams = Vec::new();
        let chars: Vec<char> = text.chars().collect();

        if chars.len() < self.n {
            return ngrams;
        }

        for i in 0..=(chars.len() - self.n) {
            let ngram: String = chars[i..i + self.n].iter().collect();
            ngrams.push(ngram);
        }

        ngrams
    }

    /// Calculate Levenshtein distance (edit distance)
    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        // Use character counts (not byte lengths) to handle Unicode correctly
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        let len1 = s1_chars.len();
        let len2 = s2_chars.len();

        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for i in 0..=len1 {
            matrix[i][0] = i;
        }

        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = *[
                    matrix[i - 1][j] + 1,
                    matrix[i][j - 1] + 1,
                    matrix[i - 1][j - 1] + cost,
                ]
                .iter()
                .min()
                .unwrap();
            }
        }

        matrix[len1][len2]
    }

    /// Calculate similarity score (0.0 to 1.0)
    fn calculate_similarity(&self, query: &str, text: &str, distance: usize) -> f64 {
        // Use character counts for length to handle Unicode correctly
        let max_len = query.chars().count().max(text.chars().count());
        if max_len == 0 {
            return 1.0;
        }

        1.0 - (distance as f64 / max_len as f64)
    }

    /// Calculate Jaccard similarity between two strings
    pub fn jaccard_similarity(&self, s1: &str, s2: &str) -> f64 {
        // Compute Jaccard similarity over character sets (unigrams).
        // This gives an intuitive similarity measure for short strings
        // like "hello" vs "hallo".
        let set1: HashSet<char> = s1.to_lowercase().chars().collect();
        let set2: HashSet<char> = s2.to_lowercase().chars().collect();

        let intersection = set1.intersection(&set2).count();
        let union = set1.union(&set2).count();

        if union == 0 {
            return 1.0;
        }
        intersection as f64 / union as f64
    }

    /// Remove a document from the index
    pub fn remove(&mut self, doc_id: u64) {
        self.doc_to_text.remove(&doc_id);

        // Remove from all n-gram maps
        self.ngram_to_docs.retain(|_, docs| {
            docs.remove(&doc_id);
            !docs.is_empty()
        });
    }

    /// Clear all documents
    pub fn clear(&mut self) {
        self.ngram_to_docs.clear();
        self.doc_to_text.clear();
    }

    /// Get document count
    pub fn doc_count(&self) -> usize {
        self.doc_to_text.len()
    }

    /// Get n-gram count
    pub fn ngram_count(&self) -> usize {
        self.ngram_to_docs.len()
    }

    /// Get n value
    pub fn n(&self) -> usize {
        self.n
    }
}

impl Default for NGramIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_index() {
        let index = NGramIndex::new();
        assert_eq!(index.n(), 3);
        assert_eq!(index.doc_count(), 0);
    }

    #[test]
    fn test_custom_n_value() {
        let index = NGramIndex::with_n(4);
        assert_eq!(index.n(), 4);
    }

    #[test]
    fn test_add_document() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");
        assert_eq!(index.doc_count(), 1);
    }

    #[test]
    fn test_fuzzy_search_exact_match() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");

        let results = index.fuzzy_search("hello", 0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, 1);
    }

    #[test]
    fn test_fuzzy_search_one_typo() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");

        let results = index.fuzzy_search("hallo", 1);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_fuzzy_search_two_typos() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");

        let results = index.fuzzy_search("helo", 1);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_fuzzy_search_no_match() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");

        let results = index.fuzzy_search("xyz", 1);
        assert!(results.is_empty());
    }

    #[test]
    fn test_multiple_documents() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");
        index.add(2, "hallo");
        index.add(3, "bye");

        assert_eq!(index.doc_count(), 3);
    }

    #[test]
    fn test_fuzzy_search_best_match() {
        let mut index = NGramIndex::new();
        index.add(1, "John Smith");
        index.add(2, "Jon Smith");
        index.add(3, "Jane Smith");

        let results = index.fuzzy_search("John Smith", 2);
        assert!(!results.is_empty());
        // Best match should be exact match (doc 1)
        assert_eq!(results[0].doc_id, 1);
    }

    #[test]
    fn test_similarity_score() {
        let mut index = NGramIndex::new();
        index.add(1, "test");

        let results = index.fuzzy_search("test", 0);
        assert_eq!(results[0].similarity, 1.0);
    }

    #[test]
    fn test_case_insensitive() {
        let mut index = NGramIndex::new();
        index.add(1, "HELLO");

        let results = index.fuzzy_search("hello", 0);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_remove_document() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");
        index.add(2, "world");

        assert_eq!(index.doc_count(), 2);

        index.remove(1);
        assert_eq!(index.doc_count(), 1);
    }

    #[test]
    fn test_clear_index() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");
        index.add(2, "world");

        index.clear();
        assert_eq!(index.doc_count(), 0);
        assert_eq!(index.ngram_count(), 0);
    }

    #[test]
    fn test_levenshtein_same_string() {
        let index = NGramIndex::new();
        let distance = index.levenshtein_distance("hello", "hello");
        assert_eq!(distance, 0);
    }

    #[test]
    fn test_levenshtein_different_length() {
        let index = NGramIndex::new();
        let distance = index.levenshtein_distance("kitten", "sitting");
        assert_eq!(distance, 3);
    }

    #[test]
    fn test_levenshtein_empty_strings() {
        let index = NGramIndex::new();
        assert_eq!(index.levenshtein_distance("", ""), 0);
        assert_eq!(index.levenshtein_distance("hello", ""), 5);
        assert_eq!(index.levenshtein_distance("", "world"), 5);
    }

    #[test]
    fn test_fuzzy_search_threshold() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");
        index.add(2, "hallo");
        index.add(3, "xyz");

        let results = index.fuzzy_search_threshold("hello", 0.8, None);
        assert!(results.len() >= 1);
    }

    #[test]
    fn test_jaccard_similarity_same() {
        let index = NGramIndex::new();
        let similarity = index.jaccard_similarity("hello", "hello");
        assert_eq!(similarity, 1.0);
    }

    #[test]
    fn test_jaccard_similarity_different() {
        let index = NGramIndex::new();
        let similarity = index.jaccard_similarity("hello", "hallo");
        assert!(similarity > 0.5);
        assert!(similarity < 1.0);
    }

    #[test]
    fn test_jaccard_similarity_empty() {
        let index = NGramIndex::new();
        let similarity = index.jaccard_similarity("", "");
        assert_eq!(similarity, 1.0);
    }

    #[test]
    fn test_fuzzy_match_with_high_distance() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");

        let results = index.fuzzy_search("xyz", 10);
        // Should still not match due to n-gram filtering
        assert!(results.is_empty());
    }

    #[test]
    fn test_ngram_count() {
        let mut index = NGramIndex::new();
        index.add(1, "hello");

        let count = index.ngram_count();
        assert!(count > 0);
    }

    #[test]
    fn test_large_document_set() {
        let mut index = NGramIndex::new();

        for i in 1..=1000 {
            index.add(i, &format!("document {}", i));
        }

        assert_eq!(index.doc_count(), 1000);
    }

    #[test]
    fn test_special_characters() {
        let mut index = NGramIndex::new();
        index.add(1, "hello@world.com");

        let results = index.fuzzy_search("hello@world.com", 0);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_unicode_characters() {
        let mut index = NGramIndex::new();
        index.add(1, "café");

        let results = index.fuzzy_search("cafe", 2);
        // May or may not match depending on unicode handling
        // This test ensures it doesn't panic
        let _ = results;
    }

    #[test]
    fn test_similar_words_ranking() {
        let mut index = NGramIndex::new();
        index.add(1, "John");
        index.add(2, "Jonah");
        index.add(3, "Jon");

        let results = index.fuzzy_search("John", 2);
        assert!(!results.is_empty());
        // Most similar should be ranked first
        assert_eq!(results[0].doc_id, 1);
    }

    #[test]
    fn test_multiple_typos() {
        let mut index = NGramIndex::new();
        index.add(1, "machine learning");

        // Allow for some distance
        let results = index.fuzzy_search("machne lerning", 3);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_short_strings() {
        let mut index = NGramIndex::new();
        index.add(1, "a");
        index.add(2, "b");

        let results = index.fuzzy_search("a", 0);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_numeric_strings() {
        let mut index = NGramIndex::new();
        index.add(1, "123456");

        let results = index.fuzzy_search("123457", 1);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_ngram_generation() {
        let index = NGramIndex::with_n(3);
        let ngrams = index.generate_ngrams("hello");
        assert!(ngrams.len() > 0);
    }

    #[test]
    fn test_default_creation() {
        let index = NGramIndex::default();
        assert_eq!(index.n(), 3);
        assert_eq!(index.doc_count(), 0);
    }
}
