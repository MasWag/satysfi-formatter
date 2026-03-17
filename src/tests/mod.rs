use crate::format;
use lspower::lsp::{FormattingOptions, FormattingProperty};
use unicode_width::UnicodeWidthStr;

mod comment;
mod common;
mod ctrl_stmt;
mod horizontal_single;
mod let_block;
mod line_width;
mod math;
mod module;
mod space;

fn default_option() -> FormattingOptions {
    FormattingOptions {
        tab_size: 4,
        insert_spaces: true,
        ..Default::default()
    }
}

fn option_with_line_width(width: i32) -> FormattingOptions {
    let mut option = default_option();
    option
        .properties
        .insert("lineWidth".to_string(), FormattingProperty::Number(width));
    option
}

fn assert_max_line_width(output: &str, width: usize) {
    for (index, line) in output.lines().enumerate() {
        assert!(
            UnicodeWidthStr::width(line) <= width,
            "line {} exceeds width {}: {:?}",
            index + 1,
            width,
            line,
        );
    }
}

fn test_tmpl_with_option(input: &str, expect: &str, option: FormattingOptions) {
    let output = format(input, option);
    assert_eq!(output, expect);
}

fn test_tmpl(input: &str, expect: &str) {
    test_tmpl_with_option(input, expect, default_option());
}

#[test]
fn test_unicode() {
    let text = r#"

document(||)'<
+section{ section }<
+p {日本語}
>>"#;

    let expect = r#"document(||)'<
    +section { section } <
        +p { 日本語 }
    >
>
"#;
    test_tmpl(text, expect)
}
