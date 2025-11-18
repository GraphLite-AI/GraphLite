// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Value type system for graph properties
//!
//! Supports various data types commonly used in fraud detection:
//! - Basic types: String, Number, Boolean, Null
//! - Temporal types: DateTime
//! - Collections: Array

use chrono::{DateTime, Utc, FixedOffset};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};
use crate::storage::types::{Node, Edge};

/// Temporal value wrapper that adds temporal metadata to any value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemporalValue {
    pub value: Box<Value>,
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub transaction_time: DateTime<Utc>,
}

impl TemporalValue {
    /// Create a new temporal value with current transaction time
    pub fn new(value: Value, valid_from: DateTime<Utc>) -> Self {
        Self {
            value: Box::new(value),
            valid_from,
            valid_to: None,
            transaction_time: Utc::now(),
        }
    }
    
    /// Create a temporal value with explicit time bounds
    pub fn with_bounds(
        value: Value,
        valid_from: DateTime<Utc>,
        valid_to: Option<DateTime<Utc>>,
        transaction_time: DateTime<Utc>,
    ) -> Self {
        Self {
            value: Box::new(value),
            valid_from,
            valid_to,
            transaction_time,
        }
    }
    
    /// Check if this value is valid at a specific point in time
    pub fn is_valid_at(&self, time: DateTime<Utc>) -> bool {
        time >= self.valid_from && self.valid_to.map_or(true, |vt| time < vt)
    }   
    
    /// Check if this value is currently valid (valid_to is None or in the future)
    pub fn is_current(&self) -> bool {
        self.valid_to.map_or(true, |vt| vt > Utc::now())
    }
}

/// Time window for temporal range operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeWindow {
    /// Create a new time window
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Self, String> {
        if start > end {
            return Err("Time window start cannot be after end".to_string());
        }
        Ok(Self { start, end })
    }
    
    /// Check if a datetime falls within this time window
    pub fn contains(&self, dt: &DateTime<Utc>) -> bool {
        dt >= &self.start && dt <= &self.end
    }
    
    /// Get the duration of this time window in seconds
    pub fn duration_seconds(&self) -> i64 {
        self.end.signed_duration_since(self.start).num_seconds()
    }
}

/// Path element in a graph path
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathElement {
    pub node_id: String,
    pub edge_id: Option<String>,
}

/// Path value representing a sequence of nodes and edges
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathValue {
    pub elements: Vec<PathElement>,
}

impl PathValue {
    /// Create a new empty path
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }
    
    /// Create a path from a list of elements
    pub fn from_elements(elements: Vec<PathElement>) -> Self {
        Self { elements }
    }
    
    /// Add an element to the path
    pub fn add_element(&mut self, element: PathElement) {
        self.elements.push(element);
    }
    
    /// Get the length of the path (number of edges)
    pub fn length(&self) -> usize {
        self.elements.iter().filter(|e| e.edge_id.is_some()).count()
    }
    
    /// Get all node IDs in the path
    pub fn get_nodes(&self) -> Vec<&str> {
        self.elements.iter().map(|e| e.node_id.as_str()).collect()
    }
    
    /// Get all edge IDs in the path (non-None values)
    pub fn get_edges(&self) -> Vec<&str> {
        self.elements.iter()
            .filter_map(|e| e.edge_id.as_ref().map(|id| id.as_str()))
            .collect()
    }
}

/// Value types for graph node and edge properties
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    DateTime(DateTime<Utc>),
    DateTimeWithFixedOffset(DateTime<FixedOffset>),
    DateTimeWithNamedTz(String, DateTime<Utc>), // Store timezone name and UTC datetime
    TimeWindow(TimeWindow),
    Array(Vec<Value>),
    List(Vec<Value>), // Alias for Array for ISO GQL compatibility
    Vector(Vec<f32>), // Dedicated vector type for better performance and type safety
    Path(PathValue),  // PATH constructor support
    Node(Node),       // Graph node with labels and properties
    Edge(Edge),       // Graph edge with label and properties
    Temporal(TemporalValue), // Temporal value wrapper
    Null,
}

