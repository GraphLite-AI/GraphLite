// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Drug Discovery Use Case Example
//!
//! This example demonstrates how GraphLite can be used for drug discovery research,
//! modeling the relationships between compounds, targets (proteins), and assays.
//!
//! Domain Model:
//! - Targets: Proteins or enzymes that play a key role in a disease (e.g., TP53 in cancer)
//! - Compounds: Small molecules that can bind to or inhibit those proteins
//! - Assays: Experiments that measure how strongly a compound affects a target
//!
//! Graph Structure:
//! Compound → TESTED_IN → Assay → MEASURES_ACTIVITY_ON → Target (Protein)
//! Compound → INHIBITS → Target (with IC50 measurements)
//! Enzyme → CATALYZES → Reaction → PRODUCES → Compound (biosynthetic pathways)
//!
//! Run with: cargo run --example drug_discovery

use graphlite::QueryCoordinator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== GraphLite Drug Discovery Example ===\n");

    // Step 1: Initialize GraphLite
    println!("1. Initializing database...");
    let coordinator = QueryCoordinator::from_path("./drug_discovery_db")
        .map_err(|e| format!("Failed to initialize database: {}", e))?;
    println!("   ✓ Database initialized\n");

    // Step 2: Create a session
    println!("2. Creating session...");
    let session_id = coordinator
        .create_simple_session("researcher")
        .map_err(|e| format!("Failed to create session: {}", e))?;
    println!("   ✓ Session created: {}\n", session_id);

    // Step 3: Setup schema and graph
    println!("3. Setting up schema and graph...");

    coordinator.process_query("CREATE SCHEMA IF NOT EXISTS /drug_discovery", &session_id)?;
    coordinator.process_query("SESSION SET SCHEMA /drug_discovery", &session_id)?;
    coordinator.process_query("CREATE GRAPH IF NOT EXISTS pharma_research", &session_id)?;
    coordinator.process_query("SESSION SET GRAPH pharma_research", &session_id)?;

    println!("   ✓ Schema and graph configured\n");

    // Step 4: Insert Target Proteins
    println!("4. Inserting target proteins (disease-related)...");

    coordinator.process_query(
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
        &session_id,
    )?;

    println!("   ✓ 4 target proteins inserted\n");

    // Step 5: Insert Compounds
    println!("5. Inserting drug compounds...");

    coordinator.process_query(
        r#"INSERT
            (:Compound {
                id: 'CP-001',
                name: 'Imatinib',
                molecular_formula: 'C29H31N7O',
                molecular_weight: 493.603,
                drug_type: 'Small molecule inhibitor',
                development_stage: 'Approved'
            }),
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
        &session_id,
    )?;

    println!("   ✓ 5 compounds inserted\n");

    // Step 6: Insert Assays
    println!("6. Inserting experimental assays...");

    coordinator.process_query(
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
            }),
            (:Assay {
                id: 'AS-005',
                name: 'Cell Viability Assay',
                assay_type: 'Cell-based',
                method: 'MTT',
                date: '2024-04-01'
            })"#,
        &session_id,
    )?;

    println!("   ✓ 5 assays inserted\n");

    // Step 7: Create relationships between compounds and assays
    println!("7. Creating compound-assay relationships...");

    coordinator.process_query(
        r#"MATCH (c:Compound {id: 'CP-002'}), (a:Assay {id: 'AS-001'})
           INSERT (c)-[:TESTED_IN {
               test_date: '2024-01-15',
               concentration_range: '0.1-1000 nM',
               replicate_count: 3
           }]->(a)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (c:Compound {id: 'CP-003'}), (a:Assay {id: 'AS-002'})
           INSERT (c)-[:TESTED_IN {
               test_date: '2024-02-20',
               concentration_range: '1-10000 nM',
               replicate_count: 4
           }]->(a)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (c:Compound {id: 'CP-004'}), (a:Assay {id: 'AS-003'})
           INSERT (c)-[:TESTED_IN {
               test_date: '2024-03-10',
               concentration_range: '0.5-500 nM',
               replicate_count: 3
           }]->(a)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (c:Compound {id: 'CP-005'}), (a:Assay {id: 'AS-004'})
           INSERT (c)-[:TESTED_IN {
               test_date: '2024-03-25',
               concentration_range: '1-1000 nM',
               replicate_count: 5
           }]->(a)"#,
        &session_id,
    )?;

    println!("   ✓ Compound-assay relationships created\n");

    // Step 8: Create relationships between assays and target proteins
    println!("8. Creating assay-protein relationships...");

    coordinator.process_query(
        r#"MATCH (a:Assay {id: 'AS-001'}), (p:Protein {id: 'EGFR'})
           INSERT (a)-[:MEASURES_ACTIVITY_ON {
               readout: 'Kinase inhibition',
               units: 'percent inhibition'
           }]->(p)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (a:Assay {id: 'AS-002'}), (p:Protein {id: 'ACE2'})
           INSERT (a)-[:MEASURES_ACTIVITY_ON {
               readout: 'Binding affinity',
               units: 'KD (nM)'
           }]->(p)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (a:Assay {id: 'AS-003'}), (p:Protein {id: 'BACE1'})
           INSERT (a)-[:MEASURES_ACTIVITY_ON {
               readout: 'Enzymatic activity',
               units: 'percent inhibition'
           }]->(p)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (a:Assay {id: 'AS-004'}), (p:Protein {id: 'TP53'})
           INSERT (a)-[:MEASURES_ACTIVITY_ON {
               readout: 'PPI disruption',
               units: 'IC50 (nM)'
           }]->(p)"#,
        &session_id,
    )?;

    println!("   ✓ Assay-protein relationships created\n");

    // Step 9: Create direct inhibition relationships with IC50 values
    println!("9. Creating compound-protein inhibition relationships with IC50 data...");

    coordinator.process_query(
        r#"MATCH (c:Compound {id: 'CP-002'}), (p:Protein {id: 'EGFR'})
           INSERT (c)-[:INHIBITS {
               IC50: 37.5,
               IC50_unit: 'nM',
               Ki: 12.3,
               selectivity_index: 25.6,
               measurement_date: '2024-01-15'
           }]->(p)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (c:Compound {id: 'CP-003'}), (p:Protein {id: 'ACE2'})
           INSERT (c)-[:INHIBITS {
               IC50: 23.0,
               IC50_unit: 'nM',
               Ki: 7.8,
               selectivity_index: 15.2,
               measurement_date: '2024-02-20'
           }]->(p)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (c:Compound {id: 'CP-004'}), (p:Protein {id: 'BACE1'})
           INSERT (c)-[:INHIBITS {
               IC50: 85.0,
               IC50_unit: 'nM',
               Ki: 28.5,
               selectivity_index: 45.1,
               measurement_date: '2024-03-10'
           }]->(p)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (c:Compound {id: 'CP-005'}), (p:Protein {id: 'TP53'})
           INSERT (c)-[:INHIBITS {
               IC50: 12.5,
               IC50_unit: 'nM',
               Ki: 3.2,
               selectivity_index: 120.5,
               measurement_date: '2024-03-25'
           }]->(p)"#,
        &session_id,
    )?;

    println!("   ✓ Inhibition relationships with IC50 data created\n");

    // Step 10: Add biosynthetic pathway data
    println!("10. Adding biosynthetic pathway data...");

    // Insert enzymes
    coordinator.process_query(
        r#"INSERT
            (:Enzyme {
                id: 'ENZ-001',
                name: 'Cytochrome P450 3A4',
                ec_number: '1.14.13.97',
                function: 'Drug metabolism'
            }),
            (:Enzyme {
                id: 'ENZ-002',
                name: 'Tyrosine Kinase',
                ec_number: '2.7.10.1',
                function: 'Phosphorylation'
            })"#,
        &session_id,
    )?;

    // Insert biochemical reactions
    coordinator.process_query(
        r#"INSERT
            (:Reaction {
                id: 'RXN-001',
                name: 'Imatinib Hydroxylation',
                reaction_type: 'Oxidation',
                pathway: 'Drug Metabolism'
            }),
            (:Reaction {
                id: 'RXN-002',
                name: 'Gefitinib Demethylation',
                reaction_type: 'Demethylation',
                pathway: 'Drug Metabolism'
            })"#,
        &session_id,
    )?;

    // Insert metabolites
    coordinator.process_query(
        r#"INSERT
            (:Compound {
                id: 'CP-001-M1',
                name: 'N-Desmethyl Imatinib',
                molecular_formula: 'C28H29N7O',
                molecular_weight: 479.576,
                drug_type: 'Active metabolite',
                development_stage: 'Metabolite'
            }),
            (:Compound {
                id: 'CP-002-M1',
                name: 'O-Desmethyl Gefitinib',
                molecular_formula: 'C21H22ClFN4O3',
                molecular_weight: 432.875,
                drug_type: 'Active metabolite',
                development_stage: 'Metabolite'
            })"#,
        &session_id,
    )?;

    // Create biosynthetic pathway relationships
    coordinator.process_query(
        r#"MATCH (e:Enzyme {id: 'ENZ-001'}), (r:Reaction {id: 'RXN-001'})
           INSERT (e)-[:CATALYZES {
               kcat: 12.5,
               km: 45.2,
               kcat_km_ratio: 0.276,
               temperature: 37.0,
               pH: 7.4
           }]->(r)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (e:Enzyme {id: 'ENZ-001'}), (r:Reaction {id: 'RXN-002'})
           INSERT (e)-[:CATALYZES {
               kcat: 8.3,
               km: 32.1,
               kcat_km_ratio: 0.258,
               temperature: 37.0,
               pH: 7.4
           }]->(r)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (r:Reaction {id: 'RXN-001'}), (c:Compound {id: 'CP-001-M1'})
           INSERT (r)-[:PRODUCES {
               yield_percent: 78.5,
               reaction_time_hours: 2.5,
               rate_constant: 0.045
           }]->(c)"#,
        &session_id,
    )?;

    coordinator.process_query(
        r#"MATCH (r:Reaction {id: 'RXN-002'}), (c:Compound {id: 'CP-002-M1'})
           INSERT (r)-[:PRODUCES {
               yield_percent: 65.2,
               reaction_time_hours: 3.0,
               rate_constant: 0.032
           }]->(c)"#,
        &session_id,
    )?;

    println!("   ✓ Biosynthetic pathway data added\n");

    // Step 11: Execute analytical queries
    println!("11. Running analytical queries...\n");

    // Query 1: Find all compounds tested on TP53 with IC50 < 100 nM
    println!("   Query 1: Compounds targeting TP53 with IC50 < 100 nM");
    let result1 = coordinator.process_query(
        r#"MATCH (c:Compound)-[i:INHIBITS]->(p:Protein {id: 'TP53'})
           WHERE i.IC50 < 100
           RETURN c.name, c.id, i.IC50, i.IC50_unit, i.Ki
           ORDER BY i.IC50"#,
        &session_id,
    )?;

    println!("   Results:");
    for row in &result1.rows {
        println!("     - {:?}", row.values);
    }
    println!();

    // Query 2: Find the complete testing path for a compound
    println!("   Query 2: Complete testing pathway for Gefitinib (CP-002)");
    let result2 = coordinator.process_query(
        r#"MATCH (c:Compound {id: 'CP-002'})-[t:TESTED_IN]->(a:Assay)-[m:MEASURES_ACTIVITY_ON]->(p:Protein)
           RETURN c.name, a.name, a.assay_type, p.name, p.disease"#,
        &session_id
    )?;

    println!("   Results:");
    for row in &result2.rows {
        println!("     {:?}", row.values);
    }
    println!();

    // Query 3: Find all compounds with their targets and IC50 values
    println!("   Query 3: All compound-target interactions sorted by potency");
    let result3 = coordinator.process_query(
        r#"MATCH (c:Compound)-[i:INHIBITS]->(p:Protein)
           RETURN c.name AS Compound,
                  p.name AS Target,
                  p.disease AS Disease,
                  i.IC50 AS IC50_nM,
                  c.development_stage AS Stage
           ORDER BY i.IC50"#,
        &session_id,
    )?;

    println!("   Results:");
    println!("   {:?}", result3.variables);
    for row in &result3.rows {
        println!("     {:?}", row.values);
    }
    println!();

    // Query 4: Find biosynthetic pathways - enzyme to product
    println!("   Query 4: Biosynthetic pathways - enzymes and their products");
    let result4 = coordinator.process_query(
        r#"MATCH (e:Enzyme)-[:CATALYZES]->(r:Reaction)-[:PRODUCES]->(c:Compound)
           RETURN e.name AS Enzyme,
                  r.name AS Reaction,
                  c.name AS Product,
                  c.drug_type AS ProductType"#,
        &session_id,
    )?;

    println!("   Results:");
    for row in &result4.rows {
        println!("     {:?}", row.values);
    }
    println!();

    // Query 5: Find all proteins targeted by multiple compounds
    println!("   Query 5: Proteins with multiple targeting compounds");
    let result5 = coordinator.process_query(
        r#"MATCH (p:Protein)<-[:INHIBITS]-(c:Compound)
           RETURN p.name AS Protein,
                  p.disease AS Disease,
                  COUNT(c) AS CompoundCount"#,
        &session_id,
    )?;

    println!("   Results:");
    for row in &result5.rows {
        println!("     {:?}", row.values);
    }
    println!();

    // Query 6: Find compounds in clinical trials with their target proteins
    println!("   Query 6: Clinical trial compounds and their targets");
    let result6 = coordinator.process_query(
        r#"MATCH (c:Compound)-[i:INHIBITS]->(p:Protein)
           WHERE c.development_stage LIKE '%Clinical Trial%'
           RETURN c.name AS Compound,
                  c.development_stage AS Stage,
                  p.name AS Target,
                  i.IC50 AS Potency_nM,
                  i.selectivity_index AS Selectivity"#,
        &session_id,
    )?;

    println!("   Results:");
    for row in &result6.rows {
        println!("     {:?}", row.values);
    }
    println!();

    // Step 12: Clean up
    println!("12. Closing session...");
    coordinator.close_session(&session_id)?;
    println!("   ✓ Session closed\n");

    println!("=== Drug Discovery Example Complete ===");
    println!("\nKey Insights:");
    println!(
        "  • Modeled {} node types: Protein, Compound, Assay, Enzyme, Reaction",
        5
    );
    println!("  • Created relationship types: TESTED_IN, MEASURES_ACTIVITY_ON, INHIBITS, CATALYZES, PRODUCES");
    println!("  • Demonstrated graph traversals for drug discovery workflows");
    println!("  • Showed IC50-based compound filtering and ranking");
    println!("  • Explored biosynthetic pathways for drug metabolism");
    println!("\nDatabase location: ./drug_discovery_db/");
    println!("To clean up: rm -rf ./drug_discovery_db/");

    Ok(())
}
