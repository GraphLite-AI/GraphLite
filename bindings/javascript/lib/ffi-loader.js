/**
 * FFI Loader for GraphLite
 * 
 * Locates and loads the GraphLite FFI shared library across different platforms.
 */

const koffi = require('koffi');
const path = require('path');
const fs = require('fs');
const os = require('os');

/**
 * Find the GraphLite FFI library based on platform
 * @returns {string} Path to the library
 * @throws {Error} If library cannot be found
 */
function findLibrary() {
  const platform = os.platform();
  let libName;
  
  // Determine library name by platform
  if (platform === 'darwin') {
    libName = 'libgraphlite_ffi.dylib';
  } else if (platform === 'win32') {
    libName = 'graphlite_ffi.dll';
  } else {
    // Linux and others
    libName = 'libgraphlite_ffi.so';
  }

  // Search paths (relative to this file and common locations)
  const searchPaths = [
    // Relative to bindings/javascript (development)
    path.join(__dirname, '..', '..', '..', 'target', 'release', libName),
    path.join(__dirname, '..', '..', '..', 'target', 'debug', libName),
    // Installed locations
    path.join('/usr', 'local', 'lib', libName),
    path.join('/usr', 'lib', libName),
    // Current working directory
    path.join(process.cwd(), libName),
    // Same directory as this module
    path.join(__dirname, libName),
  ];

  // Check environment variable
  if (process.env.GRAPHLITE_FFI_LIB) {
    searchPaths.unshift(process.env.GRAPHLITE_FFI_LIB);
  }

  for (const libPath of searchPaths) {
    if (fs.existsSync(libPath)) {
      return libPath;
    }
  }

  throw new Error(
    `Could not find GraphLite FFI library (${libName}). ` +
    `Please build it first: cargo build --release -p graphlite-ffi. ` +
    `Or set GRAPHLITE_FFI_LIB environment variable to the library path.`
  );
}

/**
 * Load the GraphLite FFI library
 * @returns {Object} FFI library with function definitions
 */
function loadLibrary() {
  const libPath = findLibrary();
  
  try {
    const lib = koffi.load(libPath);
    
    // Define function signatures
    return {
      graphlite_open: lib.func('graphlite_open', 'void*', ['string', 'int*']),
      graphlite_create_session: lib.func('graphlite_create_session', 'string', ['void*', 'string', 'int*']),
      graphlite_query: lib.func('graphlite_query', 'string', ['void*', 'string', 'string', 'int*']),
      graphlite_close_session: lib.func('graphlite_close_session', 'int', ['void*', 'string', 'int*']),
      graphlite_close: lib.func('graphlite_close', 'void', ['void*']),
      graphlite_free_string: lib.func('graphlite_free_string', 'void', ['string']),
      graphlite_version: lib.func('graphlite_version', 'string', []),
    };
  } catch (error) {
    throw new Error(
      `Failed to load GraphLite FFI library from ${libPath}: ${error.message}`
    );
  }
}

module.exports = {
  loadLibrary,
  findLibrary,
};
