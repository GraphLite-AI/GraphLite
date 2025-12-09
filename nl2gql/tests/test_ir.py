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



