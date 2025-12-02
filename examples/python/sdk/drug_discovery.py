#!/usr/bin/env python3
"""
Drug Discovery Example using GraphLite High-Level Python SDK

This example demonstrates how the GraphLite High-Level SDK can be used for
pharmaceutical research, modeling the relationships between compounds, targets
(proteins), and assays.

This version uses the high-level SDK from the python-sdk branch, which provides:
- Session-centric API (session objects vs session IDs)
- Typed exceptions
- Cleaner, more Pythonic interface matching the Rust SDK

Domain Model:
- Targets: Proteins or enzymes that play a key role in a disease (e.g., TP53 in cancer)
- Compounds: Small molecules that can bind to or inhibit those proteins
- Assays: Experiments that measure how strongly a compound affects a target

Graph Structure:
Compound → TESTED_IN → Assay → MEASURES_ACTIVITY_ON → Target (Protein)
Compound → INHIBITS → Target (with IC50 measurements)

Run with: python3 drug_discovery.py
"""

import sys
import os
import shutil
from pathlib import Path

# Add the python-sdk to the path
# Note: The python-sdk is currently in the deepgraphai/GraphLite repository
# on the python-sdk branch. You may need to adjust this path based on your setup.
graphlite_repo = Path.home() / "github" / "deepgraphai" / "GraphLite"
sdk_path = graphlite_repo / "python-sdk"

# Check if SDK exists
if not sdk_path.exists():
    print(f"ERROR: Python SDK not found at {sdk_path}")
    print("Please checkout the python-sdk branch from deepgraphai/GraphLite")
    print("Or update the path in this script to point to your python-sdk location")
    sys.exit(1)

sys.path.insert(0, str(sdk_path))

from src.connection import GraphLite
from src.error import GraphLiteError, ConnectionError, SessionError, QueryError


