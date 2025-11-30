# Drug Discovery Example - GraphLite SDK

A pharmaceutical research example demonstrating how to use the **GraphLite SDK** for drug discovery workflows.

## Overview

This example shows how the ergonomic SDK makes it easy to:
- Model complex pharmaceutical data (proteins, compounds, assays)
- Create relationships with rich properties (IC50, selectivity, experimental conditions)
- Execute analytical queries for drug discovery insights
- Use transactions for data integrity
- Leverage the query builder for cleaner code

## Running the Example

```bash
# From the repository root
cargo run --example drug_discovery

# Or from graphlite-sdk directory
cd graphlite-sdk
cargo run --example drug_discovery
```

**Cleanup:**
```bash
rm -rf ./drug_discovery_sdk_db/
```

## What's Demonstrated

### Domain Model

**Node Types:**
- **Protein** - Disease targets (TP53, EGFR, ACE2, BACE1)
- **Compound** - Drug molecules (Gefitinib, Captopril, APG-115, etc.)
- **Assay** - Laboratory experiments (TR-FRET, SPR, FRET, HTRF)

**Relationships:**
- `TESTED_IN` - Compound → Assay (with test conditions)
- `MEASURES_ACTIVITY_ON` - Assay → Protein (with readout type)
- `INHIBITS` - Compound → Protein (with IC50, Ki, selectivity data)

### SDK Features Used

#### 1. **Simple Database Opening**
```rust
let db = GraphLite::open("./drug_discovery_sdk_db")?;
let session = db.session("researcher")?;
```
No manual session ID management!

#### 2. **Convenient Schema Setup**
```rust
session.execute("CREATE SCHEMA IF NOT EXISTS drug_discovery")?;
session.execute("USE SCHEMA drug_discovery")?;
session.execute("CREATE GRAPH IF NOT EXISTS pharma_research")?;
session.execute("USE GRAPH pharma_research")?;
```
Clean, SQLite-like API.

#### 3. **Transaction Support**
```rust
{
    let mut tx = session.transaction()?;
    tx.execute("INSERT (:Protein {...})")?;
    tx.execute("INSERT (:Compound {...})")?;
    tx.commit()?;
}
// Auto-rollback if commit() not called
```
Ensures data consistency!

#### 4. **Query Builder API**
```rust
let result = session.query_builder()
    .match_pattern("(c:Compound)-[i:INHIBITS]->(p:Protein {id: 'TP53'})")
    .where_clause("i.IC50 < 100")
    .return_clause("c.name, c.id, i.IC50")
    .order_by("i.IC50")
    .execute()?;
```
Fluent, readable query construction.

#### 5. **Direct Query Execution**
```rust
let result = session.query(
    "MATCH (c:Compound)-[i:INHIBITS]->(p:Protein)
     RETURN c.name, p.name, i.IC50
     ORDER BY i.IC50"
)?;
```
For when you prefer raw GQL.

### Analytical Queries

The example demonstrates 5 key drug discovery queries:

1. **Find potent compounds for a target** - IC50 filtering
2. **Complete testing pathway** - Multi-hop graph traversal
3. **Compound-target interactions** - Sorted by potency
4. **Clinical trial compounds** - Using LIKE pattern matching
5. **Target coverage** - Aggregation with COUNT

## Comparison: Core vs SDK

| Feature | Core Library | SDK (This Example) |
|---------|-------------|-------------------|
| Database init | `QueryCoordinator::from_path()` | `GraphLite::open()` |
| Session handling | Manual session ID strings | Session objects |
| Query execution | `process_query(query, &session_id)` | `session.query(query)` |
| Transactions | Manual management | `session.transaction()` |
| Query building | String concatenation | Fluent builder API |
| Error handling | String errors | Typed `Error` enum |

**The SDK is simpler and more ergonomic!**

## Real-World Use Cases

This pattern can be extended for:

