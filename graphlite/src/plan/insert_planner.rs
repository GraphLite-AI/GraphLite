// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Logical planner for INSERT statements
//!
//! This module handles the logical planning phase for INSERT operations,
//! resolving identifier mappings and creating well-formed logical plans.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::ast::{Expression, InsertStatement, PatternElement};
use crate::plan::logical::{
    EntityType, InsertPattern, LogicalNode, LogicalPlan, NodeIdentifier, VariableInfo,
};

/// Planner for INSERT statements
pub struct InsertPlanner {
    /// Maps user identifiers to node definitions
    identifier_mappings: HashMap<String, NodeIdentifier>,
    /// Tracks which identifiers have been defined
    defined_identifiers: HashSet<String>,
}

impl InsertPlanner {
    pub fn new() -> Self {
        Self {
            identifier_mappings: HashMap::new(),
            defined_identifiers: HashSet::new(),
        }
    }

    /// Plan an INSERT statement into a logical plan
    pub fn plan_insert(
        &mut self,
        statement: &InsertStatement,
    ) -> Result<LogicalPlan, PlanningError> {
        log::debug!(
            "Planning INSERT statement with {} patterns",
            statement.graph_patterns.len()
        );

        let mut patterns = Vec::new();
        let mut variables = HashMap::new();

        // Process all patterns to resolve identifiers and create logical patterns
        for (pattern_idx, graph_pattern) in statement.graph_patterns.iter().enumerate() {
            log::debug!("Processing pattern {}", pattern_idx);

            for (element_idx, element) in graph_pattern.elements.iter().enumerate() {
                match element {
                    PatternElement::Node(node_pattern) => {
                        let logical_pattern =
                            self.process_node_pattern(node_pattern, &mut variables)?;
                        if let Some(pattern) = logical_pattern {
                            patterns.push(pattern);
                        }
                    }
                    PatternElement::Edge(edge_pattern) => {
                        let logical_patterns = self.process_edge_pattern(
                            edge_pattern,
                            &graph_pattern.elements,
                            element_idx,
                            &mut variables,
                        )?;
                        patterns.extend(logical_patterns);
                    }
                }
            }
        }

        log::debug!(
            "Created {} logical patterns with {} identifier mappings",
            patterns.len(),
            self.identifier_mappings.len()
        );

        let root = LogicalNode::Insert {
            patterns,
            identifier_mappings: self.identifier_mappings.clone(),
        };

        Ok(LogicalPlan { root, variables })
    }

    /// Process a node pattern element
    fn process_node_pattern(
        &mut self,
        node_pattern: &crate::ast::Node,
        variables: &mut HashMap<String, VariableInfo>,
    ) -> Result<Option<InsertPattern>, PlanningError> {
        // Check if this is a reference to an already-defined node
        if let Some(ref identifier) = node_pattern.identifier {
            if self.defined_identifiers.contains(identifier) {
                log::debug!(
                    "Node '{}' is a reference to existing definition, skipping node creation",
                    identifier
                );
                return Ok(None); // This is a reference, not a new node
            }
        }

        // Extract properties
        let properties = if let Some(ref prop_map) = node_pattern.properties {
            self.extract_properties(prop_map)?
        } else {
            HashMap::new()
        };

        // Generate content-based storage ID
        let storage_id = Self::generate_node_content_id(&node_pattern.labels, &properties);

        // If there's an identifier, add it to our mappings and mark as defined
        if let Some(ref identifier) = node_pattern.identifier {
            self.identifier_mappings.insert(
                identifier.clone(),
                NodeIdentifier {
                    storage_id: storage_id.clone(),
                    labels: node_pattern.labels.clone(),
                    is_reference: false,
                },
            );
            self.defined_identifiers.insert(identifier.clone());

            // Add to variables
            variables.insert(
                identifier.clone(),
                VariableInfo {
                    name: identifier.clone(),
                    entity_type: EntityType::Node,
                    labels: node_pattern.labels.clone(),
                    required_properties: properties.keys().cloned().collect(),
                },
            );
        }

        log::debug!("Creating node pattern with storage_id: {}", storage_id);

        Ok(Some(InsertPattern::CreateNode {
            storage_id,
            labels: node_pattern.labels.clone(),
            properties,
            original_identifier: node_pattern.identifier.clone(),
        }))
    }

