//! Drug Discovery Example using GraphLite SDK
//!
//! This example demonstrates how GraphLite SDK can be used for pharmaceutical research,
//! modeling the relationships between compounds, targets (proteins), and assays.
//!
//! Domain Model:
//! - Targets: Proteins or enzymes that play a key role in a disease (e.g., TP53 in cancer)
//! - Compounds: Small molecules that can bind to or inhibit those proteins
//! - Assays: Experiments that measure how strongly a compound affects a target
//! - Enzymes: Biological catalysts in metabolic pathways
//! - Reactions: Biochemical transformations in biosynthetic pathways
//!
//! Graph Structure:
//! Compound → TESTED_IN → Assay → MEASURES_ACTIVITY_ON → Target (Protein)
//! Compound → INHIBITS → Target (with IC50 measurements)
//! Enzyme → CATALYZES → Reaction → PRODUCES → Compound (biosynthetic pathways)
//!
//! Run with: cargo run --example drug_discovery

use graphlite_sdk::{GraphLite, Error};

fn main() -> Result<(), Error> {
    println!("=== GraphLite SDK Drug Discovery Example ===\n");

    // Step 1: Open database
    println!("1. Opening database...");
    let db_path = "./drug_discovery_sdk_db";
    let db = GraphLite::open(db_path)?;
    println!("   ✓ Database opened\n");

    // Step 2: Create session
    println!("2. Creating session...");
    let session = db.session("researcher")?;
    println!("   ✓ Session created\n");

    // Step 3: Setup schema and graph
    println!("3. Setting up schema and graph...");
    session.execute("CREATE SCHEMA IF NOT EXISTS /drug_discovery")?;
    session.execute("SESSION SET SCHEMA /drug_discovery")?;
    session.execute("CREATE GRAPH IF NOT EXISTS pharma_research")?;
    session.execute("SESSION SET GRAPH pharma_research")?;
    println!("   ✓ Schema and graph configured\n");

    // Step 4: Insert data using transactions
    println!("4. Inserting pharmaceutical data...");
    {
        let mut tx = session.transaction()?;

        // Insert Proteins (Disease Targets)
        println!("   → Inserting target proteins...");
        tx.execute(
            r#"INSERT
                (:Protein {
                    id: 'TP53',
                    name: 'Tumor Protein P53',
                    disease: 'Cancer',
                    function: 'Tumor suppressor',
                    gene_location: '17p13.1'
                }),
                (:Protein {
                    id: 'EGFR',
                    name: 'Epidermal Growth Factor Receptor',
                    disease: 'Cancer',
                    function: 'Cell growth and division',
                    gene_location: '7p11.2'
                }),
                (:Protein {
                    id: 'ACE2',
                    name: 'Angiotensin-Converting Enzyme 2',
                    disease: 'Hypertension',
                    function: 'Blood pressure regulation',
                    gene_location: 'Xp22.2'
                }),
                (:Protein {
                    id: 'BACE1',
                    name: 'Beta-Secretase 1',
                    disease: 'Alzheimers',
                    function: 'Amyloid beta production',
                    gene_location: '11q23.3'
                })"#,
        )?;

        // Insert Compounds
        println!("   → Inserting drug compounds...");
        tx.execute(
            r#"INSERT
                (:Compound {
                    id: 'CP-002',
                    name: 'Gefitinib',
                    molecular_formula: 'C22H24ClFN4O3',
                    molecular_weight: 446.902,
                    drug_type: 'EGFR inhibitor',
                    development_stage: 'Approved'
                }),
                (:Compound {
                    id: 'CP-003',
                    name: 'Captopril',
                    molecular_formula: 'C9H15NO3S',
                    molecular_weight: 217.285,
                    drug_type: 'ACE inhibitor',
                    development_stage: 'Approved'
                }),
                (:Compound {
                    id: 'CP-004',
                    name: 'LY2811376',
                    molecular_formula: 'C18H17F3N2O3',
                    molecular_weight: 366.33,
                    drug_type: 'BACE1 inhibitor',
                    development_stage: 'Clinical Trial Phase 1'
                }),
                (:Compound {
                    id: 'CP-005',
                    name: 'APG-115',
                    molecular_formula: 'C31H37N5O4',
                    molecular_weight: 543.66,
                    drug_type: 'MDM2-p53 inhibitor',
                    development_stage: 'Clinical Trial Phase 2'
                })"#,
        )?;

        // Insert Assays
        println!("   → Inserting experimental assays...");
        tx.execute(
            r#"INSERT
                (:Assay {
                    id: 'AS-001',
                    name: 'EGFR Kinase Inhibition Assay',
                    assay_type: 'Enzymatic',
                    method: 'TR-FRET',
                    date: '2024-01-15'
                }),
                (:Assay {
                    id: 'AS-002',
                    name: 'ACE2 Binding Assay',
                    assay_type: 'Binding',
                    method: 'SPR',
                    date: '2024-02-20'
                }),
                (:Assay {
                    id: 'AS-003',
                    name: 'BACE1 Activity Assay',
                    assay_type: 'Enzymatic',
                    method: 'FRET',
                    date: '2024-03-10'
                }),
                (:Assay {
                    id: 'AS-004',
                    name: 'p53-MDM2 Disruption Assay',
                    assay_type: 'Protein-Protein Interaction',
                    method: 'HTRF',
                    date: '2024-03-25'
                })"#,
        )?;

        tx.commit()?;
        println!("   ✓ Core data inserted\n");
    }

    // Step 5: Create relationships using query builder and transactions
    println!("5. Creating relationships...");
    {
        let mut tx = session.transaction()?;

        // Compound-Assay relationships
        println!("   → Linking compounds to assays...");
        tx.execute(
            r#"MATCH (c:Compound {id: 'CP-002'}), (a:Assay {id: 'AS-001'})
               INSERT (c)-[:TESTED_IN {
                   test_date: '2024-01-15',
                   concentration_range: '0.1-1000 nM',
                   replicate_count: 3
               }]->(a)"#,
        )?;

        tx.execute(
            r#"MATCH (c:Compound {id: 'CP-003'}), (a:Assay {id: 'AS-002'})
               INSERT (c)-[:TESTED_IN {
                   test_date: '2024-02-20',
                   concentration_range: '1-10000 nM',
                   replicate_count: 4
               }]->(a)"#,
        )?;

        tx.execute(
            r#"MATCH (c:Compound {id: 'CP-004'}), (a:Assay {id: 'AS-003'})
               INSERT (c)-[:TESTED_IN {
                   test_date: '2024-03-10',
                   concentration_range: '0.5-500 nM',
                   replicate_count: 3
               }]->(a)"#,
        )?;

        tx.execute(
            r#"MATCH (c:Compound {id: 'CP-005'}), (a:Assay {id: 'AS-004'})
               INSERT (c)-[:TESTED_IN {
                   test_date: '2024-03-25',
                   concentration_range: '1-1000 nM',
                   replicate_count: 5
               }]->(a)"#,
        )?;

        // Assay-Protein relationships
        println!("   → Linking assays to proteins...");
        tx.execute(
            r#"MATCH (a:Assay {id: 'AS-001'}), (p:Protein {id: 'EGFR'})
               INSERT (a)-[:MEASURES_ACTIVITY_ON {
                   readout: 'Kinase inhibition',
                   units: 'percent inhibition'
               }]->(p)"#,
        )?;

        tx.execute(
            r#"MATCH (a:Assay {id: 'AS-002'}), (p:Protein {id: 'ACE2'})
               INSERT (a)-[:MEASURES_ACTIVITY_ON {
                   readout: 'Binding affinity',
                   units: 'KD (nM)'
               }]->(p)"#,
        )?;

        tx.execute(
            r#"MATCH (a:Assay {id: 'AS-003'}), (p:Protein {id: 'BACE1'})
               INSERT (a)-[:MEASURES_ACTIVITY_ON {
                   readout: 'Enzymatic activity',
                   units: 'percent inhibition'
               }]->(p)"#,
        )?;

        tx.execute(
            r#"MATCH (a:Assay {id: 'AS-004'}), (p:Protein {id: 'TP53'})
               INSERT (a)-[:MEASURES_ACTIVITY_ON {
                   readout: 'PPI disruption',
                   units: 'IC50 (nM)'
               }]->(p)"#,
        )?;

        // Direct inhibition relationships with IC50 data
        println!("   → Creating inhibition relationships with IC50 data...");
        tx.execute(
            r#"MATCH (c:Compound {id: 'CP-002'}), (p:Protein {id: 'EGFR'})
               INSERT (c)-[:INHIBITS {
                   IC50: 37.5,
                   IC50_unit: 'nM',
                   Ki: 12.3,
                   selectivity_index: 25.6,
                   measurement_date: '2024-01-15'
               }]->(p)"#,
        )?;

        tx.execute(
            r#"MATCH (c:Compound {id: 'CP-003'}), (p:Protein {id: 'ACE2'})
               INSERT (c)-[:INHIBITS {
                   IC50: 23.0,
                   IC50_unit: 'nM',
                   Ki: 7.8,
                   selectivity_index: 15.2,
                   measurement_date: '2024-02-20'
               }]->(p)"#,
        )?;

        tx.execute(
            r#"MATCH (c:Compound {id: 'CP-004'}), (p:Protein {id: 'BACE1'})
               INSERT (c)-[:INHIBITS {
                   IC50: 85.0,
                   IC50_unit: 'nM',
                   Ki: 28.5,
                   selectivity_index: 45.1,
                   measurement_date: '2024-03-10'
               }]->(p)"#,
        )?;

        tx.execute(
            r#"MATCH (c:Compound {id: 'CP-005'}), (p:Protein {id: 'TP53'})
               INSERT (c)-[:INHIBITS {
                   IC50: 12.5,
                   IC50_unit: 'nM',
                   Ki: 3.2,
                   selectivity_index: 120.5,
                   measurement_date: '2024-03-25'
               }]->(p)"#,
        )?;

        tx.commit()?;
        println!("   ✓ Relationships created\n");
    }

    // Step 6: Execute analytical queries
    println!("6. Running analytical queries...\n");

    // Query 1: Find potent compounds for TP53
    println!("   Query 1: Compounds targeting TP53 with IC50 < 100 nM");
    let result = session.query_builder()
        .match_pattern("(c:Compound)-[i:INHIBITS]->(p:Protein {id: 'TP53'})")
        .where_clause("i.IC50 < 100")
        .return_clause("c.name, c.id, i.IC50, i.IC50_unit, i.Ki")
        .order_by("i.IC50")
        .execute()?;

    println!("   Results:");
    for row in &result.rows {
        println!("     - {:?}", row.values);
    }
    println!();

    // Query 2: Complete testing pathway
    println!("   Query 2: Complete testing pathway for Gefitinib");
    let result = session.query(
        r#"MATCH (c:Compound {id: 'CP-002'})-[t:TESTED_IN]->(a:Assay)-[m:MEASURES_ACTIVITY_ON]->(p:Protein)
           RETURN c.name, a.name, a.assay_type, p.name, p.disease"#,
    )?;

    println!("   Results:");
    for row in &result.rows {
        println!("     {:?}", row.values);
    }
    println!();

    // Query 3: All compound-target interactions sorted by potency
    println!("   Query 3: All compound-target interactions sorted by potency");
    let result = session.query(
        r#"MATCH (c:Compound)-[i:INHIBITS]->(p:Protein)
           RETURN c.name AS Compound,
                  p.name AS Target,
                  p.disease AS Disease,
                  i.IC50 AS IC50_nM,
                  c.development_stage AS Stage
           ORDER BY i.IC50"#,
    )?;

    println!("   Results:");
    println!("   Columns: {:?}", result.variables);
    for row in &result.rows {
        println!("     {:?}", row.values);
    }
    println!();

    // Query 4: Compounds in clinical trials
    println!("   Query 4: Clinical trial compounds and their targets");
    let result = session.query_builder()
        .match_pattern("(c:Compound)-[i:INHIBITS]->(p:Protein)")
        .where_clause("c.development_stage LIKE '%Clinical Trial%'")
        .return_clause("c.name AS Compound, c.development_stage AS Stage, p.name AS Target, i.IC50 AS Potency_nM, i.selectivity_index AS Selectivity")
        .execute()?;

    println!("   Results:");
    for row in &result.rows {
        println!("     {:?}", row.values);
    }
    println!();

    // Query 5: Proteins with multiple targeting compounds (aggregation)
    println!("   Query 5: Proteins with multiple targeting compounds");
    let result = session.query(
        r#"MATCH (p:Protein)<-[:INHIBITS]-(c:Compound)
           RETURN p.name AS Protein,
                  p.disease AS Disease,
                  COUNT(c) AS CompoundCount"#,
    )?;

    println!("   Results:");
    for row in &result.rows {
        println!("     {:?}", row.values);
    }
    println!();

    // Step 7: Summary
    println!("=== Drug Discovery Example Complete ===");
    println!("\nKey Insights:");
    println!("  • Modeled 4 node types: Protein, Compound, Assay");
    println!("  • Created relationship types: TESTED_IN, MEASURES_ACTIVITY_ON, INHIBITS");
    println!("  • Demonstrated graph traversals for drug discovery workflows");
    println!("  • Showed IC50-based compound filtering and ranking");
    println!("  • Used SDK features: transactions, query builder, automatic session management");
    println!("\nDatabase location: {}/", db_path);
    println!("To clean up: rm -rf {}/", db_path);

    Ok(())
}
