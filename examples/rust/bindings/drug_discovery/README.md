# Drug Discovery Example

A comprehensive real-world example demonstrating how GraphLite can be used for pharmaceutical research and drug discovery workflows.

## Overview

This example models the complex relationships between drug compounds, protein targets, experimental assays, enzymes, and biochemical reactions - showcasing how graph databases naturally represent the interconnected nature of drug discovery research.

## Running the Example

```bash
cargo run --example drug_discovery
```

The example will:
1. Create a database at `./drug_discovery_db/`
2. Insert realistic pharmaceutical data
3. Execute several analytical queries
4. Display results demonstrating various graph patterns

**Cleanup:**
```bash
rm -rf ./drug_discovery_db/
```

## Domain Model

### Node Types

#### 1. **Protein (Disease Targets)**
Proteins or enzymes that play a key role in diseases. These are the targets that drugs aim to inhibit or modulate.

**Example data:**
- **TP53**: Tumor Protein P53 - Cancer (tumor suppressor)
- **EGFR**: Epidermal Growth Factor Receptor - Cancer (cell growth)
- **ACE2**: Angiotensin-Converting Enzyme 2 - Hypertension (blood pressure regulation)
- **BACE1**: Beta-Secretase 1 - Alzheimer's (amyloid beta production)

**Properties:**
- `id`: Unique identifier
- `name`: Full protein name
- `disease`: Associated disease
- `function`: Biological function
- `gene_location`: Chromosomal location

#### 2. **Compound (Drug Molecules)**
Small molecules that can bind to or inhibit target proteins.

**Example data:**
- **Imatinib** (CP-001): Small molecule inhibitor - Approved
- **Gefitinib** (CP-002): EGFR inhibitor - Approved
- **Captopril** (CP-003): ACE inhibitor - Approved
- **LY2811376** (CP-004): BACE1 inhibitor - Clinical Trial Phase 1
- **APG-115** (CP-005): MDM2-p53 inhibitor - Clinical Trial Phase 2

**Properties:**
- `id`: Unique compound identifier
- `name`: Common/trade name
- `molecular_formula`: Chemical formula
- `molecular_weight`: Molecular mass
- `drug_type`: Classification
- `development_stage`: Current development status

#### 3. **Assay (Experiments)**
Laboratory experiments that measure how strongly a compound affects a target protein.

**Example data:**
- EGFR Kinase Inhibition Assay (TR-FRET method)
- ACE2 Binding Assay (SPR method)
- BACE1 Activity Assay (FRET method)
- p53-MDM2 Disruption Assay (HTRF method)

**Properties:**
- `id`: Assay identifier
- `name`: Descriptive name
- `assay_type`: Category (Enzymatic, Binding, Cell-based, etc.)
- `method`: Experimental technique
- `date`: Experiment date

#### 4. **Enzyme**
Biological catalysts involved in drug metabolism and biosynthetic pathways.

**Example data:**
- **Cytochrome P450 3A4**: Drug metabolism enzyme

**Properties:**
- `id`: Enzyme identifier
- `name`: Enzyme name
- `ec_number`: Enzyme Commission number
- `function`: Biological role

#### 5. **Reaction**
Biochemical transformations in metabolic or biosynthetic pathways.

**Example data:**
- Imatinib Hydroxylation (Oxidation)
- Gefitinib Demethylation

**Properties:**
- `id`: Reaction identifier
- `name`: Reaction name
- `reaction_type`: Chemical transformation type
- `pathway`: Metabolic pathway

### Relationship Types

#### 1. **TESTED_IN**
Connects compounds to assays where they were tested.

```
Compound -[TESTED_IN]-> Assay
```

**Properties:**
- `test_date`: When the test was performed
- `concentration_range`: Range of concentrations tested
- `replicate_count`: Number of experimental replicates

#### 2. **MEASURES_ACTIVITY_ON**
Connects assays to the protein targets they measure.

```
Assay -[MEASURES_ACTIVITY_ON]-> Protein
```

**Properties:**
- `readout`: Type of measurement (e.g., "Kinase inhibition")
- `units`: Measurement units

#### 3. **INHIBITS**
Direct inhibition relationship between compounds and proteins with potency data.

```
Compound -[INHIBITS]-> Protein
```

**Properties:**
- `IC50`: Half-maximal inhibitory concentration (nM)
- `IC50_unit`: Units for IC50
- `Ki`: Inhibition constant
- `selectivity_index`: Selectivity over other targets
- `measurement_date`: When measured

> **IC₅₀ (IC50)**: The concentration of a drug required to inhibit a biological process by 50%. Lower values indicate more potent compounds.

#### 4. **CATALYZES**
Connects enzymes to the reactions they catalyze.

