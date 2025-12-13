from nl2gql.pipeline.ir import IREdge, IRNode, IRReturn, ISOQueryIR


def test_ir_parse_and_render_roundtrip():
    query = "MATCH (a:Person)-[:KNOWS]->(b:Person) WHERE a.age > 30 RETURN a.name, b.name"

    ir, errors = ISOQueryIR.parse(query)
    assert ir is not None
    assert errors == []

    rendered = ir.render()
    ir2, errors2 = ISOQueryIR.parse(rendered)

    assert ir2 is not None
    assert errors2 == []
    assert any(edge.rel == "KNOWS" for edge in ir2.edges)


def test_ir_having_clause_parse_and_render():
    """Test that HAVING clauses are properly parsed and rendered."""
    query = """MATCH (p:Person)-[:WORKS_AT]->(c:Company)
WITH c, AVG(p.salary) AS avg_sal
HAVING avg_sal > 100000
RETURN c.name, avg_sal
ORDER BY avg_sal DESC
LIMIT 5"""

    ir, errors = ISOQueryIR.parse(query)
    assert ir is not None
    assert errors == []
    
    # Check HAVING was parsed correctly
    assert ir.having_filters == ["avg_sal > 100000"]
    assert ir.with_items == ["c", "AVG(p.salary) AS avg_sal"]
    
    # Verify roundtrip preserves HAVING
    rendered = ir.render()
    assert "HAVING avg_sal > 100000" in rendered
    
    # Re-parse and verify
    ir2, errors2 = ISOQueryIR.parse(rendered)
    assert ir2 is not None
    assert errors2 == []
    assert ir2.having_filters == ["avg_sal > 100000"]


def test_ir_having_multiple_conditions():
    """Test HAVING with multiple AND conditions."""
    query = """MATCH (p:Person)-[:WORKS_AT]->(c:Company)
WITH c.name AS company_name, AVG(p.salary) AS avg_sal, COUNT(p.id) AS headcount
HAVING avg_sal > 100000 AND headcount >= 10
RETURN company_name, avg_sal, headcount"""

    ir, errors = ISOQueryIR.parse(query)
    assert ir is not None
    assert errors == []
    
    # Check multiple HAVING conditions were parsed
    assert len(ir.having_filters) == 2
    assert "avg_sal > 100000" in ir.having_filters
    assert "headcount >= 10" in ir.having_filters
    
    rendered = ir.render()
    assert "HAVING avg_sal > 100000 AND headcount >= 10" in rendered


def test_ir_render_group_without_with_for_simple_aggregates():
    """Ensure grouped queries render without WITH when possible (GraphLite compat)."""
    ir = ISOQueryIR(
        nodes={
            "c1": IRNode(alias="c1", label="Company"),
            "c2": IRNode(alias="c2", label="City"),
            "p1": IRNode(alias="p1", label="Person"),
        },
        edges=[
            IREdge(left_alias="c1", rel="LOCATED_IN", right_alias="c2"),
            IREdge(left_alias="p1", rel="WORKS_AT", right_alias="c1"),
        ],
        filters=[],
        with_items=[
            "c1.id AS company_id",
            "c1.name AS company_name",
            "COUNT(p1.id) AS headcount",
            "AVG(p1.age) AS average_age",
        ],
        group_by=["company_id", "company_name"],
        returns=[
            IRReturn(expr="company_name"),
            IRReturn(expr="company_id"),
            IRReturn(expr="headcount"),
            IRReturn(expr="average_age"),
        ],
        order_by=[],
    )

    rendered = ir.render()

    # The compatibility path should avoid a WITH clause and collapse MATCH into a single line.
    assert "WITH " not in rendered
    assert "GROUP BY c1.id, c1.name" in rendered
    assert rendered.startswith("MATCH (c1:Company)-[:LOCATED_IN]->(c2:City), (p1:Person)-[:WORKS_AT]->(c1:Company)")
    # Grouping should appear after RETURN for GraphLite compatibility.
    return_idx = rendered.find("RETURN")
    group_idx = rendered.find("GROUP BY")
    assert return_idx != -1 and group_idx != -1 and return_idx < group_idx


def test_ir_render_group_allows_plain_with_items():
    """Compatibility mode should still trigger when WITH items include bare expressions."""
    ir = ISOQueryIR(
        nodes={
            "c1": IRNode(alias="c1", label="Company"),
            "c2": IRNode(alias="c2", label="City"),
            "p1": IRNode(alias="p1", label="Person"),
        },
        edges=[
            IREdge(left_alias="c1", rel="LOCATED_IN", right_alias="c2"),
            IREdge(left_alias="p1", rel="WORKS_AT", right_alias="c1"),
        ],
        with_items=[
            "c1.name",  # plain expression, no alias
            "c1.name AS company_name",
            "COUNT(p1.id) AS headcount",
        ],
        group_by=["company_name"],
        returns=[IRReturn(expr="company_name"), IRReturn(expr="headcount")],
    )

    rendered = ir.render()
    assert "WITH " not in rendered
    assert "GROUP BY c1.name" in rendered or "GROUP BY company_name" in rendered
    assert rendered.find("RETURN") < rendered.find("GROUP BY")
