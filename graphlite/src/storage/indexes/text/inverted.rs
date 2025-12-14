// Tantivy-backed inverted index for full-text search
// Uses fully-qualified Tantivy types to avoid name collisions with project types

use crate::storage::indexes::text::errors::TextSearchError;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Search result from the inverted index
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Document ID
    pub doc_id: u64,
    /// Relevance score (BM25)
    pub score: f32,
    /// Document data (stored fields)
    pub data: HashMap<String, String>,
// Tantivy-backed inverted index for full-text search
// Uses fully-qualified Tantivy types to avoid name collisions with project types

use crate::storage::indexes::text::errors::TextSearchError;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

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
    pub fn create_at(name: impl Into<String>, _path: impl AsRef<Path>) -> Result<Self, TextSearchError> {
        // For now, create in-memory. Path is accepted for API compatibility.
        Self::new(name)
    }

    /// Open an existing persisted index (not implemented; fallback to new in-memory index)
    pub fn open(_path: impl AsRef<Path>) -> Result<Self, TextSearchError> {
        Self::new("opened")
    }

    /// Add a single document to the index
    pub fn add_document(&self, doc_id: u64, content: &str) -> Result<(), TextSearchError> {
        let mut doc = tantivy::document::Document::default();
        doc.add_u64(self.doc_id_field, doc_id);
        doc.add_text(self.content_field, content);

        let mut writer = self.writer.lock().map_err(|e| TextSearchError::IndexError(format!("writer lock error: {}", e)))?;
        writer.add_document(doc);
        Ok(())
    }

    /// Add multiple documents (batch)
    pub fn add_documents(&self, documents: Vec<(u64, String)>) -> Result<(), TextSearchError> {
        let mut writer = self.writer.lock().map_err(|e| TextSearchError::IndexError(format!("writer lock error: {}", e)))?;
        for (doc_id, content) in documents {
            let mut doc = tantivy::document::Document::default();
            doc.add_u64(self.doc_id_field, doc_id);
            doc.add_text(self.content_field, &content);
            writer.add_document(doc);
        }
        Ok(())
    }

    /// Commit pending changes and refresh the reader
    pub fn commit(&self) -> Result<(), TextSearchError> {
        {
            let mut writer = self.writer.lock().map_err(|e| TextSearchError::IndexError(format!("writer lock error: {}", e)))?;
            writer.commit().map_err(|e| TextSearchError::IndexError(format!("tantivy commit error: {}", e)))?;
        }
        self.reader.reload().map_err(|e| TextSearchError::IndexError(format!("tantivy reader reload error: {}", e)))?;
        Ok(())
    }

    /// Search the index
    pub fn search(&self, query_text: &str) -> Result<Vec<SearchResult>, TextSearchError> {
        self.search_with_limit(query_text, None)
    }

    /// Search with an optional result limit
    pub fn search_with_limit(&self, query_text: &str, limit: Option<usize>) -> Result<Vec<SearchResult>, TextSearchError> {
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
            let retrieved = searcher.doc(doc_address).map_err(|e| TextSearchError::IndexError(format!("tantivy doc retrieval error: {}", e)))?;

            let doc_id = retrieved.get_first(self.doc_id_field).and_then(|v| v.as_u64()).unwrap_or(0);

            let content = retrieved.get_first(self.content_field).and_then(|v| v.as_text()).unwrap_or("").to_string();

            let mut data = HashMap::new();
            data.insert("content".to_string(), content);

            results.push(SearchResult { doc_id, score, data });
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
    fn test_add_multiple_documents() {
        let index = InvertedIndex::new("test").unwrap();
        let docs = vec![
            (1, "machine learning".to_string()),
            (2, "deep learning".to_string()),
            (3, "machine vision".to_string()),
        ];
        index.add_documents(docs).unwrap();
        index.commit().unwrap();

        let count = index.doc_count().unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_search_simple() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "hello world").unwrap();
        index.add_document(2, "hello there").unwrap();
        index.add_document(3, "goodbye world").unwrap();
        index.commit().unwrap();

        let results = index.search("hello").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_with_limit() {
        let index = InvertedIndex::new("test").unwrap();
        for i in 1..=10 {
            index.add_document(i, "test document").unwrap();
        }
        index.commit().unwrap();

        let results = index.search_with_limit("test", Some(5)).unwrap();
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_search_no_results() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "hello world").unwrap();
        index.commit().unwrap();

        let results = index.search("nonexistent").unwrap();
        assert_eq!(results.is_empty(), true);
    }

    #[test]
    fn test_search_returns_scores() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "machine learning machine").unwrap();
        index.add_document(2, "machine learning").unwrap();
        index.add_document(3, "learning").unwrap();
        index.commit().unwrap();

        let results = index.search("machine").unwrap();
        assert!(!results.is_empty());
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_multiple_terms() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "machine learning algorithms").unwrap();
        index.add_document(2, "deep learning networks").unwrap();
        index.add_document(3, "machine vision processing").unwrap();
        index.commit().unwrap();

        let results = index.search("machine learning").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_returns_stored_data() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "hello world").unwrap();
        index.commit().unwrap();

        let results = index.search("hello").unwrap();
        assert!(!results.is_empty());
        assert!(results[0].data.contains_key("content"));
    }

    #[test]
    fn test_large_batch_insert() {
        let index = InvertedIndex::new("test").unwrap();
        let mut docs = Vec::new();
        for i in 1..=100 {
            docs.push((i, format!("document {}", i)));
        }
        index.add_documents(docs).unwrap();
        index.commit().unwrap();

        let count = index.doc_count().unwrap();
        assert_eq!(count, 100);
    }

    #[test]
    fn test_search_performance() {
        let index = InvertedIndex::new("test").unwrap();

        // Add documents
        let mut docs = Vec::new();
        for i in 1..=1000 {
            docs.push((i, "machine learning neural networks".to_string()));
        }
        index.add_documents(docs).unwrap();
        index.commit().unwrap();

        // Time the search
        let start = std::time::Instant::now();
        let _results = index.search("machine").unwrap();
        let elapsed = start.elapsed();

        // Should be reasonably fast (< 1000ms for 1K docs in CI)
        assert!(elapsed.as_millis() < 1000);
    }

    #[test]
    fn test_search_after_multiple_commits() {
        let index = InvertedIndex::new("test").unwrap();

        index.add_document(1, "hello").unwrap();
        index.commit().unwrap();

        index.add_document(2, "world").unwrap();
        index.commit().unwrap();

        index.add_document(3, "hello world").unwrap();
        index.commit().unwrap();

        let results = index.search("hello").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_exact_doc_id() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(42, "specific document").unwrap();
        index.add_document(43, "another document").unwrap();
        index.commit().unwrap();

        let results = index.search("document").unwrap();
        assert!(!results.is_empty());

        // Check that doc_ids are preserved
        assert!(results.iter().any(|r| r.doc_id == 42));
        assert!(results.iter().any(|r| r.doc_id == 43));
    }

    #[test]
    fn test_case_insensitive_search() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "Hello World").unwrap();
        index.commit().unwrap();

        let results = index.search("hello").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, 1);
    }

    #[test]
    fn test_partial_match() {
        let index = InvertedIndex::new("test").unwrap();
        index.add_document(1, "machine learning").unwrap();
        index.commit().unwrap();

        let results = index.search("machine").unwrap();
        assert_eq!(results.len(), 1);
    }
}