    /// Process an edge pattern element
    fn process_edge_pattern(
        &mut self,
        edge_pattern: &crate::ast::Edge,
        all_elements: &[PatternElement],
        current_idx: usize,
        variables: &mut HashMap<String, VariableInfo>,
    ) -> Result<Vec<InsertPattern>, PlanningError> {
        // Find source and target nodes
        let source_node_id = self.resolve_adjacent_node(all_elements, current_idx, -1)?;
        let target_node_id = self.resolve_adjacent_node(all_elements, current_idx, 1)?;

        // Extract edge properties
        let properties = if let Some(ref prop_map) = edge_pattern.properties {
            self.extract_properties(prop_map)?
        } else {
            HashMap::new()
        };

        let edge_label = edge_pattern
            .labels
            .first()
            .cloned()
            .unwrap_or_else(|| "CONNECTED".to_string());

        // Generate content-based storage ID for the edge
        let edge_storage_id = Self::generate_edge_content_id(
            &source_node_id,
            &target_node_id,
            &edge_label,
            &properties,
        );

        // Add edge variable if present
        if let Some(ref identifier) = edge_pattern.identifier {
            variables.insert(
                identifier.clone(),
                VariableInfo {
                    name: identifier.clone(),
                    entity_type: EntityType::Edge,
                    labels: edge_pattern.labels.clone(),
                    required_properties: properties.keys().cloned().collect(),
                },
            );
        }

        log::debug!(
            "Creating edge pattern: {} -[{}]-> {}",
            source_node_id,
            edge_label,
            target_node_id
        );

        Ok(vec![InsertPattern::CreateEdge {
            storage_id: edge_storage_id,
            from_node_id: source_node_id,
            to_node_id: target_node_id,
            label: edge_label,
            properties,
            original_identifier: edge_pattern.identifier.clone(),
        }])
    }

    /// Resolve the node adjacent to an edge pattern
    fn resolve_adjacent_node(
        &self,
        all_elements: &[PatternElement],
        edge_idx: usize,
        direction: i32, // -1 for source, +1 for target
    ) -> Result<String, PlanningError> {
        let node_idx = if direction < 0 {
            if edge_idx == 0 {
                return Err(PlanningError::InvalidPattern(
                    "Edge pattern must be preceded by a source node".to_string(),
                ));
            }
            edge_idx - 1
        } else {
            if edge_idx >= all_elements.len() - 1 {
                return Err(PlanningError::InvalidPattern(
                    "Edge pattern must be followed by a target node".to_string(),
                ));
            }
            edge_idx + 1
        };

        match &all_elements[node_idx] {
            PatternElement::Node(node_pattern) => {
                if let Some(ref identifier) = node_pattern.identifier {
                    // Look up the storage ID from our mappings
                    if let Some(node_info) = self.identifier_mappings.get(identifier) {
                        Ok(node_info.storage_id.clone())
                    } else {
                        Err(PlanningError::IdentifierNotFound(identifier.clone()))
                    }
                } else {
                    // Anonymous node - generate ID from content
                    let properties = if let Some(ref prop_map) = node_pattern.properties {
                        self.extract_properties(prop_map)?
                    } else {
                        HashMap::new()
                    };

                    if node_pattern.labels.is_empty() && properties.is_empty() {
                        return Err(PlanningError::InvalidPattern(
                            "Cannot use empty anonymous node in edge pattern".to_string(),
                        ));
                    }

                    Ok(Self::generate_node_content_id(
                        &node_pattern.labels,
                        &properties,
                    ))
                }
            }
            _ => Err(PlanningError::InvalidPattern(
                "Expected node pattern adjacent to edge".to_string(),
            )),
        }
    }

    /// Extract properties from a property map
    fn extract_properties(
        &self,
        prop_map: &crate::ast::PropertyMap,
    ) -> Result<HashMap<String, Expression>, PlanningError> {
        let mut properties = HashMap::new();

        for property in &prop_map.properties {
            properties.insert(property.key.clone(), property.value.clone());
        }

        Ok(properties)
    }

    /// Generate a content-based hash ID for a node
    fn generate_node_content_id(
        labels: &[String],
        properties: &HashMap<String, Expression>,
    ) -> String {
        let mut hasher = DefaultHasher::new();

        // Hash labels (sorted for consistency)
        let mut sorted_labels = labels.to_vec();
        sorted_labels.sort();
        for label in &sorted_labels {
            label.hash(&mut hasher);
        }

        // Hash properties (sorted by key for consistency)
        let mut sorted_properties: Vec<_> = properties.iter().collect();
        sorted_properties.sort_by_key(|(k, _)| *k);
        for (key, value) in sorted_properties {
            key.hash(&mut hasher);
            // Hash the expression in a consistent way
            format!("{:?}", value).hash(&mut hasher);
        }

        let hash = hasher.finish();
        format!("node_{:x}", hash)
    }

    /// Generate a content-based hash ID for an edge
    fn generate_edge_content_id(
        from_node_id: &str,
        to_node_id: &str,
        label: &str,
        properties: &HashMap<String, Expression>,
    ) -> String {
        let mut hasher = DefaultHasher::new();

        // Hash the connection (from_node -> to_node -> label)
        from_node_id.hash(&mut hasher);
        to_node_id.hash(&mut hasher);
        label.hash(&mut hasher);

        // Hash properties (sorted by key for consistency)
        let mut sorted_properties: Vec<_> = properties.iter().collect();
        sorted_properties.sort_by_key(|(k, _)| *k);
        for (key, value) in sorted_properties {
            key.hash(&mut hasher);
            format!("{:?}", value).hash(&mut hasher);
        }

        let hash = hasher.finish();
        format!("edge_{:x}", hash)
    }
}

/// Planning errors specific to INSERT operations
#[derive(Debug, thiserror::Error)]
pub enum PlanningError {
    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),

    #[error("Identifier not found: {0}")]
    IdentifierNotFound(String),
}
