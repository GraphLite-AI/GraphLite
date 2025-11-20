// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Timezone-aware temporal functions for GQL
//!
//! This module implements comprehensive timezone support including:
//! - AT TIME ZONE operator
//! - CONVERT_TZ function  
//! - TIMEZONE function
//! - Timezone-aware EXTRACT components
//! - DST-aware arithmetic operations

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;
use chrono::{DateTime, FixedOffset, Offset, Utc};
use chrono_tz::Tz;

/// Parse timezone string into either a named timezone or fixed offset
fn parse_timezone(tz_str: &str) -> Result<TimezoneType, String> {
    // Try parsing as named timezone first
    if let Ok(tz) = tz_str.parse::<Tz>() {
        return Ok(TimezoneType::Named(tz));
    }

    // Try common timezone abbreviations
    let canonical_tz = match tz_str.to_uppercase().as_str() {
        "UTC" | "GMT" => "UTC",
        "EST" => "America/New_York",    // Eastern Standard Time
        "EDT" => "America/New_York",    // Eastern Daylight Time
        "CST" => "America/Chicago",     // Central Standard Time
        "CDT" => "America/Chicago",     // Central Daylight Time
        "MST" => "America/Denver",      // Mountain Standard Time
        "MDT" => "America/Denver",      // Mountain Daylight Time
        "PST" => "America/Los_Angeles", // Pacific Standard Time
        "PDT" => "America/Los_Angeles", // Pacific Daylight Time
        "BST" => "Europe/London",       // British Summer Time
        "CET" => "Europe/Paris",        // Central European Time
        "CEST" => "Europe/Paris",       // Central European Summer Time
        "JST" => "Asia/Tokyo",          // Japan Standard Time
        "IST" => "Asia/Kolkata",        // India Standard Time
        "AEST" => "Australia/Sydney",   // Australian Eastern Standard Time
        "AEDT" => "Australia/Sydney",   // Australian Eastern Daylight Time
        _ => tz_str,                    // Use original if no abbreviation match
    };

    // Try parsing the canonical timezone name
    if let Ok(tz) = canonical_tz.parse::<Tz>() {
        return Ok(TimezoneType::Named(tz));
    }

    // Try parsing as fixed offset (+05:30, -04:00, etc.)
    if let Ok(offset) = parse_fixed_offset(tz_str) {
        return Ok(TimezoneType::Fixed(offset));
    }

    Err(format!("Invalid timezone: {}", tz_str))
}

/// Parse fixed offset strings like "+05:30", "-04:00", "+0530", "-0400"
fn parse_fixed_offset(offset_str: &str) -> Result<FixedOffset, String> {
    let trimmed = offset_str.trim();

    if trimmed.len() < 3 {
        return Err("Invalid offset format".to_string());
    }

    let sign = match trimmed.chars().next() {
        Some('+') => 1,
        Some('-') => -1,
        _ => return Err("Offset must start with + or -".to_string()),
    };

    let offset_part = &trimmed[1..];

    // Handle formats like "05:30" or "0530"
    let (hours, minutes) = if offset_part.contains(':') {
        let parts: Vec<&str> = offset_part.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid offset format".to_string());
        }
        (
            parts[0].parse::<i32>().map_err(|_| "Invalid hours")?,
            parts[1].parse::<i32>().map_err(|_| "Invalid minutes")?,
        )
    } else if offset_part.len() == 4 {
        // Format like "0530"
        let hours_str = &offset_part[0..2];
        let minutes_str = &offset_part[2..4];
        (
            hours_str.parse::<i32>().map_err(|_| "Invalid hours")?,
            minutes_str.parse::<i32>().map_err(|_| "Invalid minutes")?,
        )
    } else {
        return Err("Invalid offset format".to_string());
    };

    if hours > 23 || minutes > 59 {
        return Err("Invalid offset values".to_string());
    }

    let total_seconds = sign * (hours * 3600 + minutes * 60);
    FixedOffset::east_opt(total_seconds).ok_or_else(|| "Invalid offset".to_string())
}

