// Type definitions for text search components

/// A token produced by a text analyzer
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    /// The token text
    pub text: String,
    /// Position in original text (character offset)
    pub position: usize,
    /// Length of original text before stemming (character offset)
    pub position_length: usize,
}

/// Analysis result containing tokens
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Tokens produced by the analyzer
    pub tokens: Vec<Token>,
    /// Unique token count
    pub unique_count: usize,
}

/// Analyzer configuration
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Language for stemming ('english', 'french', 'spanish', 'german', 'italian', 'portuguese', 'russian', 'swedish', 'norwegian')
    pub language: String,
    /// Whether to lowercase tokens
    pub lowercase: bool,
    /// Whether to remove stop words
    pub remove_stopwords: bool,
    /// Whether to apply stemming
    pub stem: bool,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            language: "english".to_string(),
            lowercase: true,
            remove_stopwords: true,
            stem: true,
        }
    }
}

/// Set of common English stop words
pub fn english_stopwords() -> Vec<&'static str> {
    vec![
        "a",
        "about",
        "above",
        "after",
        "again",
        "against",
        "all",
        "am",
        "an",
        "and",
        "any",
        "are",
        "aren't",
        "as",
        "at",
        "be",
        "because",
        "been",
        "before",
        "being",
        "below",
        "between",
        "both",
        "but",
        "by",
        "can",
        "can't",
        "cannot",
        "could",
        "couldn't",
        "did",
        "didn't",
        "do",
        "does",
        "doesn't",
        "doing",
        "don't",
        "down",
        "during",
        "each",
        "few",
        "for",
        "from",
        "further",
        "had",
        "hadn't",
        "has",
        "hasn't",
        "have",
        "haven't",
        "having",
        "he",
        "he'd",
        "he'll",
        "he's",
        "her",
        "here",
        "here's",
        "hers",
        "herself",
        "him",
        "himself",
        "his",
        "how",
        "how's",
        "i",
        "i'd",
        "i'll",
        "i'm",
        "i've",
        "if",
        "in",
        "into",
        "is",
        "isn't",
        "it",
        "it's",
        "its",
        "itself",
        "just",
        "k",
        "let's",
        "m",
        "me",
        "more",
        "most",
        "mustn't",
        "my",
        "myself",
        "no",
        "nor",
        "not",
        "of",
        "off",
        "on",
        "once",
        "only",
        "or",
        "other",
        "ought",
        "our",
        "ours",
        "ourselves",
        "out",
        "over",
        "own",
        "same",
        "shan't",
        "she",
        "she'd",
        "she'll",
        "she's",
        "should",
        "shouldn't",
        "so",
        "some",
        "such",
        "t",
        "than",
        "that",
        "that's",
        "the",
        "their",
        "theirs",
        "them",
        "themselves",
        "then",
        "there",
        "there's",
        "these",
        "they",
        "they'd",
        "they'll",
        "they're",
        "they've",
        "this",
        "those",
        "through",
        "to",
        "too",
        "under",
        "until",
        "up",
        "very",
        "was",
        "wasn't",
        "we",
        "we'd",
        "we'll",
        "we're",
        "we've",
        "were",
        "weren't",
        "what",
        "what's",
        "when",
        "when's",
        "where",
        "where's",
        "which",
        "while",
        "who",
        "who's",
        "whom",
        "why",
        "why's",
        "with",
        "won't",
        "would",
        "wouldn't",
        "y",
        "you",
        "you'd",
        "you'll",
        "you're",
        "you've",
        "your",
        "yours",
        "yourself",
        "yourselves",
    ]
}

impl AnalysisResult {
    /// Create a new analysis result
    pub fn new(tokens: Vec<Token>) -> Self {
        let unique_count = tokens
            .iter()
            .map(|t| &t.text)
            .collect::<std::collections::HashSet<_>>()
            .len();
        Self {
            tokens,
            unique_count,
        }
    }

    /// Get unique tokens
    pub fn unique_tokens(&self) -> Vec<&str> {
        let mut unique = std::collections::HashSet::new();
        for token in &self.tokens {
            unique.insert(token.text.as_str());
        }
        unique.into_iter().collect()
    }
}
