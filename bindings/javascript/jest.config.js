/**
 * Jest configuration for GraphLite JavaScript bindings
 */

module.exports = {
  testEnvironment: 'node',
  coverageDirectory: 'coverage',
  collectCoverageFrom: [
    'lib/**/*.js',
    '!lib/**/*.test.js',
  ],
  testMatch: [
    '**/test/**/*.test.js',
  ],
  testPathIgnorePatterns: [
    '/node_modules/',
    '/\\.', // Ignore hidden files
  ],
  verbose: true,
};