### Target-Based Drug Discovery
- Screen compounds against disease targets
- Identify lead compounds
- Track structure-activity relationships (SAR)

### Assay Management
- Track experimental methods
- Compare assay results
- Ensure reproducibility

### Portfolio Analysis
- Monitor clinical trial pipelines
- Identify gaps in target coverage
- Competitive intelligence

### Pharmacology Studies
- Model drug-target interactions
- Track selectivity profiles
- Predict off-target effects

## Extending the Example

### Add More Node Types
```rust
// Cell lines for testing
(:CellLine {
    name: 'HeLa',
    tissue: 'Cervical cancer',
    species: 'Human'
})

// Diseases
(:Disease {
    name: 'Non-Small Cell Lung Cancer',
    icd_code: 'C34.90',
    prevalence: 2.2M
})
```

### Add More Relationships
```rust
// Drug-drug interactions
(c1:Compound)-[:INTERACTS_WITH {severity: 'moderate'}]->(c2:Compound)

// Protein pathways
(p1:Protein)-[:PART_OF_PATHWAY {name: 'EGFR signaling'}]->(pw:Pathway)

// Clinical outcomes
(c:Compound)-[:TREATS {efficacy: 0.85}]->(d:Disease)
```

### Advanced Queries
```rust
// Multi-target compounds (polypharmacology)
let result = session.query(
    "MATCH (c:Compound)-[:INHIBITS]->(p:Protein)
     WITH c, COUNT(p) AS target_count
     WHERE target_count > 1
     RETURN c.name, target_count
     ORDER BY target_count DESC"
)?;

// Compounds with best selectivity
let result = session.query_builder()
    .match_pattern("(c:Compound)-[i:INHIBITS]->(p:Protein)")
    .where_clause("i.selectivity_index > 50")
    .return_clause("c.name, p.name, i.selectivity_index")
    .order_by("i.selectivity_index DESC")
    .execute()?;
```

## Sample Output

```
=== GraphLite SDK Drug Discovery Example ===

1. Opening database...
   ✓ Database opened

2. Creating session...
   ✓ Session created

3. Setting up schema and graph...
   ✓ Schema and graph configured

4. Inserting pharmaceutical data...
   → Inserting target proteins...
   → Inserting drug compounds...
   → Inserting experimental assays...
   ✓ Core data inserted

5. Creating relationships...
   → Linking compounds to assays...
   → Linking assays to proteins...
   → Creating inhibition relationships with IC50 data...
   ✓ Relationships created

6. Running analytical queries...

   Query 1: Compounds targeting TP53 with IC50 < 100 nM
   Results:
     - {"c.name": "APG-115", "i.IC50": 12.5, "i.Ki": 3.2, ...}

   Query 2: Complete testing pathway for Gefitinib
   Results:
     {"c.name": "Gefitinib", "a.name": "EGFR Kinase Inhibition Assay", ...}

   ...

=== Drug Discovery Example Complete ===
```

## Data Sources for Production

To build a production pharmaceutical knowledge graph:

- **ChEMBL** - Bioactivity database
- **DrugBank** - Drug and target information
- **UniProt** - Protein data
- **PubChem** - Chemical structures
- **ClinicalTrials.gov** - Clinical trial data
- **PDB** - Protein structures

## Performance Tips

For large-scale data:

1. **Batch inserts** - Use transactions to insert 100s of nodes at once
2. **Indexes** - Create indexes on frequently queried properties
3. **Limit results** - Use LIMIT clause for large result sets
4. **Selective queries** - Only return needed properties

## Resources

- **GQL Guide**: [../../GQL-GUIDE.md](../../GQL-GUIDE.md)
- **SDK Documentation**: [../README.md](../README.md)
- **Core Example**: [../../examples-core/drug_discovery/](../../examples-core/drug_discovery/)

## License

Apache-2.0 - See [LICENSE](../../LICENSE) for details.