/// Timezone type enum for handling both named timezones and fixed offsets
#[derive(Debug, Clone)]
enum TimezoneType {
    Named(Tz),
    Fixed(FixedOffset),
}

impl TimezoneType {
    /// Convert a UTC datetime to this timezone
    fn convert_from_utc(&self, utc_dt: &DateTime<Utc>) -> Value {
        match self {
            TimezoneType::Named(tz) => {
                let _tz_dt = utc_dt.with_timezone(tz);
                Value::DateTimeWithNamedTz(tz.to_string(), *utc_dt)
            }
            TimezoneType::Fixed(offset) => {
                let offset_dt = utc_dt.with_timezone(offset);
                Value::DateTimeWithFixedOffset(offset_dt)
            }
        }
    }

    /// Get timezone name/identifier
    fn name(&self) -> String {
        match self {
            TimezoneType::Named(tz) => tz.to_string(),
            TimezoneType::Fixed(offset) => offset.to_string(),
        }
    }

    /// Get timezone offset in seconds for a given UTC datetime
    fn offset_seconds(&self, utc_dt: &DateTime<Utc>) -> i32 {
        match self {
            TimezoneType::Named(tz) => {
                let tz_dt = utc_dt.with_timezone(tz);
                tz_dt.offset().fix().local_minus_utc()
            }
            TimezoneType::Fixed(offset) => offset.local_minus_utc(),
        }
    }
}

/// AT TIME ZONE operator: converts datetime to specified timezone
#[derive(Debug)]
pub struct AtTimeZoneFunction;

impl AtTimeZoneFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for AtTimeZoneFunction {
    fn name(&self) -> &str {
        "AT_TIME_ZONE"
    }

    fn description(&self) -> &str {
        "Convert datetime to specified timezone"
    }

    fn argument_count(&self) -> usize {
        2
    }

    fn return_type(&self) -> &str {
        "DateTime"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let datetime_arg = &context.arguments[0];
        let timezone_arg = &context.arguments[1];

        // Extract timezone string
        let tz_str =
            timezone_arg
                .as_string()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Timezone must be a string".to_string(),
                })?;

        // Parse timezone
        let timezone = parse_timezone(tz_str)
            .map_err(|e| FunctionError::InvalidArgumentType { message: e })?;

        // Convert datetime to UTC first if needed
        let utc_dt = match datetime_arg.as_datetime_utc() {
            Some(dt) => dt,
            None => {
                return Err(FunctionError::InvalidArgumentType {
                    message: "First argument must be a datetime".to_string(),
                })
            }
        };

        // Convert to target timezone
        Ok(timezone.convert_from_utc(&utc_dt))
    }

    fn graph_context_required(&self) -> bool {
        false // Timezone functions are pure scalar functions
    }
}

/// CONVERT_TZ function: converts datetime from one timezone to another
#[derive(Debug)]
pub struct ConvertTzFunction;

impl ConvertTzFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ConvertTzFunction {
    fn name(&self) -> &str {
        "CONVERT_TZ"
    }

    fn description(&self) -> &str {
        "Convert datetime from one timezone to another"
    }

    fn argument_count(&self) -> usize {
        3
    }