impl Value {
    /// Extract as number if possible
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }
    
    /// Extract as string if possible
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    
    /// Extract as boolean if possible
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }
    
    /// Extract as datetime if possible (UTC only)
    pub fn as_datetime(&self) -> Option<&DateTime<Utc>> {
        match self {
            Value::DateTime(dt) => Some(dt),
            Value::DateTimeWithNamedTz(_, dt) => Some(dt),
            _ => None,
        }
    }
    
    /// Extract as datetime with fixed offset if possible
    pub fn as_datetime_with_offset(&self) -> Option<&DateTime<FixedOffset>> {
        match self {
            Value::DateTimeWithFixedOffset(dt) => Some(dt),
            _ => None,
        }
    }
    
    /// Extract as datetime with named timezone if possible
    pub fn as_datetime_with_named_tz(&self) -> Option<(&str, &DateTime<Utc>)> {
        match self {
            Value::DateTimeWithNamedTz(tz_name, dt) => Some((tz_name, dt)),
            _ => None,
        }
    }
    
    /// Get any datetime as UTC, converting if necessary
    pub fn as_datetime_utc(&self) -> Option<DateTime<Utc>> {
        match self {
            Value::DateTime(dt) => Some(*dt),
            Value::DateTimeWithFixedOffset(dt) => Some(dt.with_timezone(&Utc)),
            Value::DateTimeWithNamedTz(_, dt) => Some(*dt),
            _ => None,
        }
    }
    
    /// Get timezone information if available
    pub fn get_timezone_info(&self) -> Option<String> {
        match self {
            Value::DateTime(_) => Some("UTC".to_string()),
            Value::DateTimeWithFixedOffset(dt) => Some(dt.timezone().to_string()),
            Value::DateTimeWithNamedTz(tz_name, _) => Some(tz_name.clone()),
            _ => None,
        }
    }
    
    /// Extract as array if possible
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Extract as list (supports both List and Array variants)
    pub fn as_list(&self) -> Option<&Vec<Value>> {
        match self {
            Value::List(list) => Some(list),
            Value::Array(arr) => Some(arr), // Backward compatibility
            _ => None,
        }
    }

    /// Extract as integer if possible (from number)
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Number(n) => Some(*n as i64),
            _ => None,
        }
    }
    
    /// Extract as time window if possible
    pub fn as_time_window(&self) -> Option<&TimeWindow> {
        match self {
            Value::TimeWindow(tw) => Some(tw),
            _ => None,
        }
    }
    
    /// Extract as vector if possible
    pub fn as_vector(&self) -> Option<&Vec<f32>> {
        match self {
            Value::Vector(v) => Some(v),
            _ => None,
        }
    }
    
    /// Extract as path if possible
    pub fn as_path(&self) -> Option<&PathValue> {
        match self {
            Value::Path(p) => Some(p),
            _ => None,
        }
    }
    
    /// Extract as node if possible
    pub fn as_node(&self) -> Option<&Node> {
        match self {
            Value::Node(n) => Some(n),
            _ => None,
        }
    }
    
    /// Extract as edge if possible
    pub fn as_edge(&self) -> Option<&Edge> {
        match self {
            Value::Edge(e) => Some(e),
            _ => None,
        }
    }
    
    /// Extract as temporal value if possible
    pub fn as_temporal(&self) -> Option<&TemporalValue> {
        match self {
            Value::Temporal(tv) => Some(tv),
            _ => None,
        }
    }
    
    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    
    /// Get the type name of this value
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "String",
            Value::Number(_) => "Number",
            Value::Boolean(_) => "Boolean", 
            Value::DateTime(_) => "DateTime",
            Value::DateTimeWithFixedOffset(_) => "DateTimeWithOffset",
            Value::DateTimeWithNamedTz(_, _) => "DateTimeWithTz",
            Value::TimeWindow(_) => "TimeWindow",
            Value::Array(_) => "Array",
            Value::Vector(_) => "Vector",
            Value::Path(_) => "Path",
            Value::Node(_) => "Node",
            Value::Edge(_) => "Edge",
            Value::Temporal(_) => "Temporal",
            Value::Null => "Null",
            Value::List(_) => "List",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Number(n) => write!(f, "{}", n),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::DateTime(dt) => write!(f, "{}", dt.format("%Y-%m-%d %H:%M:%S UTC")),
            Value::DateTimeWithFixedOffset(dt) => write!(f, "{}", dt.format("%Y-%m-%d %H:%M:%S %:z")),
            Value::DateTimeWithNamedTz(tz_name, dt) => write!(f, "{} {}", dt.format("%Y-%m-%d %H:%M:%S"), tz_name),
            Value::TimeWindow(tw) => write!(f, "TIME_WINDOW({}, {})", 
                tw.start.format("%Y-%m-%dT%H:%M:%SZ"), 
                tw.end.format("%Y-%m-%dT%H:%M:%SZ")),
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::List(list) => {
                write!(f, "[")?;
                for (i, item) in list.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Vector(vec) => {
                write!(f, "VECTOR[")?;
                for (i, item) in vec.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Path(path) => {
                write!(f, "PATH[")?;
                for (i, element) in path.elements.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    if let Some(edge_id) = &element.edge_id {
                        write!(f, "({}, {})", element.node_id, edge_id)?;
                    } else {
                        write!(f, "{}", element.node_id)?;
                    }
                }
                write!(f, "]")
            }
            Value::Node(node) => {
                write!(f, "NODE({}, [{}])", node.id, node.labels.join(", "))
            }
            Value::Edge(edge) => {
                write!(f, "EDGE({}, {}-[{}]->{}, {})", edge.id, edge.from_node, edge.label, edge.to_node, edge.properties.len())
            }
            Value::Temporal(tv) => {
                write!(f, "TEMPORAL({}, valid_from: {}, valid_to: {}, tx_time: {})", 
                    tv.value,
                    tv.valid_from.format("%Y-%m-%dT%H:%M:%SZ"),
                    tv.valid_to.map_or("ongoing".to_string(), |vt| vt.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                    tv.transaction_time.format("%Y-%m-%dT%H:%M:%SZ"))
            }
            Value::Null => write!(f, "null"),
        }
    }
}