```
Enzyme -[CATALYZES]-> Reaction
```

**Properties:**
- `kcat`: Catalytic rate constant
- `km`: Michaelis constant
- `kcat_km_ratio`: Catalytic efficiency
- `temperature`: Reaction temperature
- `pH`: Reaction pH

#### 5. **PRODUCES**
Connects reactions to the products they generate.

```
Reaction -[PRODUCES]-> Compound
```

**Properties:**
- `yield_percent`: Reaction yield percentage
- `reaction_time_hours`: Time required
- `rate_constant`: Reaction rate constant

## Graph Structure

The complete graph structure represents three main data flows:

### 1. Drug Testing Workflow
```
Compound → TESTED_IN → Assay → MEASURES_ACTIVITY_ON → Protein
```

This represents the experimental workflow where compounds are tested in assays that measure their effect on target proteins.

### 2. Direct Inhibition
```
Compound → INHIBITS → Protein
```

This captures the direct relationship between a drug compound and its target, including potency measurements (IC₅₀, Ki).

### 3. Biosynthetic/Metabolic Pathways
```
Enzyme → CATALYZES → Reaction → PRODUCES → Compound (Metabolite)
```

This models how enzymes convert drugs into metabolites through biochemical reactions.

## Example Queries

The example demonstrates six key analytical queries:

### Query 1: Find Potent Compounds for a Specific Target

```gql
MATCH (c:Compound)-[i:INHIBITS]->(p:Protein {id: 'TP53'})
WHERE i.IC50 < 100
RETURN c.name, c.id, i.IC50, i.IC50_unit, i.Ki
ORDER BY i.IC50
```

**Purpose:** Identify compounds that strongly inhibit TP53 (IC₅₀ < 100 nM).

**Use Case:** Target-based drug discovery - finding lead compounds for a specific disease target.

### Query 2: Complete Testing Pathway

```gql
MATCH (c:Compound {id: 'CP-002'})-[t:TESTED_IN]->(a:Assay)-[m:MEASURES_ACTIVITY_ON]->(p:Protein)
RETURN c.name, a.name, a.assay_type, p.name, p.disease
```

**Purpose:** Trace the complete experimental path for Gefitinib from compound → assay → protein.

**Use Case:** Understanding the full testing history of a compound.

### Query 3: All Compound-Target Interactions Sorted by Potency

```gql
MATCH (c:Compound)-[i:INHIBITS]->(p:Protein)
RETURN c.name AS Compound,
       p.name AS Target,
       p.disease AS Disease,
       i.IC50 AS IC50_nM,
       c.development_stage AS Stage
ORDER BY i.IC50
```

**Purpose:** Get a ranked list of all compounds by their potency across all targets.

**Use Case:** Portfolio analysis - identifying most promising compounds.

### Query 4: Biosynthetic Pathways

```gql
MATCH (e:Enzyme)-[:CATALYZES]->(r:Reaction)-[:PRODUCES]->(c:Compound)
RETURN e.name AS Enzyme,
       r.name AS Reaction,
       c.name AS Product,
       c.drug_type AS ProductType
```

**Purpose:** Explore drug metabolism pathways showing enzyme → reaction → metabolite chains.

**Use Case:** Understanding drug metabolism and pharmacokinetics (ADME studies).

### Query 5: Proteins with Multiple Targeting Compounds

```gql
MATCH (p:Protein)<-[:INHIBITS]-(c:Compound)
RETURN p.name AS Protein,
       p.disease AS Disease,
       COUNT(c) AS CompoundCount
```

**Purpose:** Identify which proteins are targeted by multiple compounds (aggregation query).

**Use Case:** Understanding target tractability and competitive landscape.

### Query 6: Clinical Trial Compounds

```gql
MATCH (c:Compound)-[i:INHIBITS]->(p:Protein)
WHERE c.development_stage LIKE '%Clinical Trial%'
RETURN c.name AS Compound,
       c.development_stage AS Stage,
       p.name AS Target,
       i.IC50 AS Potency_nM,
       i.selectivity_index AS Selectivity
```

**Purpose:** Filter compounds currently in clinical trials with their target and potency data.

**Use Case:** Pipeline analysis - tracking compounds in development.

## Real-World Applications

This graph model can be used for:

### 1. **Target-Based Drug Discovery**
- Identify all compounds tested against a specific disease target
- Find the most potent inhibitors for a target
- Explore selectivity profiles across related targets

### 2. **Compound Optimization**
- Track structure-activity relationships (SAR)
- Compare compounds across multiple targets
- Identify lead compounds for optimization

### 3. **Assay Development**
- Track which assays are used for which targets
- Compare different experimental methods
- Validate assay reproducibility

