// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Temporal function implementations for DATETIME, NOW, DURATION, and TIME_WINDOW
//! 
//! This module provides specialized temporal function execution with proper ISO 8601
//! datetime and duration processing, enabling temporal queries and arithmetic operations.
//! 
//! # Supported Functions
//! 
//! ## DATETIME(string)
//! Parses an ISO 8601 datetime string into a DateTime<Utc> value.
//! - Input: ISO 8601 datetime string (e.g., "2024-01-15T10:30:45Z")
//! - Output: DateTime value
//! - Error: Invalid datetime format
//! 
//! ## NOW()
//! Returns the current system time as DateTime<Utc>.
//! - Input: None
//! - Output: Current DateTime value
//! - Error: None (system time always available)
//! 
//! ## DURATION(string)
//! Parses an ISO 8601 duration string into a Duration value.
//! - Input: ISO 8601 duration string (e.g., "P1Y2M3DT4H5M6S", "PT1H", "P1D")
//! - Output: Duration value (represented as seconds)
//! - Error: Invalid duration format
//! 
//! # Usage Examples
//! 
//! ```gql
//! -- Parse specific datetime
//! MATCH (event:Event)
//! WHERE event.timestamp >= DATETIME('2024-01-01T00:00:00Z')
//! RETURN event
//! 
//! -- Get current time
//! MATCH (user:User)
//! RETURN user.name, NOW() as current_time
//! 
//! -- Create duration for comparisons
//! MATCH (session:Session)
//! WHERE session.duration <= DURATION('PT1H')  -- 1 hour
//! RETURN session
//! ```

use super::function_trait::{Function, FunctionError, FunctionResult, FunctionContext};
use crate::storage::{Value, TimeWindow};
use chrono::{DateTime, Utc, Datelike, Timelike, FixedOffset, Offset};
use chrono_tz::Tz;
use log::{debug, warn};

/// Timezone information for preserving original timezone in calculations
#[derive(Debug, Clone)]
enum TimezoneInfo {
    Named(Tz),
    Fixed(FixedOffset),
}

impl TimezoneInfo {
    /// Convert UTC datetime to this timezone, handling DST properly
    fn convert_from_utc(&self, utc_dt: DateTime<Utc>) -> Value {
        match self {
            TimezoneInfo::Named(tz) => {
                let _local_dt = utc_dt.with_timezone(tz);
                Value::DateTimeWithNamedTz(tz.to_string(), utc_dt)
            }
            TimezoneInfo::Fixed(offset) => {
                let offset_dt = utc_dt.with_timezone(offset);
                Value::DateTimeWithFixedOffset(offset_dt)
            }
        }
    }
    
    /// Perform DST-aware arithmetic by working in the local timezone
    fn add_duration_dst_aware(&self, utc_dt: DateTime<Utc>, interval: i64, unit: &str) -> Result<DateTime<Utc>, String> {
        match self {
            TimezoneInfo::Named(tz) => {
                let local_dt = utc_dt.with_timezone(tz);
                
                // Perform arithmetic in local timezone to handle DST
                let result_local = match unit {
                    "SECOND" | "SECONDS" => local_dt + chrono::Duration::seconds(interval),
                    "MINUTE" | "MINUTES" => local_dt + chrono::Duration::minutes(interval),
                    "HOUR" | "HOURS" => local_dt + chrono::Duration::hours(interval),
                    "DAY" | "DAYS" => local_dt + chrono::Duration::days(interval),
                    "WEEK" | "WEEKS" => local_dt + chrono::Duration::weeks(interval),
                    "MONTH" | "MONTHS" => {
                        // Handle month arithmetic in local timezone
                        let mut result = local_dt;
                        for _ in 0..interval.abs() {
                            if interval > 0 {
                                if result.month() == 12 {
                                    result = result.with_year(result.year() + 1)
                                        .unwrap_or(result)
                                        .with_month(1)
                                        .unwrap_or(result);
                                } else {
                                    result = result.with_month(result.month() + 1).unwrap_or(result);
                                }
                            } else {
                                if result.month() == 1 {
                                    result = result.with_year(result.year() - 1)
                                        .unwrap_or(result)
                                        .with_month(12)
                                        .unwrap_or(result);
                                } else {
                                    result = result.with_month(result.month() - 1).unwrap_or(result);
                                }
                            }
                        }
                        result
                    }
                    "YEAR" | "YEARS" => {
                        let new_year = local_dt.year() + interval as i32;
                        local_dt.with_year(new_year).unwrap_or(local_dt)
                    }
                    _ => return Err(format!("Unsupported unit: {}", unit)),
                };
                
                // Convert back to UTC
                Ok(result_local.with_timezone(&Utc))
            }
            TimezoneInfo::Fixed(offset) => {
                // For fixed offset, DST doesn't apply, so we can do simple arithmetic
                let offset_dt = utc_dt.with_timezone(offset);
                
                let result_offset = match unit {
                    "SECOND" | "SECONDS" => offset_dt + chrono::Duration::seconds(interval),
                    "MINUTE" | "MINUTES" => offset_dt + chrono::Duration::minutes(interval),
                    "HOUR" | "HOURS" => offset_dt + chrono::Duration::hours(interval),
                    "DAY" | "DAYS" => offset_dt + chrono::Duration::days(interval),
                    "WEEK" | "WEEKS" => offset_dt + chrono::Duration::weeks(interval),
                    "MONTH" | "MONTHS" => {
                        let mut result = offset_dt;
                        for _ in 0..interval.abs() {
                            if interval > 0 {
                                if result.month() == 12 {
                                    result = result.with_year(result.year() + 1)
                                        .unwrap_or(result)
                                        .with_month(1)
                                        .unwrap_or(result);
                                } else {
                                    result = result.with_month(result.month() + 1).unwrap_or(result);
                                }
                            } else {
                                if result.month() == 1 {
                                    result = result.with_year(result.year() - 1)
                                        .unwrap_or(result)
                                        .with_month(12)
                                        .unwrap_or(result);
                                } else {
                                    result = result.with_month(result.month() - 1).unwrap_or(result);
                                }
                            }
                        }
                        result
                    }
                    "YEAR" | "YEARS" => {
                        let new_year = offset_dt.year() + interval as i32;
                        offset_dt.with_year(new_year).unwrap_or(offset_dt)
                    }
                    _ => return Err(format!("Unsupported unit: {}", unit)),
                };
                
                Ok(result_offset.with_timezone(&Utc))
            }
        }
    }
}