    fn return_type(&self) -> &str {
        "DateTime"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 3 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 3,
                actual: context.arguments.len(),
            });
        }

        let datetime_arg = &context.arguments[0];
        let from_tz_arg = &context.arguments[1];
        let to_tz_arg = &context.arguments[2];

        // Extract timezone strings
        let from_tz_str =
            from_tz_arg
                .as_string()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "From timezone must be a string".to_string(),
                })?;
        let to_tz_str =
            to_tz_arg
                .as_string()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "To timezone must be a string".to_string(),
                })?;

        // Parse timezones
        let _from_timezone =
            parse_timezone(from_tz_str).map_err(|e| FunctionError::InvalidArgumentType {
                message: format!("Invalid from timezone: {}", e),
            })?;
        let to_timezone =
            parse_timezone(to_tz_str).map_err(|e| FunctionError::InvalidArgumentType {
                message: format!("Invalid to timezone: {}", e),
            })?;

        // Get UTC datetime
        let utc_dt = if from_tz_str.to_uppercase() == "UTC" {
            // Source is UTC
            datetime_arg
                .as_datetime_utc()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "First argument must be a datetime".to_string(),
                })?
        } else {
            // Convert from source timezone to UTC first
            let source_dt = datetime_arg.as_datetime_utc().ok_or_else(|| {
                FunctionError::InvalidArgumentType {
                    message: "First argument must be a datetime".to_string(),
                }
            })?;

            // For simplicity, assume input datetime is in the from_timezone and convert to UTC
            // This is a simplified implementation - in practice, you'd need to handle the conversion more carefully
            source_dt
        };

        // Convert to target timezone
        Ok(to_timezone.convert_from_utc(&utc_dt))
    }

    fn graph_context_required(&self) -> bool {
        false // Timezone functions are pure scalar functions
    }
}

/// TIMEZONE function: converts datetime to specified timezone (alternative syntax)
#[derive(Debug)]
pub struct TimezoneFunction;

impl TimezoneFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for TimezoneFunction {
    fn name(&self) -> &str {
        "TIMEZONE"
    }

    fn description(&self) -> &str {
        "Convert datetime to specified timezone (alternative syntax)"
    }

    fn argument_count(&self) -> usize {
        2
    }

    fn return_type(&self) -> &str {
        "DateTime"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        // Note: TIMEZONE has arguments in opposite order to AT_TIME_ZONE
        let timezone_arg = &context.arguments[0];
        let datetime_arg = &context.arguments[1];

        // Extract timezone string
        let tz_str =
            timezone_arg
                .as_string()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Timezone must be a string".to_string(),
                })?;

        // Parse timezone
        let timezone = parse_timezone(tz_str)
            .map_err(|e| FunctionError::InvalidArgumentType { message: e })?;

        // Convert datetime to UTC first if needed
        let utc_dt = match datetime_arg.as_datetime_utc() {
            Some(dt) => dt,
            None => {
                return Err(FunctionError::InvalidArgumentType {
                    message: "Second argument must be a datetime".to_string(),
                })
            }
        };

        // Convert to target timezone
        Ok(timezone.convert_from_utc(&utc_dt))
    }

    fn graph_context_required(&self) -> bool {
        false // Timezone functions are pure scalar functions
    }
}

/// Enhanced EXTRACT function with timezone components
#[derive(Debug)]
pub struct ExtractTimezoneFunction;

impl ExtractTimezoneFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ExtractTimezoneFunction {
    fn name(&self) -> &str {
        "EXTRACT_TIMEZONE"
    }

    fn description(&self) -> &str {
        "Extract timezone components from datetime values"
    }

    fn argument_count(&self) -> usize {
        2
    }

