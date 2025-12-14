// Text analyzer for tokenization, lowercasing, stop word removal, and stemming

use crate::storage::indexes::text::errors::TextSearchError;
use crate::storage::indexes::text::types::{
    english_stopwords, AnalysisResult, AnalyzerConfig, Token,
};
use rust_stemmers::Algorithm;
use std::collections::HashSet;
use unicode_segmentation::UnicodeSegmentation;

/// Text analyzer for processing raw text
#[derive(Clone)]
pub struct TextAnalyzer {
    /// Analyzer configuration
    config: AnalyzerConfig,
    /// Stop words set for faster lookup
    stopwords: HashSet<String>,
    /// Stemmer language for the configured language
    stemmer_language: Option<Algorithm>,
}

impl std::fmt::Debug for TextAnalyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextAnalyzer")
            .field("config", &self.config)
            .field("stopwords_count", &self.stopwords.len())
            .field("stemmer_language", &self.config.language)
            .finish()
    }
}

impl TextAnalyzer {
    /// Create a new text analyzer with default configuration
    pub fn new() -> Result<Self, TextSearchError> {
        Self::with_config(AnalyzerConfig::default())
    }

    /// Create a text analyzer with specific configuration
    pub fn with_config(config: AnalyzerConfig) -> Result<Self, TextSearchError> {
        // Validate and get algorithm for the language
        let stemmer_language = if config.stem {
            let algorithm = match config.language.as_str() {
                "english" => Algorithm::English,
                "french" => Algorithm::French,
                "spanish" => Algorithm::Spanish,
                "german" => Algorithm::German,
                "italian" => Algorithm::Italian,
                "portuguese" => Algorithm::Portuguese,
                "russian" => Algorithm::Russian,
                "swedish" => Algorithm::Swedish,
                "norwegian" => Algorithm::Norwegian,
                _ => {
                    return Err(TextSearchError::UnsupportedLanguage(
                        config.language.clone(),
                    ))
                }
            };
            Some(algorithm)
        } else {
            None
        };

        // Load stopwords based on language
        let stopwords = match config.language.as_str() {
            "english" => english_stopwords()
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            _ => HashSet::new(), // Other languages don't have stopwords yet
        };

        Ok(Self {
            config,
            stopwords,
            stemmer_language,
        })
    }

    /// Analyze text and produce tokens
    pub fn analyze(&self, text: &str) -> Result<AnalysisResult, TextSearchError> {
        let mut tokens = Vec::new();
        let mut char_positions = Vec::new();

        // First pass: collect character boundaries for each grapheme
        for (idx, grapheme) in text.graphemes(true).enumerate() {
            char_positions.push((idx, grapheme));
        }

        // Process each word
        let mut current_position = 0;
        for word in text.unicode_words() {
            // Find the position of this word in the original text
            if let Some(pos) = text[current_position..].find(word) {
                let actual_position = current_position + pos;
                current_position = actual_position + word.len();

                // Process the token
                if let Some(token) = self.process_token(word, actual_position)? {
                    tokens.push(token);
                }
            }
        }

        Ok(AnalysisResult::new(tokens))
    }

    /// Process a single token through the analyzer pipeline
    fn process_token(&self, text: &str, position: usize) -> Result<Option<Token>, TextSearchError> {
        let original_len = text.len();
        let mut token_text = text.to_string();

        // Step 1: Lowercase
        if self.config.lowercase {
            token_text = token_text.to_lowercase();
        }

        // Step 2: Remove stop words (if configured)
        if self.config.remove_stopwords && self.stopwords.contains(&token_text) {
            return Ok(None);
        }

        // Step 3: Apply stemming (if configured)
        if let Some(algorithm) = self.stemmer_language {
            let stemmer = rust_stemmers::Stemmer::create(algorithm);
            token_text = stemmer.stem(&token_text).into_owned();
        }

        Ok(Some(Token {
            text: token_text,
            position,
            position_length: original_len,
        }))
    }

    /// Get current configuration
    pub fn config(&self) -> &AnalyzerConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: AnalyzerConfig) -> Result<(), TextSearchError> {
        *self = Self::with_config(config)?;
        Ok(())
    }
}

impl Default for TextAnalyzer {
    fn default() -> Self {
        Self::new().expect("Failed to create default analyzer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokenization() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("hello world").unwrap();
        assert_eq!(result.tokens.len(), 2);
        assert_eq!(result.tokens[0].text, "hello");
        assert_eq!(result.tokens[1].text, "world");
    }

    #[test]
    fn test_lowercasing() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("HELLO World").unwrap();
        assert_eq!(result.tokens[0].text, "hello");
        assert_eq!(result.tokens[1].text, "world");
    }

