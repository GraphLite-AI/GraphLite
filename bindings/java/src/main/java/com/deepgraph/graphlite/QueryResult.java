package com.deepgraph.graphlite;

import org.json.JSONArray;
import org.json.JSONObject;

import java.util.*;

/**
 * Query result wrapper with convenient access methods
 */
public class QueryResult {
    private final JSONObject data;
    private final List<String> variables;
    private final List<Map<String, Object>> rows;

    /**
     * Create QueryResult from JSON string
     *
     * @param jsonString JSON result from FFI
     */
    public QueryResult(String jsonString) {
        this.data = new JSONObject(jsonString);
        this.variables = parseVariables();
        this.rows = parseRows();
    }

    private List<String> parseVariables() {
        List<String> vars = new ArrayList<>();
        JSONArray varsArray = data.optJSONArray("variables");
        if (varsArray != null) {
            for (int i = 0; i < varsArray.length(); i++) {
                vars.add(varsArray.getString(i));
            }
        }
        return Collections.unmodifiableList(vars);
    }

    private List<Map<String, Object>> parseRows() {
        List<Map<String, Object>> rowList = new ArrayList<>();
        JSONArray rowsArray = data.optJSONArray("rows");

        if (rowsArray != null) {
            for (int i = 0; i < rowsArray.length(); i++) {
                JSONObject rowObj = rowsArray.getJSONObject(i);
                JSONObject valuesObj = rowObj.optJSONObject("values");

                if (valuesObj == null) {
                    continue;
                }

                Map<String, Object> row = new HashMap<>();
                for (String key : valuesObj.keySet()) {
                    Object value = unwrapValue(valuesObj.get(key));
                    row.put(key, value);
                }
                rowList.add(Collections.unmodifiableMap(row));
            }
        }

        return Collections.unmodifiableList(rowList);
    }

    /**
     * Unwrap type-tagged values from Rust serde JSON format
     * Handles: {"String": "value"}, {"Number": 123}, {"Boolean": true}, etc.
     *
     * @param obj The object to unwrap
     * @return The unwrapped value
     */
    private Object unwrapValue(Object obj) {
        if (!(obj instanceof JSONObject)) {
            return obj;
        }

        JSONObject jsonObj = (JSONObject) obj;

        // Handle type-tagged enum variants from Rust's Value enum
        if (jsonObj.has("String")) {
            return jsonObj.get("String");
        } else if (jsonObj.has("Number")) {
            return jsonObj.get("Number");
        } else if (jsonObj.has("Boolean")) {
            return jsonObj.get("Boolean");
        } else if (jsonObj.has("Null")) {
            return null;
        } else if (jsonObj.has("Array") || jsonObj.has("List")) {
            JSONArray arr = jsonObj.optJSONArray("Array");
            if (arr == null) {
                arr = jsonObj.optJSONArray("List");
            }
            if (arr != null) {
                List<Object> list = new ArrayList<>();
                for (int i = 0; i < arr.length(); i++) {
                    list.add(unwrapValue(arr.get(i)));
                }
                return list;
            }
        } else if (jsonObj.has("DateTime")) {
            // Return DateTime as string for simplicity
            return jsonObj.get("DateTime");
        } else if (jsonObj.has("Vector")) {
            JSONArray arr = jsonObj.optJSONArray("Vector");
            if (arr != null) {
                List<Object> list = new ArrayList<>();
                for (int i = 0; i < arr.length(); i++) {
                    list.add(arr.get(i));
                }
                return list;
            }
        }

        // Return as-is for complex types (Node, Edge, Path, etc.)
        return jsonObj;
    }

    /**
     * Get column names from RETURN clause
     *
     * @return List of variable names
     */
    public List<String> getVariables() {
        return variables;
    }

    /**
     * Get all result rows
     *
     * @return List of rows (each row is a Map)
     */
    public List<Map<String, Object>> getRows() {
        return rows;
    }

    /**
     * Get number of rows
     *
     * @return Row count
     */
    public int getRowCount() {
        return rows.size();
    }

    /**
     * Get first row or null if no rows
     *
     * @return First row or null
     */
    public Map<String, Object> first() {
        return rows.isEmpty() ? null : rows.get(0);
    }

    /**
     * Get all values from a specific column
     *
     * @param columnName Column name to extract
     * @return List of values from that column
     */
    public List<Object> column(String columnName) {
        List<Object> values = new ArrayList<>();
        for (Map<String, Object> row : rows) {
            values.add(row.get(columnName));
        }
        return values;
    }

    /**
     * Check if result is empty
     *
     * @return true if no rows
     */
    public boolean isEmpty() {
        return rows.isEmpty();
    }

    @Override
    public String toString() {
        return String.format("QueryResult(rows=%d, variables=%s)", rows.size(), variables);
    }
}