/// Convert from Rust primitive types to Value
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Number(n as f64)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Boolean(b)
    }
}

impl From<DateTime<Utc>> for Value {
    fn from(dt: DateTime<Utc>) -> Self {
        Value::DateTime(dt)
    }
}

impl From<TimeWindow> for Value {
    fn from(tw: TimeWindow) -> Self {
        Value::TimeWindow(tw)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(vec: Vec<T>) -> Self {
        Value::Array(vec.into_iter().map(Into::into).collect())
    }
}

impl From<Vec<f32>> for Value {
    fn from(vec: Vec<f32>) -> Self {
        Value::Vector(vec)
    }
}

impl From<PathValue> for Value {
    fn from(path: PathValue) -> Self {
        Value::Path(path)
    }
}

impl From<Node> for Value {
    fn from(node: Node) -> Self {
        Value::Node(node)
    }
}

impl From<Edge> for Value {
    fn from(edge: Edge) -> Self {
        Value::Edge(edge)
    }
}

impl From<TemporalValue> for Value {
    fn from(tv: TemporalValue) -> Self {
        Value::Temporal(tv)
    }
}

impl Hash for TimeWindow {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.start.timestamp().hash(state);
        self.start.timestamp_subsec_nanos().hash(state);
        self.end.timestamp().hash(state);
        self.end.timestamp_subsec_nanos().hash(state);
    }
}

impl Hash for TemporalValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
        self.valid_from.timestamp().hash(state);
        self.valid_from.timestamp_subsec_nanos().hash(state);
        if let Some(vt) = self.valid_to {
            1u8.hash(state);
            vt.timestamp().hash(state);
            vt.timestamp_subsec_nanos().hash(state);
        } else {
            0u8.hash(state);
        }
        self.transaction_time.timestamp().hash(state);
        self.transaction_time.timestamp_subsec_nanos().hash(state);
    }
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Null => 0.hash(state),
            Value::Boolean(b) => {
                1.hash(state);
                b.hash(state);
            },
            Value::Number(n) => {
                2.hash(state);
                // Handle NaN and infinity consistently
                if n.is_nan() {
                    "NaN".hash(state);
                } else if n.is_infinite() {
                    if n.is_sign_positive() {
                        "Infinity".hash(state);
                    } else {
                        "-Infinity".hash(state);
                    }
                } else {
                    n.to_bits().hash(state);
                }
            },
            Value::String(s) => {
                3.hash(state);
                s.hash(state);
            },
            Value::DateTime(dt) => {
                4.hash(state);
                dt.timestamp().hash(state);
                dt.timestamp_subsec_nanos().hash(state);
            },
            Value::TimeWindow(tw) => {
                5.hash(state);
                tw.hash(state);
            },
            Value::Array(arr) => {
                6.hash(state);
                arr.len().hash(state);
                for item in arr {
                    item.hash(state);
                }
            },
            Value::List(list) => {
                11.hash(state); // Use unique discriminant
                list.len().hash(state);
                for item in list {
                    item.hash(state);
                }
            },
            Value::Vector(vec) => {
                7.hash(state);
                vec.len().hash(state);
                for &item in vec {
                    item.to_bits().hash(state);
                }
            },
            Value::DateTimeWithFixedOffset(dt) => {
                8.hash(state);
                dt.timestamp().hash(state);
                dt.timestamp_subsec_nanos().hash(state);
                dt.offset().local_minus_utc().hash(state);
            },
            Value::DateTimeWithNamedTz(tz_name, dt) => {
                9.hash(state);
                tz_name.hash(state);
                dt.timestamp().hash(state);
                dt.timestamp_subsec_nanos().hash(state);
            },
            Value::Path(path) => {
                10.hash(state);
                path.elements.len().hash(state);
                for element in &path.elements {
                    element.node_id.hash(state);
                    element.edge_id.hash(state);
                }
            },
            Value::Node(node) => {
                12.hash(state);
                node.id.hash(state);
                node.labels.hash(state);
                node.properties.len().hash(state);
                for (key, value) in &node.properties {
                    key.hash(state);
                    value.hash(state);
                }
            },
            Value::Edge(edge) => {
                13.hash(state);
                edge.id.hash(state);
                edge.from_node.hash(state);
                edge.to_node.hash(state);
                edge.label.hash(state);
                edge.properties.len().hash(state);
                for (key, value) in &edge.properties {
                    key.hash(state);
                    value.hash(state);
                }
            },
            Value::Temporal(tv) => {
                14.hash(state);
                tv.hash(state);
            },
        }
    }
}

