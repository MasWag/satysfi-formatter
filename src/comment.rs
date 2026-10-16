use satysfi_parser::{Cst, CstText, Item, Rule};

/// Build a formatting-only tree, leaving the parser's CST unchanged.
/// The renderer still understands `Rule::comments`; only this adapter creates them.
pub fn with_comments(source: &CstText, node: &Cst) -> Cst {
    let mut inner = Vec::new();
    for item in source.items(node) {
        match item {
            Item::Node(child) => inner.push(with_comments(source, child)),
            Item::Comment { span, .. } => inner.push(Cst {
                rule: Rule::comments,
                span,
                inner: Vec::new(),
            }),
            // Keep the existing renderer's blank-line normalization. These gaps
            // belong here, between the adjacent items, rather than to a child.
            Item::BlankLines(_) => {}
        }
    }
    Cst {
        rule: node.rule,
        span: node.span,
        inner,
    }
}

pub fn contains_comment(node: &Cst) -> bool {
    node.rule == Rule::comments || node.inner.iter().any(contains_comment)
}

/// コメントを文字列化する関数
pub fn to_comment_string(text: String) -> String {
    let comment = text
        .strip_prefix('%')
        .expect("parser comment spans start with %")
        .trim_end();
    if comment.is_empty() || comment.starts_with(char::is_whitespace) || comment.starts_with('%') {
        // 空白or複数の%で始まっていたらそのまま表示
        format!("%{}", comment)
    } else {
        // 空白がない場合は空白を1つ挿入
        format!("% {}", comment)
    }
}