/// DATETIME function - parses ISO 8601 datetime strings
#[derive(Debug)]
pub struct DateTimeFunction;

impl DateTimeFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for DateTimeFunction {
    fn name(&self) -> &str {
        "DATETIME"
    }
    
    fn description(&self) -> &str {
        "Parse ISO 8601 datetime string into DateTime value"
    }
    
    fn argument_count(&self) -> usize {
        1
    }
    
    fn return_type(&self) -> &str {
        "DateTime"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing DATETIME function");
        
        // Validate argument count
        context.validate_argument_count(1)?;
        
        // Get the datetime string argument
        let datetime_str = match context.get_argument(0)? {
            Value::String(s) => s,
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "DATETIME argument must be a string".to_string() 
            }),
        };
        
        // Parse the ISO 8601 datetime string
        match parse_iso_datetime(datetime_str) {
            Ok(datetime) => {
                debug!("Successfully parsed datetime: {} -> {}", datetime_str, datetime);
                Ok(Value::DateTime(datetime))
            }
            Err(e) => {
                warn!("Failed to parse datetime '{}': {}", datetime_str, e);
                Err(FunctionError::ExecutionError { 
                    message: format!("Invalid datetime format '{}': {}", datetime_str, e) 
                })
            }
        }
    }
}

/// NOW function - returns current system time
#[derive(Debug)]
pub struct NowFunction;

impl NowFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for NowFunction {
    fn name(&self) -> &str {
        "NOW"
    }
    
    fn description(&self) -> &str {
        "Return current system time as DateTime value"
    }
    
    fn argument_count(&self) -> usize {
        0
    }
    
    fn return_type(&self) -> &str {
        "DateTime"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing NOW function");
        
        // Validate argument count (should be 0)
        context.validate_argument_count(0)?;
        
        // Get current system time
        let current_time = Utc::now();
        debug!("Current system time: {}", current_time);
        
        Ok(Value::DateTime(current_time))
    }
}

/// DURATION function - parses ISO 8601 duration strings
#[derive(Debug)]
pub struct DurationFunction;

impl DurationFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for DurationFunction {
    fn name(&self) -> &str {
        "DURATION"
    }
    
    fn description(&self) -> &str {
        "Parse ISO 8601 duration string into Duration value (represented as seconds)"
    }
    
    fn argument_count(&self) -> usize {
        1
    }
    
    fn return_type(&self) -> &str {
        "Number"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing DURATION function");
        
        // Validate argument count
        context.validate_argument_count(1)?;
        
        // Get the duration string argument
        let duration_str = match context.get_argument(0)? {
            Value::String(s) => s,
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "DURATION argument must be a string".to_string() 
            }),
        };
        
        // Parse the ISO 8601 duration string
        match parse_iso_duration(duration_str) {
            Ok(duration_seconds) => {
                debug!("Successfully parsed duration: {} -> {} seconds", duration_str, duration_seconds);
                // Return duration as number of seconds for now
                // In the future, we might want a dedicated Duration value type
                Ok(Value::Number(duration_seconds as f64))
            }
            Err(e) => {
                warn!("Failed to parse duration '{}': {}", duration_str, e);
                Err(FunctionError::ExecutionError { 
                    message: format!("Invalid duration format '{}': {}", duration_str, e) 
                })
            }
        }
    }
}

/// DURATION function - numeric variant for DURATION(number, unit)
#[derive(Debug)]
pub struct DurationNumericFunction;

impl DurationNumericFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for DurationNumericFunction {
    fn name(&self) -> &str {
        "DURATION_NUMERIC"
    }
    
    fn description(&self) -> &str {
        "Create duration from numeric value and temporal unit (e.g., DURATION(30, 'MINUTES'))"
    }
    
    fn argument_count(&self) -> usize {
        2
    }
    
    fn return_type(&self) -> &str {
        "Number"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing DURATION_NUMERIC function");
        
        // Validate argument count
        context.validate_argument_count(2)?;
        
        // Get the numeric value
        let number = match context.get_argument(0)? {
            Value::Number(n) => *n,
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "First DURATION argument must be a number".to_string() 
            }),
        };
        
        // Get the unit string
        let unit_str = match context.get_argument(1)? {
            Value::String(s) => s,
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "Second DURATION argument must be a temporal unit string".to_string() 
            }),
        };
        
        // Convert to seconds based on unit
        match parse_numeric_duration(number, unit_str) {
            Ok(duration_seconds) => {
                debug!("Successfully parsed numeric duration: {} {} -> {} seconds", number, unit_str, duration_seconds);
                Ok(Value::Number(duration_seconds as f64))
            }
            Err(e) => {
                warn!("Failed to parse numeric duration '{}': {}", unit_str, e);
                Err(FunctionError::ExecutionError { 
                    message: format!("Invalid temporal unit '{}': {}", unit_str, e) 
                })
            }
        }
    }
}

/// CURRENT_DATE function - returns current date only (without time component)
#[derive(Debug)]
pub struct CurrentDateFunction;

impl CurrentDateFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for CurrentDateFunction {
    fn name(&self) -> &str {
        "CURRENT_DATE"
    }
    
    fn description(&self) -> &str {
        "Return current date only (without time component)"
    }
    
    fn argument_count(&self) -> usize {
        0
    }
    
    fn return_type(&self) -> &str {
        "String"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing CURRENT_DATE function");
        
        // Validate argument count (should be 0)
        context.validate_argument_count(0)?;
        
        // Get current date and format as date-only string
        let current_date = Utc::now().format("%Y-%m-%d").to_string();
        debug!("Current date: {}", current_date);
        
        Ok(Value::String(current_date))
    }
}

/// CURRENT_TIME function - returns current time only (without date component)
#[derive(Debug)]
pub struct CurrentTimeFunction;

impl CurrentTimeFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for CurrentTimeFunction {
    fn name(&self) -> &str {
        "CURRENT_TIME"
    }
    
    fn description(&self) -> &str {
        "Return current time only (without date component)"
    }
    
    fn argument_count(&self) -> usize {
        0
    }
    
