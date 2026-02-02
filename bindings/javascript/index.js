/**
 * GraphLite Node.js Bindings
 * 
 * JavaScript/Node.js bindings for GraphLite embedded graph database.
 * 
 * @module graphlite
 */

const GraphLite = require('./lib/graphlite');
const QueryResult = require('./lib/query-result');
const { GraphLiteError, ErrorCode } = require('./lib/errors');

module.exports = {
  GraphLite,
  QueryResult,
  GraphLiteError,
  ErrorCode,
  
  // Default export
  default: GraphLite,
};
