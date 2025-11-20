// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Pattern Analyzer
//!
//! Analyzes MATCH patterns to determine connectivity and optimal execution strategies.

use petgraph::algo::{connected_components, is_cyclic_directed};
use petgraph::graph::{Graph, NodeIndex, UnGraph};
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};

use crate::ast::ast::{Edge, PathPattern, PatternElement};
use crate::plan::pattern_optimization::pattern_analysis::{
    LinearPath, PatternConnectivity, TraversalStep,
};

/// Analyzer for determining pattern connectivity and execution strategies
///
/// **Planned Feature** - Pattern connectivity analyzer for optimization
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug)]
pub struct PatternAnalyzer {
    /// Enable detailed logging for debugging
    debug_mode: bool,
}

impl PatternAnalyzer {
    /// Create a new pattern analyzer
    pub fn new() -> Self {
        PatternAnalyzer { debug_mode: false }
    }

    /// Create a new pattern analyzer with debug mode
    #[allow(dead_code)] // ROADMAP v0.4.0 - Debug mode for pattern analysis troubleshooting
    pub fn new_debug() -> Self {
        PatternAnalyzer { debug_mode: true }
    }

    /// Analyze a set of patterns to determine their connectivity
    pub fn analyze_patterns(&self, patterns: Vec<PathPattern>) -> PatternConnectivity {
        if self.debug_mode {
            log::debug!("Analyzing {} patterns for connectivity", patterns.len());
        }

        let shared_vars = self.find_shared_variables(&patterns);
        let connectivity_graph = self.build_connectivity_graph(&patterns, &shared_vars);

        if self.debug_mode {
            log::debug!(
                "Found {} shared variables: {:?}",
                shared_vars.len(),
                shared_vars.keys().collect::<Vec<_>>()
            );
        }

        PatternConnectivity {
            patterns,
            shared_variables: shared_vars,
            connectivity_graph,
        }
    }

    /// Find variables that appear in multiple patterns
    fn find_shared_variables(&self, patterns: &[PathPattern]) -> HashMap<String, Vec<usize>> {
        let mut var_usage: HashMap<String, Vec<usize>> = HashMap::new();

        for (pattern_idx, pattern) in patterns.iter().enumerate() {
            let vars = self.extract_pattern_variables(pattern);

            if self.debug_mode {
                log::debug!("Pattern {}: variables {:?}", pattern_idx, vars);
            }

            for var in vars {
                var_usage
                    .entry(var)
                    .or_insert_with(Vec::new)
                    .push(pattern_idx);
            }
        }

        // Only keep variables that appear in multiple patterns
        var_usage.retain(|_, indices| indices.len() > 1);
        var_usage
    }

    /// Extract all variable names from a pattern
    fn extract_pattern_variables(&self, pattern: &PathPattern) -> HashSet<String> {
        let mut variables = HashSet::new();

        for element in &pattern.elements {
            match element {
                PatternElement::Node(node) => {
                    if let Some(ref identifier) = node.identifier {
                        variables.insert(identifier.clone());
                    }
                }
                PatternElement::Edge(edge) => {
                    if let Some(ref identifier) = edge.identifier {
                        variables.insert(identifier.clone());
                    }
                }
            }
        }

        variables
    }

    /// Build a graph representing pattern connectivity
    fn build_connectivity_graph(
        &self,
        patterns: &[PathPattern],
        shared_vars: &HashMap<String, Vec<usize>>,
    ) -> Graph<usize, String> {
        let mut graph = Graph::new();
        let mut node_indices: HashMap<usize, NodeIndex> = HashMap::new();

        // Add a node for each pattern
        for (pattern_idx, _) in patterns.iter().enumerate() {
            let node_index = graph.add_node(pattern_idx);
            node_indices.insert(pattern_idx, node_index);
        }

        // Add edges for shared variables
        for (var_name, pattern_indices) in shared_vars {
            // Connect all patterns that share this variable
            for i in 0..pattern_indices.len() {
                for j in i + 1..pattern_indices.len() {
                    let from_pattern = pattern_indices[i];
                    let to_pattern = pattern_indices[j];

                    if let (Some(&from_node), Some(&to_node)) = (
                        node_indices.get(&from_pattern),
                        node_indices.get(&to_pattern),
                    ) {
                        graph.add_edge(from_node, to_node, var_name.clone());

                        if self.debug_mode {
                            log::debug!(
                                "Connected patterns {} and {} via variable '{}'",
                                from_pattern,
                                to_pattern,
                                var_name
                            );
                        }
                    }
                }
            }
        }

        graph
    }