    fn return_type(&self) -> &str {
        "String"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing CURRENT_TIME function");
        
        // Validate argument count (should be 0)
        context.validate_argument_count(0)?;
        
        // Get current time and format as time-only string
        let current_time = Utc::now().format("%H:%M:%S").to_string();
        debug!("Current time: {}", current_time);
        
        Ok(Value::String(current_time))
    }
}

/// EXTRACT function - extracts parts from datetime values
#[derive(Debug)]
pub struct ExtractFunction;

impl ExtractFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ExtractFunction {
    fn name(&self) -> &str {
        "EXTRACT"
    }
    
    fn description(&self) -> &str {
        "Extract specific parts from datetime values (e.g., EXTRACT('YEAR', datetime))"
    }
    
    fn argument_count(&self) -> usize {
        2
    }
    
    fn return_type(&self) -> &str {
        "Number"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing EXTRACT function");
        
        // Validate argument count
        context.validate_argument_count(2)?;
        
        // Get the unit to extract
        let unit = match context.get_argument(0)? {
            Value::String(s) => s.to_uppercase(),
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "EXTRACT unit argument must be a string".to_string() 
            }),
        };
        
        // Get the datetime value and timezone info
        let (datetime, timezone_value) = match context.get_argument(1)? {
            Value::DateTime(dt) => (*dt, context.get_argument(1)?),
            Value::DateTimeWithFixedOffset(dt) => (dt.with_timezone(&Utc), context.get_argument(1)?),
            Value::DateTimeWithNamedTz(_, dt) => (*dt, context.get_argument(1)?),
            Value::String(s) => {
                // Try to parse string as datetime
                match parse_iso_datetime(s) {
                    Ok(dt) => (dt, &Value::DateTime(dt)),
                    Err(e) => return Err(FunctionError::ExecutionError {
                        message: format!("Invalid datetime string '{}': {}", s, e)
                    })
                }
            }
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "EXTRACT datetime argument must be a DateTime or datetime string".to_string() 
            }),
        };
        
        // Extract the requested component
        let result = match unit.as_str() {
            "YEAR" => Value::Number(datetime.year() as f64),
            "MONTH" => Value::Number(datetime.month() as f64),
            "DAY" => Value::Number(datetime.day() as f64),
            "HOUR" => Value::Number(datetime.hour() as f64),
            "MINUTE" => Value::Number(datetime.minute() as f64),
            "SECOND" => Value::Number(datetime.second() as f64),
            "DOW" | "DAYOFWEEK" => Value::Number(datetime.weekday().num_days_from_sunday() as f64),
            "DOY" | "DAYOFYEAR" => Value::Number(datetime.ordinal() as f64),
            "WEEK" => {
                // ISO week number
                Value::Number(datetime.iso_week().week() as f64)
            }
            "QUARTER" => {
                // Calculate quarter (1-4)
                Value::Number(((datetime.month() - 1) / 3 + 1) as f64)
            }
            "EPOCH" => {
                // Return Unix timestamp
                Value::Number(datetime.timestamp() as f64)
            }
            // Timezone-specific extractions
            "TIMEZONE" => {
                // Return timezone name/identifier
                match timezone_value.get_timezone_info() {
                    Some(tz_info) => Value::String(tz_info),
                    None => return Err(FunctionError::ExecutionError {
                        message: "Datetime has no timezone information".to_string()
                    }),
                }
            }
            "TIMEZONE_HOUR" => {
                // Return timezone offset in hours
                match timezone_value {
                    Value::DateTime(_) => Value::Number(0.0), // UTC is +0
                    Value::DateTimeWithFixedOffset(dt) => {
                        let offset_seconds = dt.offset().local_minus_utc();
                        Value::Number(offset_seconds as f64 / 3600.0)
                    }
                    Value::DateTimeWithNamedTz(tz_name, _) => {
                        if let Ok(tz) = tz_name.parse::<Tz>() {
                            let tz_dt = datetime.with_timezone(&tz);
                            let offset_seconds = tz_dt.offset().fix().local_minus_utc();
                            Value::Number(offset_seconds as f64 / 3600.0)
                        } else {
                            return Err(FunctionError::ExecutionError {
                                message: format!("Invalid timezone name: {}", tz_name)
                            });
                        }
                    }
                    _ => return Err(FunctionError::InvalidArgumentType {
                        message: "Argument must be a datetime".to_string()
                    }),
                }
            }
            "TIMEZONE_MINUTE" => {
                // Return timezone offset minutes component
                match timezone_value {
                    Value::DateTime(_) => Value::Number(0.0), // UTC is +0
                    Value::DateTimeWithFixedOffset(dt) => {
                        let offset_seconds = dt.offset().local_minus_utc();
                        let offset_minutes = (offset_seconds % 3600) / 60;
                        Value::Number(offset_minutes as f64)
                    }
                    Value::DateTimeWithNamedTz(tz_name, _) => {
                        if let Ok(tz) = tz_name.parse::<Tz>() {
                            let tz_dt = datetime.with_timezone(&tz);
                            let offset_seconds = tz_dt.offset().fix().local_minus_utc();
                            let offset_minutes = (offset_seconds % 3600) / 60;
                            Value::Number(offset_minutes as f64)
                        } else {
                            return Err(FunctionError::ExecutionError {
                                message: format!("Invalid timezone name: {}", tz_name)
                            });
                        }
                    }
                    _ => return Err(FunctionError::InvalidArgumentType {
                        message: "Argument must be a datetime".to_string()
                    }),
                }
            }
            _ => return Err(FunctionError::ExecutionError {
                message: format!("Unsupported EXTRACT unit: '{}'. Supported units: YEAR, MONTH, DAY, HOUR, MINUTE, SECOND, DOW, DOY, WEEK, QUARTER, EPOCH, TIMEZONE, TIMEZONE_HOUR, TIMEZONE_MINUTE", unit)
            })
        };
        
        debug!("Extracted {} from {}: {:?}", unit, datetime, result);
        Ok(result)
    }
}

/// DATE_ADD function - adds interval to date
#[derive(Debug)]
pub struct DateAddFunction;

impl DateAddFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for DateAddFunction {
    fn name(&self) -> &str {
        "DATE_ADD"
    }
    
    fn description(&self) -> &str {
        "Add interval to date (e.g., DATE_ADD(date, 1, 'DAY'))"
    }
    
