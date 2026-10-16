use super::test_tmpl;

#[test]
fn test_comment1() {
    let text = r#"@import: hello
  @require: local
  %comment
  
document(|title = {hello}|)'<+p{% comment
}>"#;

    let expect = r#"@import: hello
@require: local

% comment
document(|title = { hello }|)'<
    +p {
        % comment
    }
>
"#;
    test_tmpl(text, expect)
}

#[test]
fn test_comment2() {
    let text = r#"@import: hello
  @require: local
  %comment
  
document(|title = {hello}|)'<+p{hello% comment
}>"#;

    let expect = r#"@import: hello
@require: local

% comment
document(|title = { hello }|)'<
    +p {
        hello
        % comment
    }
>
"#;
    test_tmpl(text, expect)
}

#[test]
fn test_comment3() {
    let text = r#"@import: hello
  @require: local
  %comment
  
document(|title = {hello}|)'<+p{hello% comment
    \listing {
        * item1
        * item2
        * item3
    }
}>"#;

    let expect = r#"@import: hello
@require: local

% comment
document(|title = { hello }|)'<
    +p {
        hello
        % comment
        \listing {
            * item1
            * item2
            * item3
        }
    }
>
"#;
    test_tmpl(text, expect)
}

#[test]
fn test_comment4() {
    let text = r#"@import: hello
  @require: local
  %comment
  
document(|title = { hello }|)'<+p{% comment
    \listing {
        * item1
        * item2
        * item3
    }
}>"#;
    let expect = r#"@import: hello
@require: local

% comment
document(|title = { hello }|)'<
    +p {
        % comment
        \listing {
            * item1
            * item2
            * item3
        }
    }
>
"#;
    test_tmpl(text, expect)
}

#[test]
fn test_comment5() {
    let text = r#"@import: hello
  @require: local %comment
  
document(|title = {hello}|)'<+p% comment
{
    hello
}>"#;

    let expect = r#"@import: hello
@require: local %comment

document(|title = { hello }|)'<
    +p % comment
    { hello }
>
"#;
    test_tmpl(text, expect)
}

#[test]
fn test_comment6() {
    let text = r#"@import: hello
  @require: local %comment
  
document(|title = {hello}|)'<+p% comment
{
    hello
    \\%comment
}>"#;

    let expect = r#"@import: hello
@require: local %comment

document(|title = { hello }|)'<
    +p % comment
    {
        hello
        \\
        % comment
    }
>
"#;
    test_tmpl(text, expect)
}

#[test]
fn test_comment7() {
    let text = r#"@import: hello
  @require: local %comment
  
document(|title = {hello}|)'<
% comment
+p{hello}
% comment
>"#;

    let expect = r#"@import: hello
@require: local %comment

document(|title = { hello }|)'<
    % comment
    +p { hello }
    % comment
>
"#;
    test_tmpl(text, expect)
}

#[test]
fn test_comment8() {
    let text = r#"@import: hello
  @require: local %comment
  
document(|title = {hello}|)'<
+section{hello}<%
    +p{
        hello
    }
>%
>"#;

    let expect = r#"@import: hello
@require: local %comment

document(|title = { hello }|)'<
    +section { hello } <%
        +p { hello }
    >%
>
"#;
    test_tmpl(text, expect)
}

/// Check trivia independently of rendering, so a parse-error passthrough cannot
/// make a regression appear to pass. Compare the syntax tree without positions.
fn assert_comment_roundtrip(input: &str) -> String {
    use satysfi_parser::{grammar, Cst, CstText, Item, Rule};
    fn comments(source: &CstText, node: &Cst, out: &mut Vec<String>) {
        for item in source.items(node) {
            match item {
                Item::Node(child) => comments(source, child, out),
                Item::Comment { span, .. } => out.push(crate::comment::to_comment_string(
                    source.get_text_from_span(span).to_string(),
                )),
                Item::BlankLines(_) => {}
            }
        }
    }
    fn syntax(source: &CstText, node: &Cst) -> String {
        let token = if node.inner.is_empty() && source.items(node).is_empty() {
            source.get_text(node).trim().to_string()
        } else {
            String::new()
        };
        // Horizontal whitespace nodes may appear when the formatter wraps text.
        if node.rule == Rule::regular_text && token.is_empty() {
            return String::new();
        }
        format!(
            "({:?} {:?} {})",
            node.rule,
            token,
            node.inner
                .iter()
                .map(|child| syntax(source, child))
                .collect::<String>()
        )
    }
    let before = CstText::parse(input, grammar::program).expect("valid fixture");
    let output = crate::format(input, super::default_option());
    let after = CstText::parse(&output, grammar::program)
        .unwrap_or_else(|e| panic!("invalid output: {e:?}\n{output}"));
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    comments(&before, &before.cst, &mut expected);
    comments(&after, &after.cst, &mut actual);
    assert_eq!(actual, expected, "comments changed in {output}");
    assert_eq!(
        syntax(&before, &before.cst),
        syntax(&after, &after.cst),
        "syntax changed in {output}"
    );
    assert_eq!(
        crate::format(&output, super::default_option()),
        output,
        "not idempotent for {input}"
    );
    output
}

#[test]
fn comments_at_expression_boundaries() {
    for input in [
        // Exercise both sides of a list separator in one fixture.
        "let x = [1 % before separator\n; % after separator\n2]\n",
        // A nested argument comment must also prevent short-list compaction.
        "let x = [f% c\n1]\n",
        // Comments must remain inside argument parentheses on the second pass.
        "let x = f (% first\n1) (% second\n2)\n",
    ] {
        assert_comment_roundtrip(input);
    }
}

#[test]
fn comments_in_empty_structures() {
    assert_eq!(
        assert_comment_roundtrip("let x = [% empty\n]\n"),
        "let x =\n    [\n        % empty\n    ]\n",
    );
    assert_comment_roundtrip("let x = (|% empty\n|)\n");
    // Text and block comments already have exact-output coverage above.
}

#[test]
fn percent_tokens_are_not_comments() {
    // Also cover a trailing EOF comment without a final newline.
    assert_comment_roundtrip("let x = `100%` % real");
    assert_comment_roundtrip("let x = {100\\% done % real\n}\n");
}

#[test]
fn eof_comments_and_blank_lines() {
    // First-line, consecutive and separated comments, plus an own-line EOF
    // comment without a final newline, share the same traversal regression.
    assert_comment_roundtrip("% first\n\n% second\n\nlet x = 1\n\n% third\n% fourth");
}

#[test]
fn parser_compatibility_preserves_new_syntax() {
    for input in [
        "let x = f 1 before % sequence\ng 2\n",
        "let x = <[(0, 0) -- % line\n(1, 1) -- cycle]>\n",
        "let x = <[(0, 0) .. controls (1, 1) and (2, 2) .. (3, 3) .. controls (4, 4) and (5, 5) .. cycle]>\n",
    ] { assert_comment_roundtrip(input); }
}

#[test]
fn comment_positions_around_branches() {
    assert_eq!(
        assert_comment_roundtrip("let x = if true % condition\nthen % then\n1 % branch\nelse % else\n2\n"),
        "let x =\n    if true\n    % condition\n\n    then\n        % then\n        1\n    % branch\n\n    else\n        % else\n        2\n",
    );
}