    /// Detect if patterns form a linear path that can be executed via traversal
    pub fn detect_linear_path(&self, connectivity: &PatternConnectivity) -> Option<LinearPath> {
        if connectivity.patterns.len() < 2 {
            return None;
        }

        if self.debug_mode {
            log::debug!(
                "Checking for linear path in {} patterns",
                connectivity.patterns.len()
            );
        }

        // Convert directed graph to undirected for path analysis
        let undirected = self.to_undirected_graph(&connectivity.connectivity_graph);

        // Check if it's a path (no cycles, exactly 2 endpoints)
        if !self.is_simple_path(&undirected) {
            if self.debug_mode {
                log::debug!("Not a simple path - has cycles or branching");
            }
            return None;
        }

        // Find start and end nodes
        let endpoints = self.find_path_endpoints(&undirected);
        if endpoints.len() != 2 {
            if self.debug_mode {
                log::debug!(
                    "Path should have exactly 2 endpoints, found {}",
                    endpoints.len()
                );
            }
            return None;
        }

        // Build the linear path
        let start_pattern_idx = connectivity.connectivity_graph[endpoints[0]];
        self.build_linear_path_from_start(connectivity, start_pattern_idx)
    }

    /// Convert directed graph to undirected for path analysis
    fn to_undirected_graph(&self, directed: &Graph<usize, String>) -> UnGraph<usize, String> {
        let mut undirected = UnGraph::new_undirected();
        let mut node_map = HashMap::new();

        // Add all nodes
        for node_idx in directed.node_indices() {
            let pattern_idx = directed[node_idx];
            let new_node = undirected.add_node(pattern_idx);
            node_map.insert(node_idx, new_node);
        }

        // Add all edges (undirected)
        for edge_idx in directed.edge_indices() {
            if let Some((from, to)) = directed.edge_endpoints(edge_idx) {
                let edge_data = &directed[edge_idx];
                if let (Some(&from_new), Some(&to_new)) = (node_map.get(&from), node_map.get(&to)) {
                    undirected.add_edge(from_new, to_new, edge_data.clone());
                }
            }
        }

        undirected
    }

    /// Check if undirected graph represents a simple path
    fn is_simple_path(&self, graph: &UnGraph<usize, String>) -> bool {
        if graph.node_count() < 2 {
            return false;
        }

        // Count nodes with degree > 2 (branching points)
        let branching_nodes = graph
            .node_indices()
            .filter(|&node| graph.edges(node).count() > 2)
            .count();

        // Should have no branching nodes for a simple path
        branching_nodes == 0
    }

    /// Find the endpoints (nodes with degree 1) of a path graph
    fn find_path_endpoints(&self, graph: &UnGraph<usize, String>) -> Vec<NodeIndex> {
        graph
            .node_indices()
            .filter(|&node| graph.edges(node).count() == 1)
            .collect()
    }

    /// Build linear path starting from a specific pattern
    fn build_linear_path_from_start(
        &self,
        connectivity: &PatternConnectivity,
        start_pattern_idx: usize,
    ) -> Option<LinearPath> {
        let mut steps = Vec::new();
        let mut visited = HashSet::new();
        let mut current_idx = start_pattern_idx;

        visited.insert(current_idx);

        // Traverse the connected patterns to build steps
        while let Some(next_idx) =
            self.find_next_connected_pattern(connectivity, current_idx, &visited)
        {
            // Find the shared variable between current and next pattern
            let shared_var =
                self.find_shared_variable_between_patterns(connectivity, current_idx, next_idx)?;

            // Create traversal step
            let step = self.create_traversal_step(
                &connectivity.patterns[current_idx],
                &connectivity.patterns[next_idx],
                &shared_var,
                next_idx,
            )?;

            steps.push(step);
            visited.insert(next_idx);
            current_idx = next_idx;
        }

        if steps.is_empty() {
            return None;
        }

        let start_pattern = connectivity.patterns[start_pattern_idx].clone();
        Some(LinearPath::new(start_pattern, steps))
    }

    /// Find the next connected pattern that hasn't been visited
    fn find_next_connected_pattern(
        &self,
        connectivity: &PatternConnectivity,
        current_idx: usize,
        visited: &HashSet<usize>,
    ) -> Option<usize> {
        // Find current pattern's node in the graph
        let current_node = connectivity
            .connectivity_graph
            .node_indices()
            .find(|&node| connectivity.connectivity_graph[node] == current_idx)?;

        // Find connected patterns that haven't been visited
        for edge in connectivity.connectivity_graph.edges(current_node) {
            let target_node = edge.target();
            let target_pattern_idx = connectivity.connectivity_graph[target_node];

            if !visited.contains(&target_pattern_idx) {
                return Some(target_pattern_idx);
            }
        }

        None
    }