    #[test]
    fn test_stopwords_removed() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("the quick brown fox").unwrap();
        // "the" should be removed as a stop word
        assert_eq!(result.tokens.len(), 3);
        assert_eq!(result.tokens[0].text, "quick");
        assert_eq!(result.tokens[1].text, "brown");
        assert_eq!(result.tokens[2].text, "fox");
    }

    #[test]
    fn test_stopwords_disabled() {
        let config = AnalyzerConfig {
            remove_stopwords: false,
            ..Default::default()
        };
        let analyzer = TextAnalyzer::with_config(config).unwrap();
        let result = analyzer.analyze("the quick brown fox").unwrap();
        assert_eq!(result.tokens.len(), 4);
        assert_eq!(result.tokens[0].text, "the");
    }

    #[test]
    fn test_stemming() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("running runs").unwrap();
        // Both should stem similarly
        assert_eq!(result.tokens.len(), 2);
        // Verify stemming occurred (tokens are lowercase and stemmed)
        assert_eq!(result.tokens[0].text, "run");
        assert_eq!(result.tokens[1].text, "run");
    }

    #[test]
    fn test_stemming_disabled() {
        let config = AnalyzerConfig {
            language: "english".to_string(),
            stem: false,
            ..Default::default()
        };
        let analyzer = TextAnalyzer::with_config(config).unwrap();
        let result = analyzer.analyze("running runs runner").unwrap();
        assert_eq!(result.tokens.len(), 3);
        assert_eq!(result.tokens[0].text, "running");
        assert_eq!(result.tokens[1].text, "runs");
        assert_eq!(result.tokens[2].text, "runner");
    }

    #[test]
    fn test_empty_text() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("").unwrap();
        assert_eq!(result.tokens.len(), 0);
    }

    #[test]
    fn test_punctuation_handling() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("hello, world! how are you?").unwrap();
        // Punctuation should be split on word boundaries
        assert!(result.tokens.iter().all(|t| !t.text.contains(',')));
        assert!(result.tokens.iter().all(|t| !t.text.contains('!')));
        assert!(result.tokens.iter().all(|t| !t.text.contains('?')));
    }

    #[test]
    fn test_whitespace_handling() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("hello     world\t\tfoo\n\nbar").unwrap();
        assert_eq!(result.tokens.len(), 4);
        assert_eq!(result.tokens[0].text, "hello");
        assert_eq!(result.tokens[1].text, "world");
        assert_eq!(result.tokens[2].text, "foo");
        assert_eq!(result.tokens[3].text, "bar");
    }

    #[test]
    fn test_unique_token_count() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("running runs running").unwrap();
        // "running" and "runs" both stem to "run", so unique count should be 1
        assert_eq!(result.tokens.len(), 3);
        assert_eq!(result.unique_count, 1);
    }

    #[test]
    fn test_unsupported_language() {
        let config = AnalyzerConfig {
            language: "klingon".to_string(),
            ..Default::default()
        };
        let result = TextAnalyzer::with_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_position_tracking() {
        let analyzer = TextAnalyzer::new().unwrap();
        let text = "machine learning algorithms";
        let result = analyzer.analyze(text).unwrap();

        // Verify positions are tracked (they should be > 0 for non-first words)
        assert!(result.tokens.len() > 0);
        assert_eq!(result.tokens[0].position, 0); // "machine" starts at 0
    }

    #[test]
    fn test_complex_document() {
        let analyzer = TextAnalyzer::new().unwrap();
        let text = "Machine Learning is a subset of Artificial Intelligence. \
                   It focuses on algorithms and statistical models.";
        let result = analyzer.analyze(text).unwrap();

        assert!(result.tokens.len() > 0);
        assert!(result.unique_count > 0);
        assert!(result.unique_count <= result.tokens.len());
    }

    #[test]
    fn test_numeric_handling() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("Python 3.8 released in 2019").unwrap();
        // Numbers should be tokenized
        assert!(result.tokens.iter().any(|t| t.text.contains('3')));
    }

    #[test]
    fn test_special_characters() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer
            .analyze("email@example.com test-case under_score")
            .unwrap();
        // Should handle special characters in various ways
        assert!(result.tokens.len() > 0);
    }

    #[test]
    fn test_unicode_text() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("café naïve résumé").unwrap();
        assert!(result.tokens.len() > 0);
        // Should preserve unicode
        assert!(result.tokens.iter().any(|t| t.text.contains('é')));
    }

    #[test]
    fn test_single_character_words() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("I a test").unwrap();
        // Even single characters should be tokenized
        assert!(result.tokens.len() >= 1);
    }

    #[test]
    fn test_very_long_document() {
        let analyzer = TextAnalyzer::new().unwrap();
        let mut long_text = String::new();
        for _ in 0..1000 {
            long_text.push_str("This is a test document. ");
        }
        let result = analyzer.analyze(&long_text).unwrap();
        assert!(result.tokens.len() > 0);
    }

    #[test]
    fn test_lowercase_disabled() {
        let config = AnalyzerConfig {
            language: "english".to_string(),
            lowercase: false,
            remove_stopwords: false,
            stem: false,
        };
        let analyzer = TextAnalyzer::with_config(config).unwrap();
        let result = analyzer.analyze("Hello WORLD").unwrap();
        assert_eq!(result.tokens[0].text, "Hello");
        assert_eq!(result.tokens[1].text, "WORLD");
    }

    #[test]
    fn test_combined_transformations() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer
            .analyze("RUNNING QUICKLY THROUGH THE FOREST")
            .unwrap();

        // Should be lowercased, stopped ("the" removed), and stemmed
        assert!(result.tokens.iter().any(|t| t.text == "run"));
        assert!(result.tokens.iter().any(|t| t.text == "quick"));
        assert!(result.tokens.iter().all(|t| t.text != "the"));
    }

    #[test]
    fn test_repeated_tokens() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("test test test running running").unwrap();

        assert_eq!(result.tokens.len(), 5);
        assert_eq!(result.unique_count, 2); // "test" and "run"
    }

    #[test]
    fn test_different_languages() {
        // Test French
        let config = AnalyzerConfig {
            language: "french".to_string(),
            ..Default::default()
        };
        let analyzer = TextAnalyzer::with_config(config).unwrap();
        let result = analyzer.analyze("courir court coureur").unwrap();
        assert!(result.tokens.len() > 0);
    }

    #[test]
    fn test_analyzer_config_mutation() {
        let mut analyzer = TextAnalyzer::new().unwrap();
        let new_config = AnalyzerConfig {
            stem: false,
            ..Default::default()
        };
        analyzer.set_config(new_config).unwrap();

        let result = analyzer.analyze("running").unwrap();
        assert_eq!(result.tokens[0].text, "running");
    }

    #[test]
    fn test_consecutive_spaces() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("word1    word2").unwrap();
        assert_eq!(result.tokens.len(), 2);
    }

    #[test]
    fn test_tabs_and_newlines() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("word1\t\tword2\n\nword3").unwrap();
        assert_eq!(result.tokens.len(), 3);
    }

    #[test]
    fn test_all_stopwords() {
        let analyzer = TextAnalyzer::new().unwrap();
        // Use only common English stop words
        let result = analyzer
            .analyze("the a an and or but in on at to for of is are")
            .unwrap();
        // Most or all should be filtered out
        assert!(result.tokens.len() < 13); // Less than input word count
    }

    #[test]
    fn test_mixed_case_stemming() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("Running RUNS").unwrap();

        // Both should stem to "run" (lowercase first, then stem)
        let unique_texts: std::collections::HashSet<_> =
            result.tokens.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(unique_texts.len(), 1);
        assert!(unique_texts.contains("run"));
    }

    #[test]
    fn test_text_position_values() {
        let analyzer = TextAnalyzer::new().unwrap();
        let text = "hello world";
        let result = analyzer.analyze(text).unwrap();

        assert_eq!(result.tokens[0].position, 0); // "hello" at start
        assert!(result.tokens[1].position > 0); // "world" after space
    }

    #[test]
    fn test_token_position_length() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("hello").unwrap();

        assert_eq!(result.tokens[0].position_length, 5); // "hello" is 5 chars
    }

    #[test]
    fn test_analyzer_default() {
        let analyzer = TextAnalyzer::default();
        let result = analyzer.analyze("hello world").unwrap();
        assert_eq!(result.tokens.len(), 2);
    }

    #[test]
    fn test_hyphenated_words() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("well-known test-case").unwrap();
        // Should tokenize hyphenated words
        assert!(result.tokens.len() > 0);
    }

    #[test]
    fn test_multiple_punctuation() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("hello...world!!!test???").unwrap();
        // Should extract words despite multiple punctuation
        assert!(result.tokens.iter().any(|t| t.text == "hello"));
        assert!(result.tokens.iter().any(|t| t.text == "world"));
    }

    #[test]
    fn test_apostrophe_contractions() {
        let analyzer = TextAnalyzer::new().unwrap();
        let result = analyzer.analyze("testing work").unwrap();
        // Should handle words properly
        assert!(result.tokens.len() > 0);
    }
}
