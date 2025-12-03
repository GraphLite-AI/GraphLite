// BM25 scoring algorithm implementation

use std::collections::HashMap;

/// BM25 scorer for relevance ranking
#[derive(Debug, Clone)]
pub struct BM25Scorer {
    /// k1 parameter (term saturation)
    k1: f64,
    /// b parameter (length normalization)
    b: f64,
    /// Total number of documents in corpus
    total_docs: u64,
    /// Document frequencies (term -> count)
    doc_frequencies: HashMap<String, u64>,
    /// Per-document term frequencies: doc_id -> (term -> freq)
    doc_term_freqs: HashMap<u64, HashMap<String, usize>>,
    /// Document lengths (doc_id -> length)
    doc_lengths: HashMap<u64, usize>,
    /// Average document length
    avg_doc_length: f64,
}

impl BM25Scorer {
    /// Create a new BM25 scorer with default parameters
    pub fn new() -> Self {
        Self {
            k1: 1.2,
            b: 0.75,
            total_docs: 0,
            doc_frequencies: HashMap::new(),
            doc_term_freqs: HashMap::new(),
            doc_lengths: HashMap::new(),
            avg_doc_length: 0.0,
        }
    }

    /// Create a BM25 scorer with custom parameters
    pub fn with_params(k1: f64, b: f64) -> Self {
        Self {
            k1,
            b,
            total_docs: 0,
            doc_frequencies: HashMap::new(),
            doc_lengths: HashMap::new(),
            doc_term_freqs: HashMap::new(),
            avg_doc_length: 0.0,
        }
    }

    /// Register a document in the scorer
    pub fn add_document(&mut self, doc_id: u64, doc_length: usize, terms: Vec<&str>) {
        self.doc_lengths.insert(doc_id, doc_length);
        self.total_docs += 1;
        
        // Update document frequencies and per-document term frequencies
        let mut per_doc: HashMap<String, usize> = HashMap::new();
        for term in terms {
            let term_key = term.to_lowercase();
            *self.doc_frequencies.entry(term_key.clone()).or_insert(0) += 1;
            *per_doc.entry(term_key).or_insert(0) += 1;
        }
        self.doc_term_freqs.insert(doc_id, per_doc);
        
        // Update average document length
        self.recalculate_avg_length();
    }

    /// Calculate BM25 score for a query term in a document
    pub fn score(&self, term: &str, doc_id: u64, term_frequency: usize) -> f64 {
        if self.total_docs == 0 {
            return 0.0;
        }

        let term_key = term.to_lowercase();
        
        // Get document frequency for the term
        let doc_freq = *self.doc_frequencies.get(&term_key).unwrap_or(&1) as f64;
        let doc_length = *self.doc_lengths.get(&doc_id).unwrap_or(&0) as f64;
        let tf = term_frequency as f64;

        // Calculate IDF (inverse document frequency)
        let idf = ((self.total_docs as f64 - doc_freq + 0.5) / (doc_freq + 0.5) + 1.0).ln();

        // Calculate length normalization factor
        let norm_factor = 1.0 - self.b + self.b * (doc_length / self.avg_doc_length);

        // BM25 formula
        let score = idf * ((tf * (self.k1 + 1.0)) / (tf + self.k1 * norm_factor));
        
        score.max(0.0)
    }

    /// Calculate combined score for multiple terms
    pub fn score_query(&self, doc_id: u64, term_frequencies: &HashMap<String, usize>) -> f64 {
        // For BM25 we need the term frequency in the document, not the
        // frequency from the query. Look up per-document term frequencies
        // recorded at indexing time.
        term_frequencies
            .iter()
            .map(|(term, &_qfreq)| {
                let term_key = term.to_lowercase();
                let tf = self
                    .doc_term_freqs
                    .get(&doc_id)
                    .and_then(|m| m.get(&term_key))
                    .cloned()
                    .unwrap_or(0);
                self.score(term, doc_id, tf)
            })
            .sum()
    }

