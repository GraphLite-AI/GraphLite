// Full-text search module for GraphLite

pub mod analyzer;
pub mod benchmark;
pub mod bm25;
pub mod concurrency;
pub mod errors;
pub mod inverted_tantivy_clean;
pub mod limits;
pub mod metadata;
pub mod ngram;
pub mod performance;
pub mod recovery;
pub mod registry;
pub mod types;

#[allow(unused_imports)]
pub use bm25::BM25Scorer;
#[allow(unused_imports)]
pub use inverted_tantivy_clean::InvertedIndex;
#[allow(unused_imports)]
pub use metadata::{get_metadata, get_metadata_for_label, register_metadata, TextIndexMetadata};
#[allow(unused_imports)]
pub use ngram::NGramIndex;
#[allow(unused_imports)]
pub use registry::{
    get_text_index, list_text_indexes, register_text_index, text_index_exists,
    unregister_text_index,
};

/// Re-export commonly used items
pub mod prelude {
    #[allow(unused_imports)]
    pub use super::analyzer::TextAnalyzer;
    #[allow(unused_imports)]
    pub use super::bm25::BM25Scorer;
    #[allow(unused_imports)]
    pub use super::inverted_tantivy_clean::InvertedIndex;
    #[allow(unused_imports)]
    pub use super::metadata::{get_metadata, register_metadata, TextIndexMetadata};
    #[allow(unused_imports)]
    pub use super::ngram::NGramIndex;
    #[allow(unused_imports)]
    pub use super::registry::{get_text_index, register_text_index, unregister_text_index};
}