    fn argument_count(&self) -> usize {
        3
    }
    
    fn return_type(&self) -> &str {
        "DateTime"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing DATE_ADD function");
        
        // Validate argument count
        context.validate_argument_count(3)?;
        
        // Get the base datetime and preserve timezone info
        let (base_datetime, timezone_info) = match context.get_argument(0)? {
            Value::DateTime(dt) => (*dt, None),
            Value::DateTimeWithFixedOffset(dt) => (dt.with_timezone(&Utc), Some(TimezoneInfo::Fixed(*dt.offset()))),
            Value::DateTimeWithNamedTz(tz_name, dt) => {
                match tz_name.parse::<Tz>() {
                    Ok(tz) => (*dt, Some(TimezoneInfo::Named(tz))),
                    Err(_) => return Err(FunctionError::ExecutionError {
                        message: format!("Invalid timezone name: {}", tz_name)
                    })
                }
            }
            Value::String(s) => {
                match parse_iso_datetime(s) {
                    Ok(dt) => (dt, None),
                    Err(e) => return Err(FunctionError::ExecutionError {
                        message: format!("Invalid datetime string '{}': {}", s, e)
                    })
                }
            }
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "DATE_ADD first argument must be a DateTime or datetime string".to_string() 
            }),
        };
        
        // Get the interval value
        let interval_value = match context.get_argument(1)? {
            Value::Number(n) => *n as i64,
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "DATE_ADD interval value must be a number".to_string() 
            }),
        };
        
        // Get the interval unit
        let unit = match context.get_argument(2)? {
            Value::String(s) => s.to_uppercase(),
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "DATE_ADD interval unit must be a string".to_string() 
            }),
        };
        
        // Perform timezone-aware arithmetic
        let result_utc = if let Some(ref tz_info) = timezone_info {
            // Use DST-aware arithmetic for timezone-aware datetimes
            match tz_info.add_duration_dst_aware(base_datetime, interval_value, &unit) {
                Ok(result) => result,
                Err(e) => return Err(FunctionError::ExecutionError { message: e })
            }
        } else {
            // For UTC datetimes, use simple arithmetic
            match unit.as_str() {
                "SECOND" | "SECONDS" => {
                    base_datetime + chrono::Duration::seconds(interval_value)
                }
                "MINUTE" | "MINUTES" => {
                    base_datetime + chrono::Duration::minutes(interval_value)
                }
                "HOUR" | "HOURS" => {
                    base_datetime + chrono::Duration::hours(interval_value)
                }
                "DAY" | "DAYS" => {
                    base_datetime + chrono::Duration::days(interval_value)
                }
                "WEEK" | "WEEKS" => {
                    base_datetime + chrono::Duration::weeks(interval_value)
                }
                "MONTH" | "MONTHS" => {
                    // For months, we need to handle variable month lengths
                    let mut result = base_datetime;
                    for _ in 0..interval_value.abs() {
                        if interval_value > 0 {
                            if result.month() == 12 {
                                result = result.with_year(result.year() + 1).unwrap_or(result).with_month(1).unwrap_or(result);
                            } else {
                                result = result.with_month(result.month() + 1).unwrap_or(result);
                            }
                        } else {
                            if result.month() == 1 {
                                result = result.with_year(result.year() - 1).unwrap_or(result).with_month(12).unwrap_or(result);
                            } else {
                                result = result.with_month(result.month() - 1).unwrap_or(result);
                            }
                        }
                    }
                    result
                }
                "YEAR" | "YEARS" => {
                    let new_year = base_datetime.year() + interval_value as i32;
                    base_datetime.with_year(new_year).unwrap_or(base_datetime)
                }
                _ => return Err(FunctionError::ExecutionError {
                    message: format!("Unsupported DATE_ADD unit: '{}'. Supported units: SECOND, MINUTE, HOUR, DAY, WEEK, MONTH, YEAR", unit)
                })
            }
        };

        debug!("Added {} {} to {}: {}", interval_value, unit, base_datetime, result_utc);
        
        // Return result in original timezone format if it was timezone-aware
        if let Some(tz_info) = &timezone_info {
            Ok(tz_info.convert_from_utc(result_utc))
        } else {
            Ok(Value::DateTime(result_utc))
        }
    }
}

/// DATE_SUB function - subtracts interval from date
#[derive(Debug)]
pub struct DateSubFunction;

impl DateSubFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for DateSubFunction {
    fn name(&self) -> &str {
        "DATE_SUB"
    }
    
    fn description(&self) -> &str {
        "Subtract interval from date (e.g., DATE_SUB(date, 1, 'DAY'))"
    }
    
    fn argument_count(&self) -> usize {
        3
    }
    
    fn return_type(&self) -> &str {
        "DateTime"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing DATE_SUB function");
        
        // Validate argument count
        context.validate_argument_count(3)?;
        
        // Get the base datetime
        let base_datetime = match context.get_argument(0)? {
            Value::DateTime(dt) => *dt,
            Value::String(s) => {
                match parse_iso_datetime(s) {
                    Ok(dt) => dt,
                    Err(e) => return Err(FunctionError::ExecutionError {
                        message: format!("Invalid datetime string '{}': {}", s, e)
                    })
                }
            }
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "DATE_SUB first argument must be a DateTime or datetime string".to_string() 
            }),
        };
        
        // Get the interval value
        let interval_value = match context.get_argument(1)? {
            Value::Number(n) => *n as i64,
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "DATE_SUB interval value must be a number".to_string() 
            }),
        };
        
        // Get the interval unit
        let unit = match context.get_argument(2)? {
            Value::String(s) => s.to_uppercase(),
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "DATE_SUB interval unit must be a string".to_string() 
            }),
        };
        
        // Subtract the interval based on unit (same logic as DATE_ADD but with negated interval)
        let result_datetime = match unit.as_str() {
            "SECOND" | "SECONDS" => {
                base_datetime - chrono::Duration::seconds(interval_value)
            }
            "MINUTE" | "MINUTES" => {
                base_datetime - chrono::Duration::minutes(interval_value)
            }
            "HOUR" | "HOURS" => {
                base_datetime - chrono::Duration::hours(interval_value)
            }
            "DAY" | "DAYS" => {
                base_datetime - chrono::Duration::days(interval_value)
            }
            "WEEK" | "WEEKS" => {
                base_datetime - chrono::Duration::weeks(interval_value)
            }
            "MONTH" | "MONTHS" => {
                // For months, we need to handle variable month lengths
                let mut result = base_datetime;
                for _ in 0..interval_value.abs() {
                    if interval_value > 0 {
                        if result.month() == 1 {
                            result = result.with_year(result.year() - 1).unwrap_or(result).with_month(12).unwrap_or(result);
                        } else {
                            result = result.with_month(result.month() - 1).unwrap_or(result);
                        }
                    } else {
                        if result.month() == 12 {
                            result = result.with_year(result.year() + 1).unwrap_or(result).with_month(1).unwrap_or(result);
                        } else {
                            result = result.with_month(result.month() + 1).unwrap_or(result);
                        }
                    }
                }
                result
            }
            "YEAR" | "YEARS" => {
                let new_year = base_datetime.year() - interval_value as i32;
                base_datetime.with_year(new_year).unwrap_or(base_datetime)
            }
            _ => return Err(FunctionError::ExecutionError {
                message: format!("Unsupported DATE_SUB unit: '{}'. Supported units: SECOND, MINUTE, HOUR, DAY, WEEK, MONTH, YEAR", unit)
            })
        };
        
        debug!("Subtracted {} {} from {}: {}", interval_value, unit, base_datetime, result_datetime);
        Ok(Value::DateTime(result_datetime))
    }
}