    /// Find the shared variable between two patterns
    fn find_shared_variable_between_patterns(
        &self,
        connectivity: &PatternConnectivity,
        pattern1_idx: usize,
        pattern2_idx: usize,
    ) -> Option<String> {
        for (var_name, pattern_indices) in &connectivity.shared_variables {
            if pattern_indices.contains(&pattern1_idx) && pattern_indices.contains(&pattern2_idx) {
                return Some(var_name.clone());
            }
        }
        None
    }

    /// Create a traversal step between two connected patterns
    fn create_traversal_step(
        &self,
        from_pattern: &PathPattern,
        to_pattern: &PathPattern,
        shared_var: &str,
        to_pattern_idx: usize,
    ) -> Option<TraversalStep> {
        // Extract the relationship from the patterns
        let relationship =
            self.find_connecting_relationship(from_pattern, to_pattern, shared_var)?;

        // Determine from and to variables
        let (from_var, to_var) =
            self.determine_traversal_direction(from_pattern, to_pattern, shared_var)?;

        Some(TraversalStep {
            from_var,
            relationship,
            to_var,
            selectivity: 0.1, // Default selectivity, will be refined by cost estimator
            pattern_index: to_pattern_idx,
        })
    }

    /// Find the relationship that connects two patterns
    fn find_connecting_relationship(
        &self,
        from_pattern: &PathPattern,
        to_pattern: &PathPattern,
        _shared_var: &str,
    ) -> Option<Edge> {
        // Look for an edge in either pattern that could represent the connection
        for element in &from_pattern.elements {
            if let PatternElement::Edge(edge) = element {
                return Some(edge.clone());
            }
        }

        for element in &to_pattern.elements {
            if let PatternElement::Edge(edge) = element {
                return Some(edge.clone());
            }
        }

        None
    }

    /// Determine the traversal direction between patterns
    fn determine_traversal_direction(
        &self,
        from_pattern: &PathPattern,
        to_pattern: &PathPattern,
        shared_var: &str,
    ) -> Option<(String, String)> {
        // Extract variables from both patterns
        let from_vars = self.extract_pattern_variables(from_pattern);
        let to_vars = self.extract_pattern_variables(to_pattern);

        // Find a variable in from_pattern that's not the shared variable
        let from_var = from_vars.iter().find(|&var| var != shared_var).cloned()?;

        // Find a variable in to_pattern that's not the shared variable
        let to_var = to_vars.iter().find(|&var| var != shared_var).cloned()?;

        Some((from_var, to_var))
    }

    /// Check if patterns form a star pattern (one central node connecting to multiple others)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Star pattern detection for optimized join strategies
    pub fn is_star_pattern(&self, connectivity: &PatternConnectivity) -> bool {
        if connectivity.patterns.len() < 3 {
            return false;
        }

        // Look for a variable that appears in most/all patterns
        for (var_name, pattern_indices) in &connectivity.shared_variables {
            if pattern_indices.len() >= connectivity.patterns.len() - 1 {
                if self.debug_mode {
                    log::debug!("Star pattern detected with center variable '{}'", var_name);
                }
                return true;
            }
        }

        // Also check graph structure - look for a node with high degree
        let max_degree = connectivity
            .connectivity_graph
            .node_indices()
            .map(|node| connectivity.connectivity_graph.edges(node).count())
            .max()
            .unwrap_or(0);

        max_degree >= connectivity.patterns.len() - 1
    }

    /// Check if patterns form a cycle
    #[allow(dead_code)] // ROADMAP v0.4.0 - Cycle detection for preventing infinite traversal
    pub fn has_cycle(&self, connectivity: &PatternConnectivity) -> bool {
        is_cyclic_directed(&connectivity.connectivity_graph)
    }

    /// Get the number of connected components in the pattern graph
    #[allow(dead_code)] // ROADMAP v0.4.0 - Connected component analysis for parallel execution planning
    pub fn count_connected_components(&self, connectivity: &PatternConnectivity) -> usize {
        let undirected = self.to_undirected_graph(&connectivity.connectivity_graph);
        connected_components(&undirected)
    }
}