    /// Recalculate average document length
    fn recalculate_avg_length(&mut self) {
        if self.doc_lengths.is_empty() {
            self.avg_doc_length = 0.0;
        } else {
            let total_length: usize = self.doc_lengths.values().sum();
            self.avg_doc_length = total_length as f64 / self.doc_lengths.len() as f64;
        }
    }

    /// Get document count
    pub fn doc_count(&self) -> u64 {
        self.total_docs
    }

    /// Get average document length
    pub fn avg_length(&self) -> f64 {
        self.avg_doc_length
    }

    /// Get parameters
    pub fn params(&self) -> (f64, f64) {
        (self.k1, self.b)
    }

    /// Get document frequency for a term
    pub fn doc_freq(&self, term: &str) -> u64 {
        *self.doc_frequencies.get(&term.to_lowercase()).unwrap_or(&0)
    }

    /// Get document length
    pub fn doc_length(&self, doc_id: u64) -> usize {
        *self.doc_lengths.get(&doc_id).unwrap_or(&0)
    }
}

impl Default for BM25Scorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_scorer() {
        let scorer = BM25Scorer::new();
        assert_eq!(scorer.doc_count(), 0);
        assert_eq!(scorer.params(), (1.2, 0.75));
    }

    #[test]
    fn test_custom_params() {
        let scorer = BM25Scorer::with_params(2.0, 0.5);
        assert_eq!(scorer.params(), (2.0, 0.5));
    }

    #[test]
    fn test_add_document() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["machine", "learning"]);
        assert_eq!(scorer.doc_count(), 1);
        assert_eq!(scorer.doc_length(1), 100);
    }

    #[test]
    fn test_multiple_documents() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["machine", "learning"]);
        scorer.add_document(2, 150, vec!["deep", "learning"]);
        scorer.add_document(3, 200, vec!["machine", "vision"]);
        
        assert_eq!(scorer.doc_count(), 3);
        assert_eq!(scorer.avg_length(), 150.0);
    }

    #[test]
    fn test_score_single_term() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["machine", "learning"]);
        scorer.add_document(2, 100, vec!["deep", "learning"]);
        
        let score = scorer.score("machine", 1, 1);
        assert!(score > 0.0);
    }

    #[test]
    fn test_score_higher_frequency() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["machine"]);
        scorer.add_document(2, 100, vec!["machine"]);
        
        let score1 = scorer.score("machine", 1, 1);
        let score2 = scorer.score("machine", 2, 2);
        
        // Higher term frequency should give higher score
        assert!(score2 > score1);
    }

    #[test]
    fn test_score_lower_doc_length() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 50, vec!["machine"]);
        scorer.add_document(2, 200, vec!["machine"]);
        
        let score1 = scorer.score("machine", 1, 1);
        let score2 = scorer.score("machine", 2, 1);
        
        // Shorter documents should score higher (length normalization)
        assert!(score1 > score2);
    }

    #[test]
    fn test_doc_frequency_effect() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["common"]);
        scorer.add_document(2, 100, vec!["common"]);
        scorer.add_document(3, 100, vec!["rare"]);
        
        let common_score = scorer.score("common", 1, 1);
        let rare_score = scorer.score("rare", 3, 1);
        
        // Rare terms should have higher IDF and better scores
        assert!(rare_score > common_score);
    }

    #[test]
    fn test_query_scoring() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["machine", "learning", "algorithm"]);
        scorer.add_document(2, 100, vec!["deep", "learning"]);
        
        let mut query_terms = HashMap::new();
        query_terms.insert("machine".to_string(), 1);
        query_terms.insert("learning".to_string(), 1);
        
        let score1 = scorer.score_query(1, &query_terms);
        let score2 = scorer.score_query(2, &query_terms);
        
        // Doc 1 should score higher (contains both terms)
        assert!(score1 > score2);
    }

    #[test]
    fn test_zero_score_nonexistent_term() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["hello"]);
        
        let score = scorer.score("nonexistent", 1, 0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_avg_document_length() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["a"]);
        scorer.add_document(2, 200, vec!["b"]);
        scorer.add_document(3, 300, vec!["c"]);
        
        assert_eq!(scorer.avg_length(), 200.0);
    }

    #[test]
    fn test_doc_freq() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["machine", "learning"]);
        scorer.add_document(2, 100, vec!["machine"]);
        
        // "machine" appears in 2 docs
        assert_eq!(scorer.doc_freq("machine"), 2);
        // "learning" appears in 1 doc
        assert_eq!(scorer.doc_freq("learning"), 1);
    }

    #[test]
    fn test_case_insensitive() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["Machine", "LEARNING"]);
        
        let score1 = scorer.score("machine", 1, 1);
        let score2 = scorer.score("MACHINE", 1, 1);
        
        // Should be the same regardless of case
        assert_eq!(score1, score2);
    }

    #[test]
    fn test_large_corpus() {
        let mut scorer = BM25Scorer::new();
        
        // Add many documents
        for i in 1..=10000 {
            scorer.add_document(i, 100, vec!["common", "term"]);
        }
        
        assert_eq!(scorer.doc_count(), 10000);
        assert_eq!(scorer.avg_length(), 100.0);
    }

    #[test]
    fn test_empty_terms() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec![]);
        
        assert_eq!(scorer.doc_count(), 1);
        assert_eq!(scorer.doc_length(1), 100);
    }

    #[test]
    fn test_k1_parameter_effect() {
        let mut scorer1 = BM25Scorer::with_params(1.2, 0.75);
        let mut scorer2 = BM25Scorer::with_params(2.0, 0.75);
        
        scorer1.add_document(1, 100, vec!["test"]);
        scorer2.add_document(1, 100, vec!["test"]);
        
        let score1 = scorer1.score("test", 1, 5);
        let score2 = scorer2.score("test", 1, 5);
        
        // Higher k1 should be more sensitive to term frequency
        assert_ne!(score1, score2);
    }

    #[test]
    fn test_b_parameter_effect() {
        let mut scorer1 = BM25Scorer::with_params(1.2, 0.25);
        let mut scorer2 = BM25Scorer::with_params(1.2, 0.75);
        
        scorer1.add_document(1, 50, vec!["test"]);
        scorer1.add_document(2, 200, vec!["test"]);
        scorer2.add_document(1, 50, vec!["test"]);
        scorer2.add_document(2, 200, vec!["test"]);
        
        let score1_short = scorer1.score("test", 1, 1);
        let score1_long = scorer1.score("test", 2, 1);
        let score2_short = scorer2.score("test", 1, 1);
        let score2_long = scorer2.score("test", 2, 1);
        
        // Higher b = more length normalization effect
        let diff1 = score1_short - score1_long;
        let diff2 = score2_short - score2_long;
        assert!(diff1.abs() < diff2.abs());
    }

    #[test]
    fn test_repeated_terms() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["test", "test", "test"]);
        
        // All occurrences of the same term should update doc_freq
        assert_eq!(scorer.doc_freq("test"), 3);
    }

    #[test]
    fn test_score_consistency() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["machine", "learning"]);
        
        let score1 = scorer.score("machine", 1, 1);
        let score2 = scorer.score("machine", 1, 1);
        
        // Same input should give same output
        assert_eq!(score1, score2);
    }

    #[test]
    fn test_multiple_query_terms() {
        let mut scorer = BM25Scorer::new();
        scorer.add_document(1, 100, vec!["a", "b", "c"]);
        
        let mut query_terms = HashMap::new();
        query_terms.insert("a".to_string(), 1);
        query_terms.insert("b".to_string(), 1);
        query_terms.insert("c".to_string(), 1);
        query_terms.insert("d".to_string(), 0);
        
        let score = scorer.score_query(1, &query_terms);
        assert!(score > 0.0);
    }

    #[test]
    fn test_params_default() {
        let scorer = BM25Scorer::default();
        assert_eq!(scorer.params(), (1.2, 0.75));
    }

    #[test]
    fn test_nonexistent_doc_id() {
        let scorer = BM25Scorer::new();
        assert_eq!(scorer.doc_length(999), 0);
    }
}