/// TIME_WINDOW function - creates time windows for temporal range operations  
#[derive(Debug)]
pub struct TimeWindowFunction;

impl TimeWindowFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for TimeWindowFunction {
    fn name(&self) -> &str {
        "TIME_WINDOW"
    }
    
    fn description(&self) -> &str {
        "Create time window for temporal range operations (e.g., TIME_WINDOW('2024-01-01T00:00:00Z', '2024-01-31T23:59:59Z'))"
    }
    
    fn argument_count(&self) -> usize {
        2
    }
    
    fn return_type(&self) -> &str {
        "TimeWindow"
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing TIME_WINDOW function");
        
        // Validate argument count
        context.validate_argument_count(2)?;
        
        // Get the start datetime string
        let start_str = match context.get_argument(0)? {
            Value::String(s) => s,
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "TIME_WINDOW start argument must be a datetime string".to_string() 
            }),
        };
        
        // Get the end datetime string
        let end_str = match context.get_argument(1)? {
            Value::String(s) => s,
            _ => return Err(FunctionError::InvalidArgumentType { 
                message: "TIME_WINDOW end argument must be a datetime string".to_string() 
            }),
        };
        
        // Parse both datetime strings
        let start_dt = match parse_iso_datetime(start_str) {
            Ok(dt) => dt,
            Err(e) => {
                warn!("Failed to parse TIME_WINDOW start datetime '{}': {}", start_str, e);
                return Err(FunctionError::ExecutionError { 
                    message: format!("Invalid start datetime format '{}': {}", start_str, e) 
                });
            }
        };
        
        let end_dt = match parse_iso_datetime(end_str) {
            Ok(dt) => dt,
            Err(e) => {
                warn!("Failed to parse TIME_WINDOW end datetime '{}': {}", end_str, e);
                return Err(FunctionError::ExecutionError { 
                    message: format!("Invalid end datetime format '{}': {}", end_str, e)
                });
            }
        };
        
        // Create the time window
        match TimeWindow::new(start_dt, end_dt) {
            Ok(time_window) => {
                debug!("Successfully created time window: {} to {}", start_str, end_str);
                Ok(Value::TimeWindow(time_window))
            }
            Err(e) => {
                warn!("Failed to create time window from '{}' to '{}': {}", start_str, end_str, e);
                Err(FunctionError::ExecutionError { 
                    message: format!("Invalid time window: {}", e)
                })
            }
        }
    }
}

/// Parse ISO 8601 datetime string into DateTime<Utc>
/// 
/// Supports formats like:
/// - 2024-01-15T10:30:45Z
/// - 2024-01-15T10:30:45.123Z
/// - 2024-01-15T10:30:45+00:00
/// - 2024-01-15T10:30:45.123+00:00
fn parse_iso_datetime(datetime_str: &str) -> Result<DateTime<Utc>, String> {
    // Try different ISO 8601 formats
    let formats = [
        "%Y-%m-%dT%H:%M:%SZ",           // 2024-01-15T10:30:45Z
        "%Y-%m-%dT%H:%M:%S%.3fZ",       // 2024-01-15T10:30:45.123Z
        "%Y-%m-%dT%H:%M:%S%z",          // 2024-01-15T10:30:45+00:00
        "%Y-%m-%dT%H:%M:%S%.3f%z",      // 2024-01-15T10:30:45.123+00:00
        "%Y-%m-%dT%H:%M:%S",            // 2024-01-15T10:30:45 (assume UTC)
    ];
    
    // First try chrono's built-in RFC 3339 parser (most robust)
    if let Ok(datetime) = DateTime::parse_from_rfc3339(datetime_str) {
        return Ok(datetime.with_timezone(&Utc));
    }
    
    // Try manual parsing with different formats
    for format in &formats {
        if let Ok(datetime) = DateTime::parse_from_str(datetime_str, format) {
            return Ok(datetime.with_timezone(&Utc));
        }
    }
    
    // Try parsing as naive datetime and assume UTC
    if let Ok(naive_dt) = chrono::NaiveDateTime::parse_from_str(datetime_str, "%Y-%m-%dT%H:%M:%S") {
        return Ok(DateTime::from_naive_utc_and_offset(naive_dt, Utc));
    }
    
    Err(format!("Unable to parse datetime '{}' - expected ISO 8601 format", datetime_str))
}

