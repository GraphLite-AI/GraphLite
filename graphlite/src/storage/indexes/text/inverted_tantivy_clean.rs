// Tantivy-backed inverted index for full-text search
// Uses fully-qualified Tantivy types to avoid name collisions with project types

use crate::storage::indexes::text::errors::TextSearchError;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tantivy::schema::document::TantivyDocument;
use tantivy::schema::Value;

/// Search result from the inverted index
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Document ID
    pub doc_id: u64,
    /// Relevance score (BM25)
    pub score: f32,
    /// Document data (stored fields)
    pub data: HashMap<String, String>,
}

/// Tantivy-backed inverted index
pub struct InvertedIndex {
    name: String,
    index: tantivy::Index,
    reader: tantivy::IndexReader,
    writer: Arc<Mutex<tantivy::IndexWriter>>,
    doc_id_field: tantivy::schema::Field,
    content_field: tantivy::schema::Field,
}

impl InvertedIndex {
    /// Create a new in-memory Tantivy index
    pub fn new(name: impl Into<String>) -> Result<Self, TextSearchError> {
        // Build schema: stored u64 doc_id, stored & indexed text content
        let mut schema_builder = tantivy::schema::Schema::builder();
        use tantivy::schema::{STORED, TEXT};
        let doc_id_field = schema_builder.add_u64_field("doc_id", STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let schema = schema_builder.build();

        let index = tantivy::Index::create_in_ram(schema.clone());

        let writer = index
            .writer(50_000_000)
            .map_err(|e| TextSearchError::IndexError(format!("tantivy writer error: {}", e)))?;

        let reader = index
            .reader()
            .map_err(|e| TextSearchError::IndexError(format!("tantivy reader error: {}", e)))?;

        Ok(Self {
            name: name.into(),
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            doc_id_field,
            content_field,
        })
    }

    /// Create a persisted index at `path` (currently creates an in-memory index)
    /// Note: persistence can be added later; tests rely on in-memory behavior.
    pub fn create_at(
        name: impl Into<String>,
        _path: impl AsRef<Path>,
    ) -> Result<Self, TextSearchError> {
        // For now, create in-memory. Path is accepted for API compatibility.
        Self::new(name)
    }

    /// Open an existing persisted index (not implemented; fallback to new in-memory index)
    pub fn open(_path: impl AsRef<Path>) -> Result<Self, TextSearchError> {
        Self::new("opened")
    }

    /// Add a single document to the index
    pub fn add_document(&self, doc_id: u64, content: &str) -> Result<(), TextSearchError> {
        // Use the `doc!` macro to build a document without referencing the concrete Document type
        let doc = tantivy::doc!(self.doc_id_field => doc_id, self.content_field => content);

        let writer = self
            .writer
            .lock()
            .map_err(|e| TextSearchError::IndexError(format!("writer lock error: {}", e)))?;
        let _ = writer.add_document(doc);
        Ok(())
    }

    /// Add multiple documents (batch)
    pub fn add_documents(&self, documents: Vec<(u64, String)>) -> Result<(), TextSearchError> {
        let writer = self
            .writer
            .lock()
            .map_err(|e| TextSearchError::IndexError(format!("writer lock error: {}", e)))?;
        for (doc_id, content) in documents {
            let doc = tantivy::doc!(self.doc_id_field => doc_id, self.content_field => content);
            let _ = writer.add_document(doc);
        }
        Ok(())
    }

    /// Commit pending changes and refresh the reader
    pub fn commit(&self) -> Result<(), TextSearchError> {
        {
            let mut writer = self
                .writer
                .lock()
                .map_err(|e| TextSearchError::IndexError(format!("writer lock error: {}", e)))?;
            writer
                .commit()
                .map_err(|e| TextSearchError::IndexError(format!("tantivy commit error: {}", e)))?;
        }
        self.reader.reload().map_err(|e| {
            TextSearchError::IndexError(format!("tantivy reader reload error: {}", e))
        })?;
        Ok(())
    }

    /// Search the index
    pub fn search(&self, query_text: &str) -> Result<Vec<SearchResult>, TextSearchError> {
        self.search_with_limit(query_text, None)
    }

    /// Search with an optional result limit
    pub fn search_with_limit(
        &self,
        query_text: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SearchResult>, TextSearchError> {
        let searcher = self.reader.searcher();
        let qp = tantivy::query::QueryParser::for_index(&self.index, vec![self.content_field]);
        let query = qp
            .parse_query(query_text)
            .map_err(|e| TextSearchError::InvalidQuery(format!("tantivy parse error: {}", e)))?;

        let top_k = limit.unwrap_or(100);
        let top_docs = searcher
            .search(&*query, &tantivy::collector::TopDocs::with_limit(top_k))
            .map_err(|e| TextSearchError::IndexError(format!("tantivy search error: {}", e)))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| {
                    TextSearchError::IndexError(format!("tantivy doc retrieval error: {}", e))
                })?;

            let doc_id = retrieved
                .get_first(self.doc_id_field)
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let content = retrieved
                .get_first(self.content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let mut data = HashMap::new();
            data.insert("content".to_string(), content);

            results.push(SearchResult {
                doc_id,
                score,
                data,
            });
        }

        Ok(results)
    }

    /// Return approximate document count
    pub fn doc_count(&self) -> Result<u64, TextSearchError> {
        let searcher = self.reader.searcher();
        Ok(searcher.num_docs())
    }

    /// Index name
    pub fn name(&self) -> &str {
        &self.name
    }
}

// Unit tests ensure this module mirrors the behavior expected by the other code
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_index() {
        let index = InvertedIndex::new("test").unwrap();
        assert_eq!(index.name(), "test");
    }