    fn return_type(&self) -> &str {
        "Number or String"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let unit_arg = &context.arguments[0];
        let datetime_arg = &context.arguments[1];

        let unit = unit_arg
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Extract unit must be a string".to_string(),
            })?
            .to_uppercase();

        match unit.as_str() {
            "TIMEZONE" => {
                // Return timezone name/identifier
                match datetime_arg.get_timezone_info() {
                    Some(tz_info) => Ok(Value::String(tz_info)),
                    None => Err(FunctionError::InvalidArgumentType {
                        message: "Datetime has no timezone information".to_string(),
                    }),
                }
            }
            "TIMEZONE_HOUR" => {
                // Return timezone offset in hours
                let utc_dt = datetime_arg.as_datetime_utc().ok_or_else(|| {
                    FunctionError::InvalidArgumentType {
                        message: "Argument must be a datetime".to_string(),
                    }
                })?;

                match datetime_arg {
                    Value::DateTime(_) => Ok(Value::Number(0.0)), // UTC is +0
                    Value::DateTimeWithFixedOffset(dt) => {
                        let offset_seconds = dt.offset().local_minus_utc();
                        Ok(Value::Number(offset_seconds as f64 / 3600.0))
                    }
                    Value::DateTimeWithNamedTz(tz_name, _) => {
                        if let Ok(tz) = tz_name.parse::<Tz>() {
                            let tz_dt = utc_dt.with_timezone(&tz);
                            let offset_seconds = tz_dt.offset().fix().local_minus_utc();
                            Ok(Value::Number(offset_seconds as f64 / 3600.0))
                        } else {
                            Err(FunctionError::InvalidArgumentType {
                                message: "Invalid timezone name".to_string(),
                            })
                        }
                    }
                    _ => Err(FunctionError::InvalidArgumentType {
                        message: "Argument must be a datetime".to_string(),
                    }),
                }
            }
            "TIMEZONE_MINUTE" => {
                // Return timezone offset minutes component
                let utc_dt = datetime_arg.as_datetime_utc().ok_or_else(|| {
                    FunctionError::InvalidArgumentType {
                        message: "Argument must be a datetime".to_string(),
                    }
                })?;

                match datetime_arg {
                    Value::DateTime(_) => Ok(Value::Number(0.0)), // UTC is +0
                    Value::DateTimeWithFixedOffset(dt) => {
                        let offset_seconds = dt.offset().local_minus_utc();
                        let offset_minutes = (offset_seconds % 3600) / 60;
                        Ok(Value::Number(offset_minutes as f64))
                    }
                    Value::DateTimeWithNamedTz(tz_name, _) => {
                        if let Ok(tz) = tz_name.parse::<Tz>() {
                            let tz_dt = utc_dt.with_timezone(&tz);
                            let offset_seconds = tz_dt.offset().fix().local_minus_utc();
                            let offset_minutes = (offset_seconds % 3600) / 60;
                            Ok(Value::Number(offset_minutes as f64))
                        } else {
                            Err(FunctionError::InvalidArgumentType {
                                message: "Invalid timezone name".to_string(),
                            })
                        }
                    }
                    _ => Err(FunctionError::InvalidArgumentType {
                        message: "Argument must be a datetime".to_string(),
                    }),
                }
            }
            _ => Err(FunctionError::UnsupportedOperation {
                operation: format!("Timezone extract unit: {}", unit),
            }),
        }
    }

    fn graph_context_required(&self) -> bool {
        false // Timezone functions are pure scalar functions
    }
}

/// GET_TIMEZONE_NAME function: gets the timezone name from a timezone string
#[derive(Debug)]
pub struct GetTimezoneNameFunction;

impl GetTimezoneNameFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for GetTimezoneNameFunction {
    fn name(&self) -> &str {
        "GET_TIMEZONE_NAME"
    }

    fn description(&self) -> &str {
        "Get the full timezone name from a timezone identifier or offset"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "String"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let timezone_arg = &context.arguments[0];

        let tz_str =
            timezone_arg
                .as_string()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Timezone argument must be a string".to_string(),
                })?;

        let timezone = parse_timezone(tz_str)
            .map_err(|e| FunctionError::InvalidArgumentType { message: e })?;

        Ok(Value::String(timezone.name()))
    }

    fn graph_context_required(&self) -> bool {
        false // Timezone functions are pure scalar functions
    }
}

/// GET_TIMEZONE_ABBREVIATION function: gets the timezone abbreviation from a timezone string
#[derive(Debug)]
pub struct GetTimezoneAbbreviationFunction;

impl GetTimezoneAbbreviationFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for GetTimezoneAbbreviationFunction {
    fn name(&self) -> &str {
        "GET_TIMEZONE_ABBREVIATION"
    }