/// Parse ISO 8601 duration string into total seconds
/// 
/// Supports formats like:
/// - P1Y2M3DT4H5M6S (1 year, 2 months, 3 days, 4 hours, 5 minutes, 6 seconds)
/// - PT1H (1 hour)
/// - P1D (1 day)
/// - PT30M (30 minutes)
/// - PT45S (45 seconds)
/// 
/// Note: For months and years, we use approximations:
/// - 1 month = 30 days = 2,592,000 seconds
/// - 1 year = 365 days = 31,536,000 seconds
fn parse_iso_duration(duration_str: &str) -> Result<i64, String> {
    if !duration_str.starts_with('P') {
        return Err("Duration must start with 'P'".to_string());
    }
    
    let mut total_seconds = 0i64;
    let mut chars = duration_str[1..].chars().peekable(); // Skip the 'P'
    let mut number_str = String::new();
    let mut in_time_part = false;
    
    while let Some(ch) = chars.next() {
        match ch {
            'T' => {
                in_time_part = true;
                continue;
            }
            '0'..='9' => {
                number_str.push(ch);
            }
            'Y' => {
                if let Ok(years) = number_str.parse::<i64>() {
                    total_seconds += years * 365 * 24 * 3600; // Approximate: 1 year = 365 days
                }
                number_str.clear();
            }
            'M' if !in_time_part => {
                if let Ok(months) = number_str.parse::<i64>() {
                    total_seconds += months * 30 * 24 * 3600; // Approximate: 1 month = 30 days
                }
                number_str.clear();
            }
            'D' => {
                if let Ok(days) = number_str.parse::<i64>() {
                    total_seconds += days * 24 * 3600; // 1 day = 86,400 seconds
                }
                number_str.clear();
            }
            'H' => {
                if let Ok(hours) = number_str.parse::<i64>() {
                    total_seconds += hours * 3600; // 1 hour = 3,600 seconds
                }
                number_str.clear();
            }
            'M' if in_time_part => {
                if let Ok(minutes) = number_str.parse::<i64>() {
                    total_seconds += minutes * 60; // 1 minute = 60 seconds
                }
                number_str.clear();
            }
            'S' => {
                if let Ok(seconds) = number_str.parse::<i64>() {
                    total_seconds += seconds;
                }
                number_str.clear();
            }
            _ => {
                return Err(format!("Invalid duration character: '{}'", ch));
            }
        }
    }
    
    if total_seconds == 0 {
        return Err("Duration must specify at least one time component".to_string());
    }
    
    Ok(total_seconds)
}

