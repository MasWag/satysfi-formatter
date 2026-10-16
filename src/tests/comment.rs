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
fn assert_comment_roundtrip(input: &str) {
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
    let original = before.cst.clone();
    crate::comment::with_comments(&before, &before.cst);
    assert_eq!(before.cst, original, "adapter must not mutate the CST");
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
}

#[test]
fn comments_at_expression_boundaries() {
    for input in [
        "% first\nlet x = 1 % trailing\n",
        "let x = if true % condition\nthen % then\n1 % branch\nelse % else\n2\n",
        "let x = f % function\n1 % argument\n2\n",
        "let x = [1; % element\n2; % last\n]\n",
        "let x = [f% c\n1]\n",
        "let x = f (% first\n1) (% second\n2)\n",
        "let x = [1 % before separator\n; 2]\n",
    ] {
        assert_comment_roundtrip(input);
    }
}

#[test]
fn comments_in_empty_structures() {
    for input in [
        "let x = [% empty\n]\n",
        "let x = (|% empty\n|)\n",
        "let x = {% empty\n}\n",
        "let x = '<% empty\n>\n",
    ] {
        assert_comment_roundtrip(input);
    }
}

#[test]
fn percent_tokens_are_not_comments() {
    for input in [
        "let x = `100%` % real\n",
        "let x = {100\\% done % real\n}\n",
        "let x = {\\% % real\n}\n",
        "@require: package%name\nlet x = 1\n",
    ] {
        assert_comment_roundtrip(input);
    }
}

#[test]
fn eof_comments_and_blank_lines() {
    for input in [
        "let x = 1 % eof",
        "let x = 1\n% eof",
        "% first\n\n% second\n\nlet x = 1\n\n% third\n% fourth\n",
    ] {
        assert_comment_roundtrip(input);
    }
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
    test_tmpl(
        "let x = if true % condition\nthen % then\n1 % branch\nelse % else\n2\n",
        "let x =\n    if true\n    % condition\n\n    then\n        % then\n        1\n    % branch\n\n    else\n        % else\n        2\n",
    );
}

#[test]
fn empty_list_comment_keeps_closing_delimiter_on_its_own_line() {
    test_tmpl(
        "let x = [% empty\n]\n",
        "let x =\n    [\n        % empty\n    ]\n",
    );
}

#[test]
fn lsp_formatting_uses_the_same_comment_adapter() {
    let input = "let x = `100%` % eof";
    let edits = crate::formatting(input, super::default_option());
    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].new_text,
        crate::format(input, super::default_option())
    );
}