### 4. **Drug Metabolism Studies (ADME)**
- Model metabolic pathways
- Identify active metabolites
- Predict drug-drug interactions
- Understand pharmacokinetics

### 5. **Portfolio Management**
- Track compounds through development stages
- Analyze clinical trial pipelines
- Identify gaps in target coverage
- Compare competitive compounds

### 6. **Multi-Target Drug Discovery**
- Find compounds with poly-pharmacology
- Design drugs targeting multiple proteins
- Understand off-target effects

### 7. **Knowledge Graph Integration**
- Link to external databases (ChEMBL, DrugBank, UniProt)
- Integrate literature data
- Connect to clinical outcomes
- Build comprehensive pharmaceutical knowledge graphs

## Extending the Example

### Additional Node Types to Consider

1. **Disease**: Model disease-target relationships more explicitly
2. **Patient**: Connect to clinical outcomes
3. **Gene**: Link proteins to their encoding genes
4. **Pathway**: Group proteins by biological pathways
5. **Side Effect**: Track adverse drug reactions
6. **Publication**: Link to scientific literature

### Additional Relationship Types

1. **CAUSES** (Protein → Disease)
2. **TREATED_BY** (Disease → Compound)
3. **INTERACTS_WITH** (Compound → Compound) - Drug interactions
4. **BINDS_TO** (Compound → Protein) - Binding site information
5. **EXPRESSED_IN** (Protein → Tissue/Cell)
6. **MODULATES** (Compound → Pathway)

### Advanced Queries to Implement

```gql
-- Find multi-target compounds (polypharmacology)
MATCH (c:Compound)-[:INHIBITS]->(p:Protein)
WITH c, COUNT(p) AS target_count
WHERE target_count > 1
RETURN c.name, target_count

-- Find biosynthetic pathway chains (2+ hops)
MATCH path = (e:Enzyme)-[:CATALYZES]->()-[:PRODUCES]->()
           -[:SUBSTRATE_OF]->()-[:PRODUCES]->(final:Compound)
RETURN path

-- Compare compounds for the same target
MATCH (c1:Compound)-[i1:INHIBITS]->(p:Protein {id: 'TP53'}),
      (c2:Compound)-[i2:INHIBITS]->(p)
WHERE c1.id < c2.id
RETURN c1.name, i1.IC50, c2.name, i2.IC50,
       (i1.IC50 - i2.IC50) AS potency_difference

-- Find compounds with good selectivity
MATCH (c:Compound)-[i:INHIBITS]->(p:Protein)
WHERE i.selectivity_index > 50
RETURN c.name, p.name, i.selectivity_index
ORDER BY i.selectivity_index DESC
```

## Graph Database Advantages for Drug Discovery

### 1. **Natural Relationship Modeling**
Drug discovery data is inherently relational. Graph databases represent these connections naturally without complex joins.

### 2. **Flexible Schema**
Easily add new node types (e.g., Biomarkers, Cell Lines) or properties as research evolves.

### 3. **Path Traversal**
Efficiently explore multi-hop relationships like:
- Compound → Assay → Target → Pathway → Disease
- Drug → Metabolite → Enzyme → Gene

### 4. **Pattern Matching**
Find complex patterns like:
- "Compounds that inhibit multiple proteins in the same pathway"
- "Targets with no approved drugs but active compounds in trials"

### 5. **Integration**
Easily integrate with external knowledge graphs and databases.

## Data Sources for Real Implementations

To build a production drug discovery graph database, consider integrating:

1. **ChEMBL** - Bioactivity database (compounds, assays, targets)
2. **DrugBank** - Drug information database
3. **UniProt** - Protein sequence and functional information
4. **PubChem** - Chemical structures and properties
5. **ClinicalTrials.gov** - Clinical trial data
6. **PDB** - Protein structure data
7. **KEGG** - Pathway and reaction databases
8. **STRING** - Protein-protein interaction networks

## Performance Considerations

For large-scale drug discovery graphs:

1. **Indexing**: Create indexes on frequently queried properties (compound IDs, protein IDs)
2. **Batching**: Insert data in batches for better performance
3. **Caching**: Use GraphLite's caching for frequently accessed data
4. **Pagination**: Use LIMIT/OFFSET for large result sets
5. **Selective Loading**: Only load necessary properties in queries

## References

- **ISO GQL**: [GQL Guide](../GQL-GUIDE.md)
- **GraphLite SDK**: [SDK Examples](../graphlite-sdk/examples/)
- **Main Documentation**: [README](../README.md)

## License

Apache-2.0 - See [LICENSE](../LICENSE) for details.

---

**Note**: This is a demonstration example with synthetic data. Real pharmaceutical data should be sourced from validated databases and properly licensed.
