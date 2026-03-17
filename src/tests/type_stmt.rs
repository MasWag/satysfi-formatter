use super::test_tmpl;

#[test]
fn single_line_variant_without_leading_bar() {
    let text = r#"type t = | Center | North | South"#;
    let expect = r#"type t = Center | North | South
"#;
    test_tmpl(text, expect);
}

#[test]
fn single_line_variant_with_payload() {
    let text = r#"type point-source = | Point of point | AnchorPoint of Node.t * Anchor.t"#;
    let expect = r#"type point-source = Point of point | AnchorPoint of Node.t * Anchor.t
"#;
    test_tmpl(text, expect);
}

#[test]
fn multiline_variant_preserved() {
    let text = r#"type t =
  | Center
  | North
  | South"#;
    let expect = r#"type t =
    | Center
    | North
    | South
"#;
    test_tmpl(text, expect);
}

#[test]
fn multiline_variant_with_and() {
    let text = r#"type t =
  | Center
  | North
and u = int"#;
    let expect = r#"type t =
    | Center
    | North
and u = int
"#;
    test_tmpl(text, expect);
}