def main():
    print("=== GraphLite High-Level SDK Drug Discovery Example ===\n")

    # Step 1: Open database
    print("1. Opening database...")
    db_path = "./drug_discovery_highlevel_sdk_db"

    # Clean up old database if exists
    if os.path.exists(db_path):
        shutil.rmtree(db_path)

    try:
        # Using high-level SDK: .open() static method
        db = GraphLite.open(db_path)
        print("   ✓ Database opened\n")

        # Step 2: Create session
        print("2. Creating session...")
        # Using high-level SDK: returns Session object (not session ID)
        session = db.session("researcher")
        print("   ✓ Session created\n")

        # Step 3: Setup schema and graph
        print("3. Setting up schema and graph...")
        # Using high-level SDK: execute() method on session object
        session.execute("CREATE SCHEMA IF NOT EXISTS /drug_discovery")
        session.execute("SESSION SET SCHEMA /drug_discovery")
        session.execute("CREATE GRAPH IF NOT EXISTS pharma_research")
        session.execute("SESSION SET GRAPH pharma_research")
        print("   ✓ Schema and graph configured\n")

        # Step 4: Insert data
        print("4. Inserting pharmaceutical data...")

        # Insert Proteins (Disease Targets)
        print("   → Inserting target proteins...")
        session.execute("""INSERT
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
            })""")

        # Insert Compounds
        print("   → Inserting drug compounds...")
        session.execute("""INSERT
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
            })""")

        # Insert Assays
        print("   → Inserting experimental assays...")
        session.execute("""INSERT
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
            })""")

        print("   ✓ Core data inserted\n")

        # Step 5: Create relationships
        print("5. Creating relationships...")

        # Compound-Assay relationships
        print("   → Linking compounds to assays...")
        session.execute("""MATCH (c:Compound {id: 'CP-002'}), (a:Assay {id: 'AS-001'})
           INSERT (c)-[:TESTED_IN {
               test_date: '2024-01-15',
               concentration_range: '0.1-1000 nM',
               replicate_count: 3
           }]->(a)""")

        session.execute("""MATCH (c:Compound {id: 'CP-003'}), (a:Assay {id: 'AS-002'})
           INSERT (c)-[:TESTED_IN {
               test_date: '2024-02-20',
               concentration_range: '1-10000 nM',
               replicate_count: 4
           }]->(a)""")

        session.execute("""MATCH (c:Compound {id: 'CP-004'}), (a:Assay {id: 'AS-003'})
           INSERT (c)-[:TESTED_IN {
               test_date: '2024-03-10',
               concentration_range: '0.5-500 nM',
               replicate_count: 3
           }]->(a)""")

        session.execute("""MATCH (c:Compound {id: 'CP-005'}), (a:Assay {id: 'AS-004'})
           INSERT (c)-[:TESTED_IN {
               test_date: '2024-03-25',
               concentration_range: '1-1000 nM',
               replicate_count: 5
           }]->(a)""")

        # Assay-Protein relationships
        print("   → Linking assays to proteins...")
        session.execute("""MATCH (a:Assay {id: 'AS-001'}), (p:Protein {id: 'EGFR'})
           INSERT (a)-[:MEASURES_ACTIVITY_ON {
               readout: 'Kinase inhibition',
               units: 'percent inhibition'
           }]->(p)""")

        session.execute("""MATCH (a:Assay {id: 'AS-002'}), (p:Protein {id: 'ACE2'})
           INSERT (a)-[:MEASURES_ACTIVITY_ON {
               readout: 'Binding affinity',
               units: 'KD (nM)'
           }]->(p)""")

        session.execute("""MATCH (a:Assay {id: 'AS-003'}), (p:Protein {id: 'BACE1'})
           INSERT (a)-[:MEASURES_ACTIVITY_ON {
               readout: 'Enzymatic activity',
               units: 'percent inhibition'
           }]->(p)""")

        session.execute("""MATCH (a:Assay {id: 'AS-004'}), (p:Protein {id: 'TP53'})
           INSERT (a)-[:MEASURES_ACTIVITY_ON {
               readout: 'PPI disruption',
               units: 'IC50 (nM)'
           }]->(p)""")

        # Direct inhibition relationships with IC50 data
        print("   → Creating inhibition relationships with IC50 data...")
        session.execute("""MATCH (c:Compound {id: 'CP-002'}), (p:Protein {id: 'EGFR'})
           INSERT (c)-[:INHIBITS {
               IC50: 37.5,
               IC50_unit: 'nM',
               Ki: 12.3,
               selectivity_index: 25.6,
               measurement_date: '2024-01-15'
           }]->(p)""")

        session.execute("""MATCH (c:Compound {id: 'CP-003'}), (p:Protein {id: 'ACE2'})
           INSERT (c)-[:INHIBITS {
               IC50: 23.0,
               IC50_unit: 'nM',
               Ki: 7.8,
               selectivity_index: 15.2,
               measurement_date: '2024-02-20'
           }]->(p)""")

        session.execute("""MATCH (c:Compound {id: 'CP-004'}), (p:Protein {id: 'BACE1'})
           INSERT (c)-[:INHIBITS {
               IC50: 85.0,
               IC50_unit: 'nM',
               Ki: 28.5,
               selectivity_index: 45.1,
               measurement_date: '2024-03-10'
           }]->(p)""")

        session.execute("""MATCH (c:Compound {id: 'CP-005'}), (p:Protein {id: 'TP53'})
           INSERT (c)-[:INHIBITS {
               IC50: 12.5,
               IC50_unit: 'nM',
               Ki: 3.2,
               selectivity_index: 120.5,
               measurement_date: '2024-03-25'
           }]->(p)""")

        print("   ✓ Relationships created\n")

        # Step 6: Execute analytical queries
        print("6. Running analytical queries...\n")

        # Query 1: Find potent compounds for TP53
        print("   Query 1: Compounds targeting TP53 with IC50 < 100 nM")
        # Using high-level SDK: query() method on session object
        result = session.query(
            """MATCH (c:Compound)-[i:INHIBITS]->(p:Protein {id: 'TP53'})
               WHERE i.IC50 < 100
               RETURN c.name, c.id, i.IC50, i.IC50_unit, i.Ki
               ORDER BY i.IC50""")

        print("   Results:")
        for row in result.rows:
            print(f"     - {row}")
        print()

        # Query 2: Complete testing pathway
        print("   Query 2: Complete testing pathway for Gefitinib")
        result = session.query(
            """MATCH (c:Compound {id: 'CP-002'})-[t:TESTED_IN]->(a:Assay)-[m:MEASURES_ACTIVITY_ON]->(p:Protein)
               RETURN c.name, a.name, a.assay_type, p.name, p.disease""")

        print("   Results:")
        for row in result.rows:
            print(f"     {row}")
        print()

        # Query 3: All compound-target interactions sorted by potency
        print("   Query 3: All compound-target interactions sorted by potency")
        result = session.query(
            """MATCH (c:Compound)-[i:INHIBITS]->(p:Protein)
               RETURN c.name AS Compound,
                      p.name AS Target,
                      p.disease AS Disease,
                      i.IC50 AS IC50_nM,
                      c.development_stage AS Stage
               ORDER BY i.IC50""")

        print(f"   Columns: {result.variables}")
        print("   Results:")
        for row in result.rows:
            print(f"     {row}")
        print()

        # Query 4: Compounds in clinical trials
        print("   Query 4: Clinical trial compounds and their targets")
        result = session.query(
            """MATCH (c:Compound)-[i:INHIBITS]->(p:Protein)
               WHERE c.development_stage LIKE '%Clinical Trial%'
               RETURN c.name AS Compound,
                      c.development_stage AS Stage,
                      p.name AS Target,
                      i.IC50 AS Potency_nM,
                      i.selectivity_index AS Selectivity""")

        print("   Results:")
        for row in result.rows:
            print(f"     {row}")
        print()

        # Query 5: Proteins with multiple targeting compounds (aggregation)
        print("   Query 5: Proteins with multiple targeting compounds")
        result = session.query(
            """MATCH (p:Protein)<-[:INHIBITS]-(c:Compound)
               RETURN p.name AS Protein,
                      p.disease AS Disease,
                      COUNT(c) AS CompoundCount""")

        print("   Results:")
        for row in result.rows:
            print(f"     {row}")
        print()

        # Step 7: Summary
        print("=== Drug Discovery Example Complete ===")
        print("\nKey Insights:")
        print("  • Modeled 4 node types: Protein, Compound, Assay")
        print("  • Created relationship types: TESTED_IN, MEASURES_ACTIVITY_ON, INHIBITS")
        print("  • Demonstrated graph traversals for drug discovery workflows")
        print("  • Showed IC50-based compound filtering and ranking")
        print("  • Used High-Level SDK features:")
        print("    - Session-centric API (session.query() vs db.query(session_id, ...))")
        print("    - Typed exceptions (ConnectionError, SessionError, QueryError)")
        print("    - Cleaner, more Pythonic interface")
        print(f"\nDatabase location: {db_path}/")
        print(f"To clean up: rm -rf {db_path}/")

        return 0

    except ConnectionError as e:
        print(f"\n❌ Connection Error: {e}")
        return 1
    except SessionError as e:
        print(f"\n❌ Session Error: {e}")
        return 1
    except QueryError as e:
        print(f"\n❌ Query Error: {e}")
        return 1
    except GraphLiteError as e:
        print(f"\n❌ GraphLite Error: {e}")
        return 1
    except Exception as e:
        print(f"\n❌ Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(main())
