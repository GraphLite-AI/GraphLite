// Full-text search module for GraphLite

pub mod types;
pub mod errors;
pub mod analyzer;
pub mod inverted_tantivy_clean;
pub mod bm25;
pub mod ngram;
pub mod registry;
pub mod metadata;
pub mod performance;
pub mod benchmark;
pub mod recovery;
pub mod concurrency;
pub mod limits;

#[allow(unused_imports)]
pub use inverted_tantivy_clean::InvertedIndex;
#[allow(unused_imports)]
pub use bm25::BM25Scorer;
#[allow(unused_imports)]
pub use ngram::NGramIndex;
#[allow(unused_imports)]
pub use registry::{register_text_index, get_text_index, unregister_text_index, list_text_indexes, text_index_exists};
#[allow(unused_imports)]
pub use metadata::{TextIndexMetadata, register_metadata, get_metadata, get_metadata_for_label};

/// Re-export commonly used items
pub mod prelude {
    #[allow(unused_imports)]
    pub use super::analyzer::TextAnalyzer;
    #[allow(unused_imports)]
    pub use super::inverted_tantivy_clean::InvertedIndex;
    #[allow(unused_imports)]
    pub use super::bm25::BM25Scorer;
    #[allow(unused_imports)]
    pub use super::ngram::NGramIndex;
    #[allow(unused_imports)]
    pub use super::registry::{register_text_index, get_text_index, unregister_text_index};
    #[allow(unused_imports)]
    pub use super::metadata::{TextIndexMetadata, register_metadata, get_metadata};
}


