/**
 * QueryResult wrapper for GraphLite query results
 * 
 * Provides convenient access methods for query results returned from GraphLite.
 */

const { GraphLiteError, ErrorCode } = require('./errors');

/**
 * Wrapper for query results with convenient access methods
 */
class QueryResult {
  /**
   * Create a QueryResult from JSON data
   * @param {Object} data - Parsed JSON result from FFI
   */
  constructor(data) {
    if (!data || typeof data !== 'object') {
      throw new GraphLiteError(
        ErrorCode.JSON_ERROR,
        'Invalid query result data'
      );
    }

    this._data = data;
    this.variables = data.variables || [];
    
    // Flatten rows from nested structure
    const rawRows = data.rows || [];
    this.rows = rawRows.map(row => this._flattenRow(row));
    this.rowCount = this.rows.length;
  }

  /**
   * Flatten a row from nested value structure to simple object
   * @param {Object} row - Raw row from FFI
   * @returns {Object} Flattened row
   * @private
   */
  _flattenRow(row) {
    if (!row || typeof row !== 'object') {
      return row;
    }

    // If row doesn't have 'values', return as-is
    if (!row.values) {
      return row;
    }

    const flattened = {};
    for (const [key, valueWrapper] of Object.entries(row.values)) {
      flattened[key] = this._extractValue(valueWrapper);
    }
    return flattened;
  }

  /**
   * Extract value from Rust enum wrapper
   * Handles formats like {'String': 'foo'}, {'Number': 42}, etc.
   * @param {*} valueWrapper - Wrapped value from FFI
   * @returns {*} Extracted value
   * @private
   */
  _extractValue(valueWrapper) {
    // If not an object, return as-is
    if (!valueWrapper || typeof valueWrapper !== 'object') {
      return valueWrapper;
    }

    // Handle Rust enum variants
    if ('String' in valueWrapper) {
      return valueWrapper.String;
    }
    
    if ('Number' in valueWrapper) {
      const num = valueWrapper.Number;
      // Convert to integer if it's a whole number
      return Number.isInteger(num) ? num : num;
    }
    
    if ('Boolean' in valueWrapper) {
      return valueWrapper.Boolean;
    }
    
    if ('Null' in valueWrapper) {
      return null;
    }
    
    if ('List' in valueWrapper) {
      return valueWrapper.List.map(v => this._extractValue(v));
    }
    
    if ('Map' in valueWrapper) {
      const extracted = {};
      for (const [k, v] of Object.entries(valueWrapper.Map)) {
        extracted[k] = this._extractValue(v);
      }
      return extracted;
    }
    
    // Return complex types (Node, Edge, Path) as-is
    if ('Node' in valueWrapper || 'Edge' in valueWrapper || 'Path' in valueWrapper) {
      return valueWrapper;
    }

    // Unknown format, return as-is
    return valueWrapper;
  }

  /**
   * Get the first row or null if no rows
   * @returns {Object|null} First row or null
   */
  first() {
    return this.rows.length > 0 ? this.rows[0] : null;
  }

  /**
   * Get all values from a specific column
   * @param {string} columnName - Column/variable name
   * @returns {Array} Array of column values
   */
  column(columnName) {
    return this.rows.map(row => row[columnName]);
  }

  /**
   * Convert to array of rows
   * @returns {Array<Object>} Array of row objects
   */
  toArray() {
    return this.rows;
  }

  /**
   * Get raw dictionary representation
   * @returns {Object} Raw JSON data
   */
  toJSON() {
    return this._data;
  }

  /**
   * Get a subset of columns from all rows
   * @param {...string} columnNames - Column names to select
   * @returns {Array<Object>} Array of objects with selected columns
   */
  select(...columnNames) {
    return this.rows.map(row => {
      const selected = {};
      for (const col of columnNames) {
        if (col in row) {
          selected[col] = row[col];
        }
      }
      return selected;
    });
  }

  /**
   * Filter rows based on a predicate function
   * @param {Function} predicate - Function that returns true for rows to keep
   * @returns {Array<Object>} Filtered rows
   */
  filter(predicate) {
    return this.rows.filter(predicate);
  }

  /**
   * Map rows using a transformer function
   * @param {Function} transformer - Function to transform each row
   * @returns {Array} Transformed rows
   */
  map(transformer) {
    return this.rows.map(transformer);
  }

  /**
   * Check if result has any rows
   * @returns {boolean}
   */
  isEmpty() {
    return this.rowCount === 0;
  }

  /**
   * Get string representation
   * @returns {string}
   */
  toString() {
    return `QueryResult(rows=${this.rowCount}, variables=[${this.variables.join(', ')}])`;
  }

  /**
   * Iterate over rows (makes QueryResult iterable)
   */
  [Symbol.iterator]() {
    return this.rows[Symbol.iterator]();
  }
}

module.exports = QueryResult;