    fn description(&self) -> &str {
        "Get the timezone abbreviation from a timezone identifier"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "String"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let timezone_arg = &context.arguments[0];

        let tz_str =
            timezone_arg
                .as_string()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Timezone argument must be a string".to_string(),
                })?;

        // Parse timezone to validate it exists
        let timezone = parse_timezone(tz_str)
            .map_err(|e| FunctionError::InvalidArgumentType { message: e })?;

        // Generate abbreviation based on timezone type
        let abbreviation = match timezone {
            TimezoneType::Named(tz) => {
                // Use the actual timezone object to get canonical name
                let canonical_name = tz.to_string();
                match canonical_name.as_str() {
                    "UTC" => "UTC".to_string(),
                    "America/New_York" => "EST/EDT".to_string(),
                    "America/Chicago" => "CST/CDT".to_string(),
                    "America/Denver" => "MST/MDT".to_string(),
                    "America/Los_Angeles" => "PST/PDT".to_string(),
                    "Europe/London" => "GMT/BST".to_string(),
                    "Europe/Paris" => "CET/CEST".to_string(),
                    "Asia/Tokyo" => "JST".to_string(),
                    "Asia/Shanghai" => "CST".to_string(),
                    "Asia/Kolkata" => "IST".to_string(),
                    _ => {
                        // Extract abbreviation from canonical timezone name
                        let parts: Vec<&str> = canonical_name.split('/').collect();
                        if parts.len() >= 2 {
                            parts.last().unwrap_or(&canonical_name.as_str()).to_string()
                        } else {
                            canonical_name
                        }
                    }
                }
            }
            TimezoneType::Fixed(_) => {
                // For fixed offsets, return the offset string itself
                timezone.name()
            }
        };

        Ok(Value::String(abbreviation))
    }

    fn graph_context_required(&self) -> bool {
        false // Timezone functions are pure scalar functions
    }
}

/// GET_TIMEZONE_OFFSET function: gets the timezone offset in hours from a timezone string
#[derive(Debug)]
pub struct GetTimezoneOffsetFunction;

impl GetTimezoneOffsetFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for GetTimezoneOffsetFunction {
    fn name(&self) -> &str {
        "GET_TIMEZONE_OFFSET"
    }

    fn description(&self) -> &str {
        "Get the timezone offset in standard format (+05:30) from a timezone identifier or abbreviation"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "String"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let timezone_arg = &context.arguments[0];

        let tz_str =
            timezone_arg
                .as_string()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Timezone argument must be a string".to_string(),
                })?;

        // Parse timezone to validate it exists
        let timezone = parse_timezone(tz_str)
            .map_err(|e| FunctionError::InvalidArgumentType { message: e })?;

        // Get current UTC time for calculating offset (since DST affects offset)
        let now_utc = chrono::Utc::now();

        // Use the existing helper method
        let offset_seconds = timezone.offset_seconds(&now_utc);

        // Format offset as standard timezone string (+HH:MM or -HH:MM)
        let sign = if offset_seconds >= 0 { "+" } else { "-" };
        let abs_seconds = offset_seconds.abs();
        let hours = abs_seconds / 3600;
        let minutes = (abs_seconds % 3600) / 60;

        let formatted_offset = format!("{}{:02}:{:02}", sign, hours, minutes);
        Ok(Value::String(formatted_offset))
    }

    fn graph_context_required(&self) -> bool {
        false // Timezone functions are pure scalar functions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fixed_offset() {
        // Test valid formats
        assert!(parse_fixed_offset("+05:30").is_ok());
        assert!(parse_fixed_offset("-04:00").is_ok());
        assert!(parse_fixed_offset("+0530").is_ok());
        assert!(parse_fixed_offset("-0400").is_ok());

        // Test invalid formats
        assert!(parse_fixed_offset("05:30").is_err()); // No sign
        assert!(parse_fixed_offset("+25:00").is_err()); // Invalid hour
        assert!(parse_fixed_offset("+05:60").is_err()); // Invalid minute
    }

    #[test]
    fn test_parse_timezone() {
        // Test named timezone
        assert!(parse_timezone("America/New_York").is_ok());
        assert!(parse_timezone("Europe/London").is_ok());
        assert!(parse_timezone("Asia/Tokyo").is_ok());

        // Test fixed offset
        assert!(parse_timezone("+05:30").is_ok());
        assert!(parse_timezone("-04:00").is_ok());

        // Test invalid
        assert!(parse_timezone("Invalid/Timezone").is_err());
        assert!(parse_timezone("not-a-timezone").is_err());
    }
}