/// Parse numeric duration with temporal unit into total seconds
/// 
/// Supports units like:
/// - SECONDS, SECOND, S - seconds
/// - MINUTES, MINUTE, M - minutes (60 seconds each)
/// - HOURS, HOUR, H - hours (3600 seconds each)
/// - DAYS, DAY, D - days (86400 seconds each)
/// - WEEKS, WEEK, W - weeks (604800 seconds each)
/// - MONTHS, MONTH - months (2592000 seconds each, 30 days)
/// - YEARS, YEAR, Y - years (31536000 seconds each, 365 days)
fn parse_numeric_duration(number: f64, unit: &str) -> Result<i64, String> {
    if number < 0.0 {
        return Err("Duration cannot be negative".to_string());
    }
    
    let unit_upper = unit.to_uppercase();
    let multiplier = match unit_upper.as_str() {
        // Seconds
        "SECONDS" | "SECOND" | "S" => 1,
        // Minutes
        "MINUTES" | "MINUTE" | "M" => 60,
        // Hours
        "HOURS" | "HOUR" | "H" => 3600,
        // Days
        "DAYS" | "DAY" | "D" => 86400,
        // Weeks
        "WEEKS" | "WEEK" | "W" => 604800,
        // Months (approximate: 30 days)
        "MONTHS" | "MONTH" => 2592000,
        // Years (approximate: 365 days)
        "YEARS" | "YEAR" | "Y" => 31536000,
        _ => return Err(format!("Unknown temporal unit: '{}'", unit)),
    };
    
    let total_seconds = (number * multiplier as f64) as i64;
    Ok(total_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Value;
    use chrono::Datelike;
    use std::collections::HashMap;
    
    #[test]
    fn test_parse_iso_datetime() {
        // Test various ISO 8601 formats
        let test_cases = vec![
            "2024-01-15T10:30:45Z",
            "2024-01-15T10:30:45.123Z",
            "2024-01-15T10:30:45+00:00",
            "2024-12-31T23:59:59Z",
        ];
        
        for datetime_str in test_cases {
            let result = parse_iso_datetime(datetime_str);
            assert!(result.is_ok(), "Failed to parse valid datetime: {}", datetime_str);
        }
        
        // Test invalid formats
        let invalid_cases = vec![
            "not-a-date",
            "2024-13-15T10:30:45Z", // Invalid month
            "2024-01-32T10:30:45Z", // Invalid day
            "2024-01-15T25:30:45Z", // Invalid hour
        ];
        
        for datetime_str in invalid_cases {
            let result = parse_iso_datetime(datetime_str);
            assert!(result.is_err(), "Should have failed to parse invalid datetime: {}", datetime_str);
        }
    }
    
    #[test]
    fn test_parse_iso_duration() {
        // Test various ISO 8601 duration formats
        let test_cases = vec![
            ("PT1H", 3600),           // 1 hour
            ("PT30M", 1800),          // 30 minutes
            ("PT45S", 45),            // 45 seconds
            ("P1D", 86400),           // 1 day
            ("P1DT1H", 90000),        // 1 day + 1 hour
            ("PT1H30M", 5400),        // 1 hour 30 minutes
            ("P1Y", 31536000),        // 1 year (approximate)
            ("P1M", 2592000),         // 1 month (approximate)
        ];
        
        for (duration_str, expected_seconds) in test_cases {
            let result = parse_iso_duration(duration_str);
            assert!(result.is_ok(), "Failed to parse valid duration: {}", duration_str);
            assert_eq!(result.unwrap(), expected_seconds, "Wrong seconds for duration: {}", duration_str);
        }
        
        // Test invalid formats
        let invalid_cases = vec![
            "not-a-duration",
            "1H",                     // Missing P
            "PT",                     // No components
            "PX",                     // Invalid character
        ];
        
        for duration_str in invalid_cases {
            let result = parse_iso_duration(duration_str);
            assert!(result.is_err(), "Should have failed to parse invalid duration: {}", duration_str);
        }
    }
    
    #[test]
    fn test_datetime_function() {
        let datetime_func = DateTimeFunction::new();
        
        // Test valid datetime
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("2024-01-15T10:30:45Z".to_string())],
        );
        
        let result = datetime_func.execute(&context);
        assert!(result.is_ok());
        
        if let Ok(Value::DateTime(dt)) = result {
            assert_eq!(dt.year(), 2024);
            assert_eq!(dt.month(), 1);
            assert_eq!(dt.day(), 15);
        } else {
            panic!("Expected DateTime value");
        }
        
        // Test invalid argument count
        let context_no_args = FunctionContext::new(vec![], HashMap::new(), vec![]);
        let result = datetime_func.execute(&context_no_args);
        assert!(result.is_err());
        
        // Test invalid argument type
        let context_invalid = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(123.0)],
        );
        let result = datetime_func.execute(&context_invalid);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_now_function() {
        let now_func = NowFunction::new();
        
        // Test NOW with no arguments
        let context = FunctionContext::new(vec![], HashMap::new(), vec![]);
        let result = now_func.execute(&context);
        assert!(result.is_ok());
        
        if let Ok(Value::DateTime(_)) = result {
            // Success - we got a datetime
        } else {
            panic!("Expected DateTime value from NOW()");
        }
        
        // Test NOW with arguments (should fail)
        let context_with_args = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("invalid".to_string())],
        );
        let result = now_func.execute(&context_with_args);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_duration_function() {
        let duration_func = DurationFunction::new();
        
        // Test valid duration
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("PT1H".to_string())],
        );
        
        let result = duration_func.execute(&context);
        assert!(result.is_ok());
        
        if let Ok(Value::Number(seconds)) = result {
            assert_eq!(seconds, 3600.0); // 1 hour = 3600 seconds
        } else {
            panic!("Expected Number value representing seconds");
        }
        
        // Test invalid argument count
        let context_no_args = FunctionContext::new(vec![], HashMap::new(), vec![]);
        let result = duration_func.execute(&context_no_args);
        assert!(result.is_err());
        
        // Test invalid duration format
        let context_invalid = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("not-a-duration".to_string())],
        );
        let result = duration_func.execute(&context_invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_current_date_function() {
        let current_date_func = CurrentDateFunction::new();
        
        // Test CURRENT_DATE with no arguments
        let context = FunctionContext::new(vec![], HashMap::new(), vec![]);
        let result = current_date_func.execute(&context);
        assert!(result.is_ok());
        
        if let Ok(Value::String(date_str)) = result {
            // Should be in YYYY-MM-DD format
            assert!(date_str.len() == 10);
            assert!(date_str.matches('-').count() == 2);
            
            // Should be parseable as a date
            let parts: Vec<&str> = date_str.split('-').collect();
            assert_eq!(parts.len(), 3);
            
            // Year should be 4 digits
            assert_eq!(parts[0].len(), 4);
            assert!(parts[0].parse::<i32>().is_ok());
            
            // Month should be 2 digits, 01-12
            assert_eq!(parts[1].len(), 2);
            let month = parts[1].parse::<u32>().unwrap();
            assert!(month >= 1 && month <= 12);
            
            // Day should be 2 digits, 01-31
            assert_eq!(parts[2].len(), 2);
            let day = parts[2].parse::<u32>().unwrap();
            assert!(day >= 1 && day <= 31);
        } else {
            panic!("Expected String value representing date");
        }
        
        // Test CURRENT_DATE with arguments (should fail)
        let context_with_args = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("invalid".to_string())],
        );
        let result = current_date_func.execute(&context_with_args);
        assert!(result.is_err());
    }

    #[test]
    fn test_current_time_function() {
        let current_time_func = CurrentTimeFunction::new();
        
        // Test CURRENT_TIME with no arguments
        let context = FunctionContext::new(vec![], HashMap::new(), vec![]);
        let result = current_time_func.execute(&context);
        assert!(result.is_ok());
        
        if let Ok(Value::String(time_str)) = result {
            // Should be in HH:MM:SS format
            assert!(time_str.len() == 8);
            assert!(time_str.matches(':').count() == 2);
            
            // Should be parseable as a time
            let parts: Vec<&str> = time_str.split(':').collect();
            assert_eq!(parts.len(), 3);
            
            // Hour should be 00-23
            let hour = parts[0].parse::<u32>().unwrap();
            assert!(hour <= 23);
            
            // Minute should be 00-59
            let minute = parts[1].parse::<u32>().unwrap();
            assert!(minute <= 59);
            
            // Second should be 00-59
            let second = parts[2].parse::<u32>().unwrap();
            assert!(second <= 59);
        } else {
            panic!("Expected String value representing time");
        }
        
        // Test CURRENT_TIME with arguments (should fail)
        let context_with_args = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("invalid".to_string())],
        );
        let result = current_time_func.execute(&context_with_args);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_function() {
        let extract_func = ExtractFunction::new();
        let test_datetime = parse_iso_datetime("2024-03-15T14:30:45Z").unwrap();
        
        // Test various extractions
        let test_cases = vec![
            ("YEAR", 2024.0),
            ("MONTH", 3.0),
            ("DAY", 15.0),
            ("HOUR", 14.0),
            ("MINUTE", 30.0),
            ("SECOND", 45.0),
            ("QUARTER", 1.0), // March is Q1
        ];
        
        for (unit, expected) in test_cases {
            let context = FunctionContext::new(
                vec![],
                HashMap::new(),
                vec![
                    Value::String(unit.to_string()),
                    Value::DateTime(test_datetime)
                ],
            );
            
            let result = extract_func.execute(&context);
            assert!(result.is_ok(), "EXTRACT({}) should succeed", unit);
            
            if let Ok(Value::Number(extracted)) = result {
                assert_eq!(extracted, expected, "EXTRACT({}) should be {}", unit, expected);
            } else {
                panic!("Expected Number value for EXTRACT({})", unit);
            }
        }
        
        // Test with string datetime
        let context_str = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::String("YEAR".to_string()),
                Value::String("2024-03-15T14:30:45Z".to_string())
            ],
        );
        
        let result = extract_func.execute(&context_str);
        assert!(result.is_ok());
        
        if let Ok(Value::Number(year)) = result {
            assert_eq!(year, 2024.0);
        }
        
        // Test invalid unit
        let context_invalid_unit = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::String("INVALID_UNIT".to_string()),
                Value::DateTime(test_datetime)
            ],
        );
        
        let result = extract_func.execute(&context_invalid_unit);
        assert!(result.is_err());
        
        // Test invalid argument count
        let context_invalid_args = FunctionContext::new(vec![], HashMap::new(), vec![]);
        let result = extract_func.execute(&context_invalid_args);
        assert!(result.is_err());
    }

    #[test]
    fn test_date_add_function() {
        let date_add_func = DateAddFunction::new();
        let base_datetime = parse_iso_datetime("2024-01-15T10:30:45Z").unwrap();
        
        // Test various date additions
        let test_cases = vec![
            (1, "DAY", "2024-01-16T10:30:45Z"),
            (7, "DAYS", "2024-01-22T10:30:45Z"),
            (1, "WEEK", "2024-01-22T10:30:45Z"),
            (2, "HOURS", "2024-01-15T12:30:45Z"),
            (30, "MINUTES", "2024-01-15T11:00:45Z"),
            (15, "SECONDS", "2024-01-15T10:31:00Z"),
        ];
        
        for (value, unit, expected_str) in test_cases {
            let context = FunctionContext::new(
                vec![],
                HashMap::new(),
                vec![
                    Value::DateTime(base_datetime),
                    Value::Number(value as f64),
                    Value::String(unit.to_string())
                ],
            );
            
            let result = date_add_func.execute(&context);
            assert!(result.is_ok(), "DATE_ADD({} {}) should succeed", value, unit);
            
            if let Ok(Value::DateTime(result_dt)) = result {
                let expected_dt = parse_iso_datetime(expected_str).unwrap();
                assert_eq!(result_dt, expected_dt, "DATE_ADD({} {}) failed", value, unit);
            } else {
                panic!("Expected DateTime value for DATE_ADD({} {})", value, unit);
            }
        }
        
        // Test with string datetime input
        let context_str = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::String("2024-01-15T10:30:45Z".to_string()),
                Value::Number(1.0),
                Value::String("DAY".to_string())
            ],
        );
        
        let result = date_add_func.execute(&context_str);
        assert!(result.is_ok());
        
        // Test invalid argument count
        let context_invalid = FunctionContext::new(vec![], HashMap::new(), vec![]);
        let result = date_add_func.execute(&context_invalid);
        assert!(result.is_err());
        
        // Test invalid unit
        let context_invalid_unit = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::DateTime(base_datetime),
                Value::Number(1.0),
                Value::String("INVALID_UNIT".to_string())
            ],
        );
        
        let result = date_add_func.execute(&context_invalid_unit);
        assert!(result.is_err());
    }

    #[test]
    fn test_date_sub_function() {
        let date_sub_func = DateSubFunction::new();
        let base_datetime = parse_iso_datetime("2024-01-15T10:30:45Z").unwrap();
        
        // Test various date subtractions
        let test_cases = vec![
            (1, "DAY", "2024-01-14T10:30:45Z"),
            (7, "DAYS", "2024-01-08T10:30:45Z"),
            (1, "WEEK", "2024-01-08T10:30:45Z"),
            (2, "HOURS", "2024-01-15T08:30:45Z"),
            (30, "MINUTES", "2024-01-15T10:00:45Z"),
            (15, "SECONDS", "2024-01-15T10:30:30Z"),
        ];
        
        for (value, unit, expected_str) in test_cases {
            let context = FunctionContext::new(
                vec![],
                HashMap::new(),
                vec![
                    Value::DateTime(base_datetime),
                    Value::Number(value as f64),
                    Value::String(unit.to_string())
                ],
            );
            
            let result = date_sub_func.execute(&context);
            assert!(result.is_ok(), "DATE_SUB({} {}) should succeed", value, unit);
            
            if let Ok(Value::DateTime(result_dt)) = result {
                let expected_dt = parse_iso_datetime(expected_str).unwrap();
                assert_eq!(result_dt, expected_dt, "DATE_SUB({} {}) failed", value, unit);
            } else {
                panic!("Expected DateTime value for DATE_SUB({} {})", value, unit);
            }
        }
        
        // Test with string datetime input
        let context_str = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::String("2024-01-15T10:30:45Z".to_string()),
                Value::Number(1.0),
                Value::String("DAY".to_string())
            ],
        );
        
        let result = date_sub_func.execute(&context_str);
        assert!(result.is_ok());
        
        // Test invalid argument count
        let context_invalid = FunctionContext::new(vec![], HashMap::new(), vec![]);
        let result = date_sub_func.execute(&context_invalid);
        assert!(result.is_err());
        
        // Test invalid unit
        let context_invalid_unit = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::DateTime(base_datetime),
                Value::Number(1.0),
                Value::String("INVALID_UNIT".to_string())
            ],
        );
        
        let result = date_sub_func.execute(&context_invalid_unit);
        assert!(result.is_err());
    }

    #[test]
    fn test_date_add_sub_month_year_handling() {
        let date_add_func = DateAddFunction::new();
        let date_sub_func = DateSubFunction::new();
        
        // Test month addition edge cases
        let base_datetime = parse_iso_datetime("2024-01-31T10:30:45Z").unwrap(); // End of January
        
        let context_add_month = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::DateTime(base_datetime),
                Value::Number(1.0),
                Value::String("MONTH".to_string())
            ],
        );
        
        let result = date_add_func.execute(&context_add_month);
        assert!(result.is_ok(), "DATE_ADD(1 MONTH) should handle end-of-month");
        
        // Test year addition
        let context_add_year = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::DateTime(base_datetime),
                Value::Number(1.0),
                Value::String("YEAR".to_string())
            ],
        );
        
        let result = date_add_func.execute(&context_add_year);
        assert!(result.is_ok(), "DATE_ADD(1 YEAR) should succeed");
        
        if let Ok(Value::DateTime(result_dt)) = result {
            assert_eq!(result_dt.year(), 2025);
            assert_eq!(result_dt.month(), 1);
            assert_eq!(result_dt.day(), 31);
        }
        
        // Test month subtraction
        let base_march = parse_iso_datetime("2024-03-15T10:30:45Z").unwrap();
        
        let context_sub_month = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::DateTime(base_march),
                Value::Number(1.0),
                Value::String("MONTH".to_string())
            ],
        );
        
        let result = date_sub_func.execute(&context_sub_month);
        assert!(result.is_ok(), "DATE_SUB(1 MONTH) should succeed");
        
        if let Ok(Value::DateTime(result_dt)) = result {
            assert_eq!(result_dt.month(), 2); // Should be February
        }
    }
}