impl Default for PatternAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::ast::{EdgeDirection, Location, Node};

    fn create_test_node(id: &str, label: &str) -> PatternElement {
        PatternElement::Node(Node {
            identifier: Some(id.to_string()),
            labels: if label.is_empty() {
                vec![]
            } else {
                vec![label.to_string()]
            },
            properties: None,
            location: Location::default(),
        })
    }

    fn create_test_edge(id: Option<&str>, label: &str) -> PatternElement {
        PatternElement::Edge(Edge {
            identifier: id.map(|s| s.to_string()),
            labels: vec![label.to_string()],
            properties: None,
            direction: EdgeDirection::Outgoing,
            quantifier: None,
            location: Location::default(),
        })
    }

    fn create_test_pattern(elements: Vec<PatternElement>) -> PathPattern {
        PathPattern {
            assignment: None,
            path_type: None,
            elements,
            location: Location::default(),
        }
    }

    #[test]
    fn test_variable_extraction() {
        let pattern = create_test_pattern(vec![
            create_test_node("a", "Person"),
            create_test_edge(Some("r"), "KNOWS"),
            create_test_node("b", "Person"),
        ]);

        let analyzer = PatternAnalyzer::new();
        let vars = analyzer.extract_pattern_variables(&pattern);

        assert!(vars.contains("a"));
        assert!(vars.contains("b"));
        assert!(vars.contains("r"));
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_shared_variable_detection() {
        let patterns = vec![
            create_test_pattern(vec![
                create_test_node("a", "Person"),
                create_test_edge(None, "KNOWS"),
                create_test_node("b", "Person"),
            ]),
            create_test_pattern(vec![
                create_test_node("b", "Person"),
                create_test_edge(None, "WORKS_IN"),
                create_test_node("c", "Department"),
            ]),
        ];

        let analyzer = PatternAnalyzer::new();
        let shared = analyzer.find_shared_variables(&patterns);

        assert!(shared.contains_key("b"));
        assert_eq!(shared.get("b").unwrap(), &vec![0, 1]);
        assert_eq!(shared.len(), 1);
    }

    #[test]
    fn test_linear_path_detection() {
        let patterns = vec![
            create_test_pattern(vec![
                create_test_node("a", "Person"),
                create_test_edge(None, "KNOWS"),
                create_test_node("b", "Person"),
            ]),
            create_test_pattern(vec![
                create_test_node("b", "Person"),
                create_test_edge(None, "WORKS_IN"),
                create_test_node("c", "Department"),
            ]),
        ];

        let analyzer = PatternAnalyzer::new_debug();
        let connectivity = analyzer.analyze_patterns(patterns);
        let path = analyzer.detect_linear_path(&connectivity);

        assert!(path.is_some(), "Should detect linear path");
        let path = path.unwrap();
        assert_eq!(path.length(), 1);
    }

    #[test]
    fn test_star_pattern_detection() {
        let patterns = vec![
            create_test_pattern(vec![
                create_test_node("a", "Person"),
                create_test_edge(None, "KNOWS"),
                create_test_node("b", "Person"),
            ]),
            create_test_pattern(vec![
                create_test_node("a", "Person"),
                create_test_edge(None, "WORKS_IN"),
                create_test_node("d", "Department"),
            ]),
            create_test_pattern(vec![
                create_test_node("a", "Person"),
                create_test_edge(None, "LIVES_IN"),
                create_test_node("c", "City"),
            ]),
        ];

        let analyzer = PatternAnalyzer::new_debug();
        let connectivity = analyzer.analyze_patterns(patterns);

        assert!(analyzer.is_star_pattern(&connectivity));
        assert!(!analyzer.has_cycle(&connectivity));
    }

    #[test]
    fn test_no_shared_variables() {
        let patterns = vec![
            create_test_pattern(vec![
                create_test_node("a", "Person"),
                create_test_edge(None, "KNOWS"),
                create_test_node("b", "Person"),
            ]),
            create_test_pattern(vec![
                create_test_node("c", "Department"),
                create_test_edge(None, "HAS_EMPLOYEE"),
                create_test_node("d", "Person"),
            ]),
        ];

        let analyzer = PatternAnalyzer::new();
        let connectivity = analyzer.analyze_patterns(patterns);

        assert!(!connectivity.has_shared_variables());
        assert!(analyzer.detect_linear_path(&connectivity).is_none());
        assert!(!analyzer.is_star_pattern(&connectivity));
        assert_eq!(analyzer.count_connected_components(&connectivity), 2);
    }
}