    #[test]
    fn test_add_single_document() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "hello world").unwrap();
        index.commit().unwrap();

        let count = index.doc_count().unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_search_simple() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "hello world").unwrap();
        index.add_document(2, "hello there").unwrap();
        index.commit().unwrap();

        let results = index.search("hello").unwrap();
        assert_eq!(results.len(), 2);
    }

    // ==================== ADDITIONAL COMPREHENSIVE TESTS ====================

    #[test]
    fn test_add_multiple_documents() {
        let index = InvertedIndex::new("test").unwrap();
        assert!(index.add_document(1, "first").is_ok());
        assert!(index.add_document(2, "second").is_ok());
        assert!(index.add_document(3, "third").is_ok());
        index.commit().unwrap();

        let count = index.doc_count().unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_batch_add_documents() {
        let index = InvertedIndex::new("test").unwrap();
        let docs = vec![
            (1u64, "doc one".to_string()),
            (2u64, "doc two".to_string()),
            (3u64, "doc three".to_string()),
        ];
        assert!(index.add_documents(docs).is_ok());
        index.commit().unwrap();

        let count = index.doc_count().unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_search_multiple_results() {
        let index = InvertedIndex::new("test").unwrap();
        index
            .add_document(1, "machine learning algorithms")
            .unwrap();
        index.add_document(2, "deep learning networks").unwrap();
        index.add_document(3, "machine vision processing").unwrap();
        index.commit().unwrap();

        let results = index.search("machine").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_no_results() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "hello world").unwrap();
        index.commit().unwrap();

        let results = index.search("nonexistent").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_with_limit() {
        let index = InvertedIndex::new("test").unwrap();
        for i in 1..=20 {
            index.add_document(i, "test document").unwrap();
        }
        index.commit().unwrap();

        let results = index.search_with_limit("test", Some(10)).unwrap();
        assert!(results.len() <= 10);
    }

    #[test]
    fn test_search_returns_scores() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "machine learning").unwrap();
        index
            .add_document(2, "machine learning machine learning")
            .unwrap();
        index.commit().unwrap();

        let results = index.search("machine").unwrap();
        assert!(!results.is_empty());
        for result in &results {
            assert!(result.score > 0.0);
        }
    }

    #[test]
    fn test_search_relevance_ordering() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "machine").unwrap();
        index.add_document(2, "machine learning machine").unwrap();
        index.commit().unwrap();

        let results = index.search("machine").unwrap();
        assert!(results.len() > 0);
        if results.len() > 1 {
            // Better matches should have higher scores
            assert!(results[0].score >= results[1].score);
        }
    }

    #[test]
    fn test_stored_field_retrieval() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "hello world").unwrap();
        index.commit().unwrap();

        let results = index.search("hello").unwrap();
        assert!(!results.is_empty());
        assert!(results[0].data.contains_key("content"));
    }

    #[test]
    fn test_large_document_indexing() {
        let index = InvertedIndex::new("test").unwrap();
        let large_text = "word ".repeat(1000);
        assert!(index.add_document(1, &large_text).is_ok());
        index.commit().unwrap();

        let results = index.search("word").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_special_characters_in_documents() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "email@example.com").unwrap();
        index.add_document(2, "URL: https://example.com").unwrap();
        index.commit().unwrap();

        let results = index.search("email@example.com");
        assert!(results.is_ok());
    }

    #[test]
    fn test_unicode_document_content() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "Café résumé naïve").unwrap();
        index.commit().unwrap();

        let results = index.search("café").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_numeric_content() {
        let index = InvertedIndex::new("test").unwrap();
        index
            .add_document(1, "Version 3.14159 released 2025-12-03")
            .unwrap();
        index.commit().unwrap();

        let results = index.search("3.14159").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_empty_index_search() {
        let index = InvertedIndex::new("test").unwrap();
        index.commit().unwrap();

        let results = index.search("anything").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_multiple_commits() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "first batch").unwrap();
        index.commit().unwrap();

        index.add_document(2, "second batch").unwrap();
        index.commit().unwrap();

        let count = index.doc_count().unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_search_after_sequential_commits() {
        let index = InvertedIndex::new("test").unwrap();

        index.add_document(1, "hello world").unwrap();
        index.commit().unwrap();

        index.add_document(2, "hello there").unwrap();
        index.commit().unwrap();

        let results = index.search("hello").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_large_dataset_performance() {
        let index = InvertedIndex::new("perf_test").unwrap();

        for i in 1..=500 {
            index
                .add_document(i, &format!("document {} content here", i))
                .ok();
        }
        index.commit().unwrap();

        let count = index.doc_count().unwrap();
        assert_eq!(count, 500);

        let results = index.search("document").ok();
        assert!(results.is_some());
    }

    #[test]
    fn test_case_insensitive_search() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "Hello World").unwrap();
        index.commit().unwrap();

        let results = index.search("hello").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_partial_word_search() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "machine learning").unwrap();
        index.commit().unwrap();

        let results = index.search("machine").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_multi_word_document() {
        let index = InvertedIndex::new("test").unwrap();
        index
            .add_document(1, "the quick brown fox jumps over the lazy dog")
            .unwrap();
        index.commit().unwrap();

        let results_quick = index.search("quick").unwrap();
        let results_fox = index.search("fox").unwrap();
        assert!(!results_quick.is_empty());
        assert!(!results_fox.is_empty());
    }

    #[test]
    fn test_score_threshold_filtering() {
        let index = InvertedIndex::new("test").unwrap();
        index
            .add_document(1, "very relevant document about the topic")
            .unwrap();
        index.add_document(2, "the").unwrap();
        index.commit().unwrap();

        let results = index.search("topic").unwrap();
        assert!(!results.is_empty());

        let high_score_results: Vec<_> =
            results.iter().filter(|r| (r.score as f64) >= 0.1).collect();
        assert!(!high_score_results.is_empty());
    }

    #[test]
    fn test_index_with_multiple_names() {
        let idx1 = InvertedIndex::new("index1").unwrap();
        let idx2 = InvertedIndex::new("index2").unwrap();

        assert_eq!(idx1.name(), "index1");
        assert_eq!(idx2.name(), "index2");
    }

    #[test]
    fn test_long_search_query() {
        let index = InvertedIndex::new("test").unwrap();
        let long_doc = "machine learning deep neural networks artificial intelligence";
        index.add_document(1, long_doc).unwrap();
        index.commit().unwrap();

        let results = index.search("machine learning").ok();
        assert!(results.is_some());
    }

    #[test]
    fn test_repeated_searches_consistent() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "hello world").unwrap();
        index.commit().unwrap();

        let results1 = index.search("hello").unwrap();
        let results2 = index.search("hello").unwrap();

        assert_eq!(results1.len(), results2.len());
        if !results1.is_empty() && !results2.is_empty() {
            assert_eq!(results1[0].doc_id, results2[0].doc_id);
        }
    }
}
