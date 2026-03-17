use crate::comment::{get_comments, to_comment_string, Comment};
use crate::reserved_words::*;
use lspower::lsp::{FormattingOptions, FormattingProperty};
use satysfi_parser::{Cst, CstText, Rule};
use std::collections::VecDeque;
use unicode_width::UnicodeWidthStr;

const DEFAULT_LINE_WIDTH: usize = 120;

pub struct Formatter<'a> {
    pub text: &'a str,
    pub lines: &'a Vec<usize>,
    pub comments: VecDeque<Comment>,
    pub depth: usize,
    pub output: String,
    option: FormattingOptions,
}

impl<'a> Formatter<'a> {
    pub fn new(csttext: &'a CstText, option: FormattingOptions) -> Self {
        let comments = get_comments(csttext);
        Self {
            text: &csttext.text,
            lines: &csttext.lines,
            comments,
            depth: 0,
            output: String::new(),
            option,
        }
    }

    /// 文字列を format して出力する
    /// 前処理後処理もここで行う
    pub fn format(&self, input: &str, cst: &Cst, depth: usize) -> String {
        let mut output = self.to_string_cst(input, cst, depth);
        // 末尾スペースを全て除去
        output = output
            .split('\n')
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n");

        // 末尾に改行がない場合、改行を挿入して終了
        if self.option.insert_final_newline.unwrap_or(true) && !output.ends_with('\n') {
            output += "\n";
        }
        output
    }

    fn line_width(&self) -> usize {
        match self.option.properties.get("lineWidth") {
            Some(FormattingProperty::Number(value)) if *value > 0 => *value as usize,
            _ => DEFAULT_LINE_WIDTH,
        }
    }

    fn max_line_width(&self, text: &str) -> usize {
        text.lines().map(UnicodeWidthStr::width).max().unwrap_or(0)
    }

    fn line_fits(&self, depth: usize, extra_prefix_width: usize, text: &str) -> bool {
        let used_width = self.option.tab_size as usize * depth + extra_prefix_width;
        used_width + self.max_line_width(text) <= self.line_width()
    }

    fn append_indented_rendered(&self, current: String, rendered: &str, depth: usize) -> String {
        let prefix = format!(
            "\n{}",
            indent_space(self.option.tab_size as usize, depth + 1)
        );
        if current.ends_with(&prefix) {
            current + rendered.trim_start()
        } else {
            current + &prefix + rendered.trim_start()
        }
    }

    fn allows_inline_multiline_expr(&self, rendered: &str) -> bool {
        rendered.starts_with("'<")
            || rendered.starts_with('{')
            || rendered.starts_with('[')
            || rendered.starts_with("(|")
    }

    fn join_rendered_expr_with_wrap(
        &self,
        text: &str,
        expr_cst: &Cst,
        current: String,
        depth: usize,
        inline_sep: &str,
        break_sep: &str,
        extra_prefix_width: usize,
        inline_expr: &str,
        force_wrap: bool,
    ) -> String {
        let force_wrap = force_wrap
            || current.contains('\n')
            || (inline_expr.contains('\n') && !self.allows_inline_multiline_expr(inline_expr));
        if !force_wrap
            && self.line_fits(
                depth,
                extra_prefix_width,
                &(current.clone() + inline_sep + inline_expr),
            )
        {
            current + inline_sep + inline_expr
        } else {
            let rendered_expr = self.to_string_cst(text, expr_cst, depth + 1);
            self.append_indented_rendered(current + break_sep, &rendered_expr, depth)
        }
    }

    fn join_expr_with_wrap(
        &self,
        text: &str,
        expr_cst: &Cst,
        current: String,
        depth: usize,
        inline_sep: &str,
        break_sep: &str,
        extra_prefix_width: usize,
    ) -> String {
        let inline_expr = self.to_string_cst(text, expr_cst, depth);
        self.join_rendered_expr_with_wrap(
            text,
            expr_cst,
            current,
            depth,
            inline_sep,
            break_sep,
            extra_prefix_width,
            &inline_expr,
            false,
        )
    }

    fn is_structured_match_arm_expr(&self, cst: &Cst) -> bool {
        let cst = if cst.rule == Rule::expr {
            cst.inner.first().unwrap_or(cst)
        } else {
            cst
        };

        matches!(cst.rule, Rule::bind_stmt | Rule::lambda | Rule::match_expr)
    }

    fn format_match_arm_rhs(
        &self,
        text: &str,
        expr_cst: &Cst,
        current: String,
        depth: usize,
        extra_prefix_width: usize,
    ) -> String {
        let inline_expr = self.to_string_cst(text, expr_cst, depth);
        self.join_rendered_expr_with_wrap(
            text,
            expr_cst,
            current,
            depth,
            " -> ",
            " ->",
            extra_prefix_width,
            &inline_expr,
            self.is_structured_match_arm_expr(expr_cst),
        )
    }

    fn should_break_assignment_rhs(&self, rendered: &str) -> bool {
        rendered.starts_with("let")
            || (!rendered.starts_with("'<") && !rendered.starts_with('{'))
                && rendered.contains('\n')
    }

    fn format_assignment_rhs(
        &self,
        text: &str,
        expr_cst: &Cst,
        rendered: &str,
        current: String,
        binding_depth: usize,
        extra_prefix_width: usize,
    ) -> String {
        self.join_rendered_expr_with_wrap(
            text,
            expr_cst,
            current,
            binding_depth,
            " = ",
            " =",
            extra_prefix_width,
            rendered,
            self.should_break_assignment_rhs(rendered),
        )
    }

    fn application_flat_output(
        &self,
        text: &str,
        csts: &[Cst],
        depth: usize,
    ) -> (String, bool, bool) {
        let first_text = self.to_string_cst(text, &csts[0], depth);
        let insert_space = first_text != "document";
        let mut force_multiline = first_text.contains('\n');
        let mut output = first_text;

        for cst in csts.iter().skip(1) {
            let s = self.to_string_cst(text, cst, depth);
            force_multiline |= cst.rule == Rule::comments;
            if insert_space {
                output += " ";
            }
            output += &s;
        }

        (output, force_multiline, insert_space)
    }
    /// cst の inner の要素を結合して文字列に変換する関数
    fn to_string_cst_inner(&self, text: &str, cst: &Cst, depth: usize) -> String {
        /*
        Cst {
            rule: Rule,
            span: Span { start: number, end: number },
            inner: [Cst]
        }
        */
        let csts = cst.inner.clone();
        // 関数内で改行するときはこれを使用する
        let indent = indent_space(self.option.tab_size as usize, depth);
        let newline = format!("\n{indent}");
        let sep = &match cst.rule {
            Rule::block_cmd | Rule::inline_cmd => " ".to_string(),
            // Rule::type_application => " ".to_string(),
            Rule::type_prod => " * ".to_string(),
            Rule::dyadic_expr | Rule::match_expr | Rule::unary_operator_expr => " ".to_string(),
            Rule::vertical | Rule::horizontal_bullet_list => newline.clone(),
            Rule::horizontal_list => format!("{newline}|"),
            Rule::unary => "#".to_string(),
            Rule::type_optional => " ?-> ".to_string(),
            Rule::list => format!(";{newline}"),
            Rule::tuple => ", ".to_string(),
            Rule::record | Rule::type_record => newline.clone(),
            Rule::type_block_cmd | Rule::type_inline_cmd | Rule::type_math_cmd => {
                format!(";{newline}")
            }
            Rule::horizontal_single => "".to_string(),
            // Rule::variant_constructor => " ".to_string(),
            Rule::program_saty | Rule::program_satyh => newline.clone(),
            _ => " ".to_string(),
        };

        let output = match cst.rule {
            Rule::variant_constructor => {
                let mut output = String::new();
                for cst in csts {
                    let s = self.to_string_cst(text, &cst, depth);
                    if output.is_empty() {
                        output = s;
                    } else if !(cst.rule == Rule::unary && s.starts_with('(')) {
                        output += sep;
                        output += &s;
                    } else {
                        output += &s;
                    }
                }
                output
            }
            Rule::let_mutable_stmt => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    return s;
                }
                match now_cst.rule {
                    Rule::var => current + " " + &s,
                    Rule::expr => self.join_expr_with_wrap(
                        text,
                        now_cst,
                        current,
                        depth,
                        " <- ",
                        " <-",
                        UnicodeWidthStr::width(RESERVED_WORD.let_mutable) + 1,
                    ),
                    Rule::comments => current + &s,
                    _ => unreachable!(),
                }
            }),
            Rule::type_stmt => {
                let rendered = csts
                    .iter()
                    .map(|now_cst| (now_cst.rule, self.to_string_cst(text, now_cst, depth)))
                    .collect::<Vec<_>>();
                let cnt = rendered
                    .iter()
                    .filter(|(rule, _)| *rule == Rule::type_inner)
                    .count();
                let break_and = cnt > 2
                    || rendered
                        .iter()
                        .any(|(rule, s)| *rule == Rule::type_inner && s.contains('\n'));
                rendered
                    .into_iter()
                    .fold(String::new(), |current, (rule, s)| {
                        if current.is_empty() {
                            return s;
                        }
                        match rule {
                            Rule::type_inner => {
                                if break_and {
                                    current + &newline + RESERVED_WORD.and + " " + &s
                                } else {
                                    current + " and " + &s
                                }
                            }
                            Rule::comments => current + &s,
                            _ => unreachable!(),
                        }
                    })
            }
            Rule::let_rec_stmt => {
                let mut cnt = 0;
                let output = csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    if current.is_empty() {
                        if now_cst.rule != Rule::comments {
                            cnt += 1;
                        }
                        return s;
                    }
                    match now_cst.rule {
                        Rule::let_rec_inner => {
                            cnt += 1;
                            if cnt > 2 {
                                current + " and " + &s
                            } else {
                                current + &s
                            }
                        }
                        Rule::comments => current + &s,
                        _ => unreachable!(),
                    }
                });
                if cnt > 2 {
                    output
                        .split(" and ")
                        .collect::<Vec<_>>()
                        .join((newline + &indent + RESERVED_WORD.and).as_str())
                } else {
                    output
                }
            }
            Rule::sig_val_stmt | Rule::sig_direct_stmt => {
                csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    let s = if cst.rule == Rule::sig_val_stmt && now_cst.rule == Rule::bin_operator
                    {
                        format!("({s})")
                    } else {
                        s
                    };
                    if current.is_empty() {
                        return s;
                    }
                    match now_cst.rule {
                        Rule::var => current + " " + &s,
                        Rule::bin_operator => current + &format!(" ({s})"),
                        Rule::inline_cmd_name => current + " " + &s,
                        Rule::block_cmd_name => current + " " + &s,
                        Rule::type_expr => current + ": " + &s,
                        Rule::comments => {
                            if current.ends_with(char::is_whitespace) {
                                current + &s
                            } else {
                                current + &newline + &s
                            }
                        }
                        _ => current + " " + &s,
                    }
                })
            }
            Rule::let_block_stmt_ctx
            | Rule::let_block_stmt_noctx
            | Rule::let_inline_stmt_ctx
            | Rule::let_inline_stmt_noctx
            | Rule::let_stmt
            | Rule::let_math_stmt => {
                csts.iter()
                    .enumerate()
                    .fold(String::new(), |current, (index, now_cst)| {
                        let s = self.to_string_cst(text, now_cst, depth);
                        let s = if cst.rule == Rule::sig_val_stmt
                            && now_cst.rule == Rule::bin_operator
                        {
                            format!("({s})")
                        } else {
                            s
                        };
                        if current.is_empty() {
                            return s;
                        }
                        match now_cst.rule {
                            Rule::var => current + " " + &s,
                            Rule::block_cmd_name => current + " " + &s,
                            Rule::bin_operator => current + &format!(" ({s})"),
                            Rule::type_expr => current + ": " + &s,
                            Rule::constraint => {
                                // 1つインデントを深くする
                                let s = self.to_string_cst(text, now_cst, depth + 1);
                                self.append_indented_rendered(current, &s, depth)
                            }
                            Rule::expr => {
                                // 直前がコメント
                                if index > 0 && csts[index - 1].rule == Rule::comments {
                                    // 1つインデントを深くする
                                    let s = self.to_string_cst(text, now_cst, depth + 1);
                                    current + &s
                                } else {
                                    let extra_prefix_width = match cst.rule {
                                        Rule::let_block_stmt_ctx | Rule::let_block_stmt_noctx => {
                                            UnicodeWidthStr::width(RESERVED_WORD.let_block) + 1
                                        }
                                        Rule::let_inline_stmt_ctx | Rule::let_inline_stmt_noctx => {
                                            UnicodeWidthStr::width(RESERVED_WORD.let_inline) + 1
                                        }
                                        Rule::let_math_stmt => {
                                            UnicodeWidthStr::width(RESERVED_WORD.let_math) + 1
                                        }
                                        Rule::let_stmt => {
                                            UnicodeWidthStr::width(RESERVED_WORD.let_stmt) + 1
                                        }
                                        _ => 0,
                                    };
                                    self.format_assignment_rhs(
                                        text,
                                        now_cst,
                                        &s,
                                        current,
                                        depth,
                                        extra_prefix_width,
                                    )
                                }
                            }
                            Rule::comments => {
                                if index + 1 < csts.len() && csts[index + 1].rule == Rule::expr {
                                    // 1つインデントを深くする
                                    let s = self.to_string_cst(text, now_cst, depth + 1);
                                    self.append_indented_rendered(current + " =", &s, depth)
                                } else {
                                    current + &s
                                }
                            }
                            _ => current + " " + &s,
                        }
                    })
            }
            Rule::math_cmd_expr_arg | Rule::math_cmd_expr_option => {
                // 高々1つの要素
                csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    match now_cst.rule {
                        Rule::math_list | Rule::math_single => current + &format!("{{ {s} }}"),
                        Rule::horizontal_list
                        | Rule::horizontal_bullet_list
                        | Rule::horizontal_single => current + &format!("!{{ {s} }}"),
                        Rule::vertical => current + &format!("!{s}"),
                        Rule::expr => current + &format!("!({s})"),
                        Rule::record | Rule::list => current + &format!("!{s}"),
                        Rule::comments => current + &s,
                        _ => unreachable!(),
                    }
                })
            }
            Rule::cmd_expr_option => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                let s = if now_cst.rule == Rule::expr {
                    format!("({s})")
                } else {
                    s
                };
                if current.is_empty() {
                    return s;
                }
                match now_cst.rule {
                    Rule::expr => current + &s,
                    _ => current + sep + &s,
                }
            }),
            Rule::pat_cons => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    return s;
                }
                match now_cst.rule {
                    Rule::pattern => {
                        if s.starts_with('(') {
                            current + &s
                        } else {
                            current + " " + &s
                        }
                    }
                    Rule::pat_variant => current + " " + &s,
                    Rule::pat_as => current + " :: " + &s,
                    Rule::comments => current + &newline + &s,
                    _ => unreachable!(),
                }
            }),
            Rule::pat_variant => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    return s;
                }
                match now_cst.rule {
                    Rule::pattern => {
                        if s.starts_with('(') {
                            current + &s
                        } else {
                            current + " " + &s
                        }
                    }
                    Rule::variant_name => current + " " + &s,
                    Rule::comments => current + &newline + &s,
                    _ => unreachable!(),
                }
            }),
            Rule::constraint => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    return s;
                }
                match now_cst.rule {
                    Rule::type_param => current + " " + &s,
                    Rule::type_record => current + " :: " + &s,
                    Rule::comments => current + &newline + &s,
                    _ => unreachable!(),
                }
            }),
            Rule::record | Rule::type_record => {
                if csts.len() == 1 {
                    return self.to_string_cst(text, &csts[0], depth);
                }
                let mut iter = csts.into_iter().peekable();
                let mut output = String::new();
                while iter.peek().is_some() {
                    let now_cst = &iter.next().unwrap();
                    let s = self.to_string_cst(text, now_cst, depth);
                    let s = if now_cst.rule == Rule::unary {
                        format!("{s} {} ", RESERVED_WORD.with)
                    } else if now_cst.rule == Rule::record_unit
                        || now_cst.rule == Rule::type_record_unit
                    {
                        s + ";"
                    } else {
                        s
                    };
                    match now_cst.rule {
                        Rule::unary => {
                            output += &s;
                            continue;
                        }
                        Rule::record_unit => {
                            output += &s;
                        }
                        Rule::type_record_unit => {
                            output += &s;
                        }
                        Rule::comments => {
                            output += &s;
                        }
                        _ => unreachable!(),
                    };
                    // 次の要素が存在すれば結合
                    let next = iter.peek();
                    if next.is_some()
                        && now_cst.rule != Rule::comments
                        && next.unwrap().rule == Rule::comments
                    {
                        output += sep;
                    } else if next.is_some()
                        && (next.unwrap().rule == Rule::record_unit
                            || next.unwrap().rule == Rule::type_record_unit)
                    {
                        output += sep;
                    } else if next.is_none() && now_cst.rule == Rule::comments {
                        output = output.trim_end().to_string();
                    };
                }
                output
            }
            Rule::type_inner => {
                let rendered = csts
                    .iter()
                    .map(|now_cst| (now_cst.rule, self.to_string_cst(text, now_cst, depth)))
                    .collect::<Vec<_>>();
                let multiline_variant = text
                    .get(cst.span.start..cst.span.end)
                    .unwrap()
                    .trim_end()
                    .contains('\n')
                    || rendered
                        .iter()
                        .any(|(rule, s)| *rule == Rule::type_variant && s.contains('\n'));
                let variant_indent = indent_space(self.option.tab_size as usize, depth + 1);
                let mut has_variant = false;
                rendered
                    .into_iter()
                    .fold(String::new(), |mut current, (rule, s)| {
                        match rule {
                            Rule::type_param => {
                                current += &s;
                            }
                            Rule::type_name => {
                                if current.is_empty() {
                                    current += &(s + " = ");
                                } else {
                                    current += " ";
                                    current += &(s + " = ");
                                }
                            }
                            Rule::type_variant => {
                                if multiline_variant {
                                    current = current.trim_end().to_string();
                                    current += "\n";
                                    current += &variant_indent;
                                    current += "| ";
                                    current += &s;
                                } else if has_variant {
                                    current += " | ";
                                    current += &s;
                                } else {
                                    current += &s;
                                }
                                has_variant = true;
                            }
                            Rule::type_expr => {
                                current += &s;
                            }
                            _ => {
                                current += &s;
                            }
                        }
                        current
                    })
            }
            Rule::type_variant => {
                let output = csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    if current.is_empty() {
                        return s;
                    }
                    match now_cst.rule {
                        Rule::variant_name => current + &s,
                        Rule::type_expr => current + " of " + &s,
                        Rule::comments => current + &s,
                        _ => unreachable!(),
                    }
                });
                output
            }
            Rule::let_rec_inner => {
                // for rule let_rec_stmt_argument()
                let mut type_expr = false;
                csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    if current.is_empty() {
                        return s;
                    }

                    match now_cst.rule {
                        Rule::pattern => current + &s,
                        Rule::let_rec_matcharm => current + &newline + "| " + s.trim(),
                        Rule::type_expr => {
                            type_expr = true;
                            current + ": " + &s
                        }
                        Rule::arg => {
                            if type_expr {
                                // 一度だけマッチ
                                type_expr = false;
                                current + " | " + &s
                            } else {
                                current + " " + &s
                            }
                        }
                        Rule::expr => self.format_assignment_rhs(
                            text,
                            now_cst,
                            &s,
                            current,
                            depth.saturating_sub(1),
                            UnicodeWidthStr::width(RESERVED_WORD.let_rec) + 1,
                        ),
                        _ => current + &s,
                    }
                })
            }
            Rule::let_rec_matcharm => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    return s;
                }
                match now_cst.rule {
                    Rule::arg => current + " " + &s,
                    Rule::expr => self.format_assignment_rhs(
                        text,
                        now_cst,
                        &s,
                        current,
                        depth.saturating_sub(1),
                        UnicodeWidthStr::width("| "),
                    ),
                    _ => unreachable!(),
                }
            }),
            Rule::match_arm => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    return s;
                }
                // ptn:pat_as() _ guard:match_guard()? _ "->" _ expr:(!match_expr() e:expr() {e})
                match now_cst.rule {
                    Rule::pat_as => current + " " + &s,
                    Rule::match_guard => current + " " + &s,
                    Rule::expr => self.format_match_arm_rhs(
                        text,
                        now_cst,
                        current,
                        depth,
                        UnicodeWidthStr::width("| "),
                    ),
                    _ => current + &s,
                }
            }),
            Rule::match_expr => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    if now_cst.rule == Rule::expr {
                        return format!("{} {s} {}", RESERVED_WORD.match_stmt, RESERVED_WORD.with);
                    }
                    return s;
                }
                match now_cst.rule {
                    Rule::expr => {
                        current
                            + " "
                            + &format!("{} {s} {}", RESERVED_WORD.match_stmt, RESERVED_WORD.with)
                    }
                    Rule::match_arm => current + &newline + "| " + &s,
                    _ => current + &s,
                }
            }),
            Rule::ctrl_if => {
                // let mut break_line_flag = false;

                let output = csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);

                    match now_cst.rule {
                        Rule::expr => current + "if " + &s,
                        Rule::ctrl_then => current + &newline + &s,
                        Rule::ctrl_else => current + &newline + &s,
                        Rule::comments => current + &newline + &s,
                        _ => unreachable!(),
                    }
                });

                output
            }
            Rule::ctrl_then | Rule::ctrl_else => {
                let output = csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);

                    if current.is_empty() {
                        newline.clone() + &s
                    } else if s.is_empty() {
                        current
                    } else if current.ends_with(&newline) {
                        current + &s
                    } else {
                        current + sep + &s
                    }
                });
                output
                    .split('\n')
                    .filter(|line| !line.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Rule::ctrl_while => {
                let mut cnt = 0;
                let output = csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    match now_cst.rule {
                        Rule::expr => {
                            cnt += 1;
                            match cnt {
                                1 => current + &s + " " + RESERVED_WORD.do_stmt + " ",
                                _ => current + &s,
                            }
                        }
                        Rule::comments => current + &s,
                        _ => current + &s,
                    }
                });

                format!("{} {output}", RESERVED_WORD.while_stmt, output = output)
            }
            Rule::unary => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                let s = if now_cst.rule == Rule::bin_operator {
                    // ( || ) のようなパターンが存在するので、スペースを開ける
                    format!("( {s} )")
                } else if now_cst.rule == Rule::expr {
                    format!("({s})")
                } else {
                    s
                };
                if current.is_empty() {
                    return s;
                }
                match &*current {
                    "!" | "&" | "~" => {
                        return current + &s;
                    }
                    _ => {}
                }
                match now_cst.rule {
                    Rule::bin_operator | Rule::expr => current + &s,
                    _ => current + sep + &s,
                }
            }),
            Rule::lambda => csts
                .iter()
                .fold(RESERVED_WORD.fun.to_string(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    if current.is_empty() {
                        return s;
                    }
                    match now_cst.rule {
                        Rule::pattern => current + " " + &s,
                        Rule::comments => current + &s,
                        _ => self
                            .join_expr_with_wrap(text, now_cst, current, depth, " -> ", " ->", 0),
                    }
                }),
            Rule::record_unit => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    return s;
                }
                match now_cst.rule {
                    Rule::var_ptn => current + " " + &s,
                    Rule::expr => {
                        self.join_expr_with_wrap(text, now_cst, current, depth, " = ", " =", 0)
                    }
                    Rule::comments => current + &s,
                    _ => unreachable!(),
                }
            }),
            Rule::type_record_unit => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    return s;
                }
                match now_cst.rule {
                    Rule::var => current + " " + &s,
                    Rule::type_expr => current + ": " + &s,
                    Rule::comments => current + &s,
                    _ => unreachable!(),
                }
            }),
            Rule::type_application => {
                let mut output = String::new();
                for cst in &csts {
                    let s = self.to_string_cst(text, cst, depth);
                    if cst.rule == Rule::comments {
                        if !output.ends_with(char::is_whitespace) {
                            output += &newline;
                        }
                        output += &s;
                        continue;
                    }
                    if !output.is_empty() {
                        output += sep;
                    }
                    if cst.rule == Rule::type_expr {
                        output += &format!("({s})");
                    } else {
                        output += &s;
                    }
                }
                output
            }
            Rule::assignment => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                if current.is_empty() {
                    return s;
                }
                match now_cst.rule {
                    Rule::var => current + " " + &s,
                    Rule::dyadic_expr
                    | Rule::unary_operator_expr
                    | Rule::application
                    | Rule::unary
                    | Rule::variant_constructor => {
                        self.join_expr_with_wrap(text, now_cst, current, depth, " <- ", " <-", 0)
                    }
                    Rule::comments => current + &s,
                    _ => unreachable!(),
                }
            }),
            Rule::application => {
                if csts.is_empty() {
                    return "".to_string();
                }
                let (flat_output, force_multiline, insert_space) =
                    self.application_flat_output(text, &csts, depth);
                let arg_count = csts.len().saturating_sub(1);
                let aggressive_wrap =
                    arg_count >= 3 && self.max_line_width(&flat_output) > self.line_width() / 2;
                if !insert_space
                    || (!force_multiline
                        && !aggressive_wrap
                        && self.line_fits(depth, 0, &flat_output))
                {
                    return flat_output;
                }

                let mut output = self.to_string_cst(text, &csts[0], depth);
                for cst in csts.iter().skip(1) {
                    let rendered = self.to_string_cst(text, cst, depth + 1);
                    output = self.append_indented_rendered(output, &rendered, depth);
                }
                output
            }
            Rule::bind_stmt => {
                // let* ~ in のとき用
                let output =
                    csts.iter()
                        .enumerate()
                        .fold(String::new(), |current, (index, now_cst)| {
                            let s = self.to_string_cst(text, now_cst, depth);
                            match now_cst.rule {
                                Rule::let_stmt
                                | Rule::let_rec_stmt
                                | Rule::let_math_stmt
                                | Rule::let_mutable_stmt
                                | Rule::open_stmt => {
                                    if index == 0 {
                                        current + &s + " " + RESERVED_WORD.in_stmt + &newline
                                    } else {
                                        current
                                            + &newline
                                            + &s
                                            + " "
                                            + RESERVED_WORD.in_stmt
                                            + &newline
                                    }
                                }
                                Rule::expr => {
                                    if s.starts_with("let") {
                                        current + s.trim_start()
                                    } else if s.contains('\n') {
                                        let s = self.to_string_cst(text, now_cst, depth + 1);
                                        // 1つ深くする
                                        current
                                            + &indent_space(self.option.tab_size as usize, 1)
                                            + s.trim_start()
                                    } else {
                                        current + s.trim_start()
                                    }
                                }
                                Rule::comments => {
                                    if current.ends_with(RESERVED_WORD.in_stmt) {
                                        current + &newline + &s
                                    } else {
                                        current + &s
                                    }
                                }
                                _ => current + &s,
                            }
                        });
                output
            }
            Rule::type_expr => {
                let mut iter = csts.into_iter().peekable();
                let mut now_cst = iter.next().unwrap();
                let mut output = self.to_string_cst(text, &now_cst, depth);
                while iter.peek().is_some() {
                    // 次の要素が存在すれば結合
                    if now_cst.rule == Rule::type_optional {
                        output += " ?-> ";
                    } else {
                        output += " -> ";
                    }
                    now_cst = iter.next().unwrap();

                    let s = self.to_string_cst(text, &now_cst, depth);
                    match now_cst.rule {
                        Rule::type_prod | Rule::type_optional => {
                            output += &s;
                        }
                        Rule::comments => {
                            output += &s;
                        }
                        _ => unreachable!(),
                    }
                }
                output
            }
            Rule::module_stmt => {
                let mut iter = csts.into_iter().peekable();
                let first = iter.next().unwrap();
                let mut output = self.to_string_cst(text, &first, depth);
                while iter.peek().is_some() {
                    let now_cst = &iter.next().unwrap();
                    let s = self.to_string_cst(text, now_cst, depth);
                    match now_cst.rule {
                        Rule::module_stmt => {}
                        Rule::sig_stmt => {
                            output += ": ";
                        }
                        Rule::struct_stmt => {
                            output += " = ";
                        }
                        Rule::comments => {}
                        _ => unreachable!(),
                    }
                    output += &s;
                }
                output
            }
            Rule::struct_stmt => {
                let check = &format!("\n{newline}");
                let output =
                    csts.iter()
                        .enumerate()
                        .fold(String::new(), |current, (index, now_cst)| {
                            let s = self.to_string_cst(text, now_cst, depth);

                            // 改行の制御
                            let current = if current.is_empty() || current.ends_with(check) {
                                current
                            } else if index > 0 && csts[index - 1].rule == Rule::comments {
                                current
                            } else if index > 0 && csts[index - 1].rule != now_cst.rule {
                                // ルールの切り替わり位置
                                current + "\n" + &newline
                            } else if !s.contains('\n') {
                                current + &newline
                            } else if csts[index - 1].rule == Rule::let_stmt
                                || csts[index - 1].rule == Rule::let_rec_stmt
                            {
                                current + "\n" + &newline
                            } else {
                                match now_cst.rule {
                                    Rule::let_stmt | Rule::let_rec_stmt => {
                                        current + "\n" + &newline
                                    }
                                    Rule::comments => current,
                                    _ => {
                                        // 基本的に改行する
                                        current + &newline
                                    }
                                }
                            };
                            match now_cst.rule {
                                Rule::let_stmt | Rule::let_rec_stmt => current + &s,
                                Rule::comments => current + &s,
                                Rule::bind_stmt => current + &s,
                                _ => current + &s,
                            }
                        });
                output
            }
            Rule::sig_stmt => {
                let output = csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    match now_cst.rule {
                        Rule::sig_val_stmt | Rule::sig_type_stmt => current + &newline + &s,
                        Rule::module_name => current + " " + &s,
                        Rule::struct_stmt => current + "= " + RESERVED_WORD.struct_stmt + &s,
                        Rule::comments => current + &s,
                        _ => current + &s,
                    }
                });
                output.trim().to_string()
            }
            Rule::horizontal_single => {
                let output = csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    if current.is_empty() {
                        s
                    } else if now_cst.rule == Rule::regular_text && s.trim().is_empty() {
                        // 空行・スペースの処理
                        if current.ends_with(char::is_whitespace) {
                            // 既に空白がある場合には何もしない
                            current
                        } else {
                            current + &s
                        }
                    } else if now_cst.rule == Rule::regular_text {
                        if current.ends_with(char::is_whitespace) {
                            // 既に空白がある場合には何もしない
                            current + s.trim_start()
                        } else {
                            current + &s
                        }
                    } else if now_cst.rule == Rule::comments {
                        format!("{}{newline}{s}", current.trim_end())
                    } else {
                        current + &s
                    }
                });

                // コメントが末尾にあるとき余計な改行が残ってしまうので削除
                output.trim().to_string()
            }
            Rule::preamble => csts.iter().fold(String::new(), |current, now_cst| {
                // 例外処理
                let s = self.to_string_cst(text, now_cst, depth).trim().to_string();
                if current.is_empty() {
                    s
                } else if s.is_empty() {
                    current
                } else {
                    match now_cst.rule {
                        Rule::module_stmt => current + "\n\n" + &s,
                        _ => current + "\n" + &s,
                    }
                }
            }),
            Rule::horizontal_list => csts.iter().fold("|".to_string(), |current, now_cst| {
                // 実装しているが使わない
                let s = self.to_string_cst(text, now_cst, depth);
                let flag = now_cst.rule == Rule::comments;
                if flag {
                    current + &s
                } else if s.is_empty() {
                    current
                } else if now_cst.rule == Rule::comments {
                    current + &s
                } else {
                    current + " " + &s + sep
                }
            }),
            Rule::list => csts
                .iter()
                .fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    let flag = now_cst.rule == Rule::comments;
                    if flag {
                        current + &s
                    } else if current.is_empty() {
                        s + sep
                    } else if s.is_empty() {
                        current
                    } else if now_cst.rule == Rule::comments {
                        current + &s
                    } else {
                        current + &s + sep
                    }
                })
                .trim_end()
                .to_string(),
            Rule::block_cmd | Rule::inline_cmd => {
                csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    if current.is_empty() {
                        s
                    } else if s.is_empty() {
                        current
                    } else if current.ends_with(&newline) {
                        current + &s
                    } else {
                        current + sep + &s
                    }
                })
            }
            Rule::math_single => {
                let mut last_token = String::new();

                let output = csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);

                    let output = if current.is_empty() {
                        s.clone()
                    } else if s.is_empty() {
                        current
                    } else if current.ends_with(&newline) {
                        current + &s
                    } else if last_token.starts_with('\\') {
                        current + sep + &s
                    } else if now_cst.rule == Rule::comments {
                        format!("{}{newline}{s}", current.trim_end())
                    } else if s.starts_with(char::is_alphabetic)
                        && current.ends_with(char::is_alphabetic)
                    {
                        current + &s
                    } else {
                        current + sep + &s
                    };
                    last_token = s;
                    output
                });
                output
            }
            Rule::math_token => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);

                if current.is_empty() {
                    s
                } else if s.is_empty() {
                    current
                } else if current.ends_with(&newline) {
                    current + &s
                } else if matches!(now_cst.rule, Rule::math_sup | Rule::math_sub) {
                    current + &s
                } else {
                    current + sep + &s
                }
            }),
            Rule::vertical => {
                let mut line_index = cst.span.end; // 範囲外のusizeで初期化
                csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    let output = if current.is_empty() {
                        s
                    } else if s.is_empty() {
                        current
                    } else if current.ends_with(&newline) {
                        current + &s
                    } else if s.starts_with("%\n") {
                        current + &s
                    } else {
                        // 複数行の改行を省略して1行にする
                        let start = now_cst.span.start;
                        let mut cnt = 0;
                        for &value in self.lines.iter() {
                            if line_index < value && value < start {
                                cnt += 1;
                            }
                        }
                        let current = if cnt > 1 { current + "\n" } else { current };
                        current + sep + &s
                    };
                    line_index = now_cst.span.end;

                    output.trim_end().to_string()
                })
            }
            Rule::dyadic_expr => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);
                let output = if current.is_empty() {
                    s
                } else if s.is_empty() {
                    current
                } else if current.ends_with(&newline) {
                    current + &s
                } else if now_cst.rule == Rule::bin_operator && s.trim() == "|>" {
                    current + &s
                } else {
                    current + sep + &s
                };
                output
            }),
            Rule::program_saty => {
                csts.iter().fold(String::new(), |current, now_cst| {
                    let s = self.to_string_cst(text, now_cst, depth);
                    let output = if current.is_empty() {
                        s
                    } else if s.is_empty() {
                        current
                    } else if current.ends_with(&newline) {
                        current + &s
                    } else if s.starts_with("%\n") {
                        current + &s
                    } else {
                        current + sep + &s
                    };
                    if now_cst.rule == Rule::preamble {
                        // program saty だった場合、in を入れる
                        output + "\n" + RESERVED_WORD.in_stmt + "\n\n"
                    } else {
                        output
                    }
                })
            }
            _ => csts.iter().fold(String::new(), |current, now_cst| {
                let s = self.to_string_cst(text, now_cst, depth);

                if current.is_empty() {
                    s
                } else if s.is_empty() {
                    current
                } else if current.ends_with(&newline) {
                    current + &s
                } else {
                    current + sep + &s
                }
            }),
        };

        output
    }

    /// cst を文字列にするための関数
    fn to_string_cst(&self, text: &str, cst: &Cst, depth: usize) -> String {
        // インデントを制御するための変数
        let new_depth = match cst.rule {
            Rule::block_text | Rule::cmd_text_arg | Rule::record | Rule::type_record => depth + 1,
            // Rule::horizontal_list | Rule::list => depth + 1,
            Rule::list => depth + 1,
            Rule::type_block_cmd | Rule::type_inline_cmd | Rule::math_cmd => depth + 1,
            Rule::match_expr | Rule::let_rec_matcharm => depth + 1,
            Rule::let_rec_inner => depth + 1,
            Rule::sig_stmt | Rule::struct_stmt => depth + 1,
            Rule::ctrl_then | Rule::ctrl_else => depth + 1,
            _ => depth,
        };
        let start_indent =
            "\n".to_string() + &indent_space(self.option.tab_size as usize, new_depth);
        let end_indent = "\n".to_string() + &indent_space(self.option.tab_size as usize, depth);

        let output = self.to_string_cst_inner(text, cst, new_depth);
        let self_text = text.get(cst.span.start..cst.span.end).unwrap().to_string();

        use satysfi_parser::Rule;
        // 中身をそのまま返すものは output をそのまま返す
        // self_text は元の文字列をそのまま返したいときに使用
        match cst.rule {
            Rule::comments => to_comment_string(self_text) + &end_indent,
            // header
            // stage の次は必ず改行する
            Rule::stage => "@stage: ".to_string() + &self_text + "\n\n",
            // headers があれば必ず改行する
            Rule::headers => {
                if !output.is_empty() {
                    output + "\n"
                } else {
                    output
                }
            }
            Rule::header_require => "@require: ".to_string() + &output + "\n",
            Rule::header_import => "@import: ".to_string() + &output + "\n",
            // 末尾のスペースなどは削除 (スペースで終わるpkgnameが導入されるとバグるけれど無いでしょう……)
            Rule::pkgname => self_text.trim().to_string(),

            // statement
            Rule::let_stmt => format!("{} {output}", RESERVED_WORD.let_stmt),
            Rule::let_rec_stmt => format!("{} {output}", RESERVED_WORD.let_rec),
            Rule::let_rec_inner => output,
            Rule::let_rec_matcharm => output,
            Rule::let_inline_stmt_ctx => {
                format!("{} {output}", RESERVED_WORD.let_inline)
            }
            Rule::let_inline_stmt_noctx => {
                format!("{} {output}", RESERVED_WORD.let_inline)
            }
            Rule::let_block_stmt_ctx => format!("{} {output}", RESERVED_WORD.let_block),
            Rule::let_block_stmt_noctx => {
                format!("{} {output}", RESERVED_WORD.let_block)
            }
            Rule::let_math_stmt => format!("{} {}", RESERVED_WORD.let_math, output),
            Rule::let_mutable_stmt => format!("{} {}", RESERVED_WORD.let_mutable, output),
            Rule::type_stmt => format!("{} {}", RESERVED_WORD.type_stmt, output),
            Rule::type_inner => output,
            Rule::type_variant => output,
            Rule::module_stmt => format!("{start_indent}{} {}", RESERVED_WORD.module, output),
            Rule::open_stmt => format!("{} {output}", RESERVED_WORD.open),
            Rule::arg => self_text,

            // struct
            Rule::sig_stmt => format!(
                "{}{start_indent}{output}{end_indent}{}",
                RESERVED_WORD.sig, RESERVED_WORD.end
            ), // TODO
            Rule::struct_stmt => format!(
                "{}{start_indent}{output}{end_indent}{}",
                RESERVED_WORD.struct_stmt, RESERVED_WORD.end
            ), // TODO
            Rule::sig_type_stmt => format!("{} {output}", RESERVED_WORD.type_stmt),
            Rule::sig_val_stmt => format!("{} {output}", RESERVED_WORD.val),
            Rule::sig_direct_stmt => format!("{start_indent}{} {output}", RESERVED_WORD.direct),

            // types
            Rule::type_expr => output,
            Rule::type_optional => output,
            Rule::type_prod => output,
            Rule::type_inline_cmd => {
                if output.contains('\n') {
                    format!(
                        "[{start_indent}{output};{end_indent}] {}",
                        RESERVED_WORD.inline_command
                    )
                } else {
                    format!("[{output}] {}", RESERVED_WORD.inline_command)
                }
            }
            Rule::type_block_cmd => {
                if output.contains('\n') {
                    format!(
                        "[{start_indent}{output};{end_indent}] {}",
                        RESERVED_WORD.block_command
                    )
                } else {
                    format!("[{output}] {}", RESERVED_WORD.block_command)
                }
            }
            Rule::type_math_cmd => {
                if output.contains('\n') {
                    format!(
                        "[{start_indent}{output};{end_indent}] {}",
                        RESERVED_WORD.math_command
                    )
                } else {
                    format!("[{output}] {}", RESERVED_WORD.math_command)
                }
            }
            Rule::type_list_unit_optional => output + "?",
            Rule::type_application => output,
            Rule::type_name => self_text,
            // Rule::type_record => output,
            Rule::type_record_unit => output,
            Rule::type_param => format!("'{output}"),
            Rule::constraint => format!("{} {output}", RESERVED_WORD.constraint),

            // unary
            Rule::unary => output,
            Rule::unary_prefix => self_text,
            Rule::block_text => {
                if !output.is_empty() {
                    format!("'<{start_indent}{output}{end_indent}>")
                } else if output.starts_with("%\n") {
                    format!("'<{output}{end_indent}>")
                } else {
                    format!("'<{output}>")
                }
            }
            // Rule::horizontal_text => output,
            // Rule::math_text => self_text,
            Rule::math_text => format!("${{{output}}}"),
            Rule::list => {
                let trimed_self_text: String = self_text.split(char::is_whitespace).collect();
                if output.is_empty() {
                    "[]".to_string()
                } else if trimed_self_text.len() < 15 {
                    // list の文字の長さが十分に短い easy tableの [l;c;r;] など
                    let inner = output
                        .split('\n')
                        .into_iter()
                        .map(|line| line.trim().to_string())
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<String>>()
                        .join("");
                    format!("[{inner}]")
                } else {
                    format!("[{start_indent}{output}{end_indent}]")
                }
            }
            Rule::record | Rule::type_record => {
                if cst.inner.len() > 1 {
                    // 2 つ以上のときは改行
                    format!("(|{start_indent}{output}{end_indent}|)")
                } else {
                    // 1つだけの時は、改行しない
                    format!("(|{output}|)")
                }
            }
            Rule::record_unit => output,
            Rule::tuple => format!("({output})"),
            Rule::bin_operator => {
                if self_text == "|>" {
                    // 1つ深くする
                    format!(
                        "{start_indent}{}{self_text}",
                        indent_space(self.option.tab_size as usize, 1)
                    )
                } else {
                    self_text
                }
            }
            Rule::expr_with_mod => self_text,
            Rule::var => self_text,
            Rule::var_ptn => self_text,
            Rule::modvar => self_text,
            Rule::mod_cmd_name => self_text,
            Rule::module_name => self_text,
            Rule::variant_name => self_text,

            // command
            Rule::cmd_name_ptn => self_text,
            Rule::cmd_expr_arg => {
                if self_text.starts_with('(') {
                    format!("({output})",)
                } else {
                    output
                }
            }
            Rule::cmd_expr_option => {
                if self_text == ("?*") {
                    "?*".to_string()
                } else {
                    format!("?:{output}")
                }
            }
            Rule::cmd_text_arg | Rule::horizontal_text => {
                let output = output.trim();
                // 括弧の種類を取得
                let start_arg = self_text.chars().next().unwrap();
                let end_arg = self_text.chars().last().unwrap();
                // コメントで開始 or 改行を含んでいたら、改行を入れる
                let include_comment = output.starts_with('%');
                let include_kaigyou =
                    output.find('\n').is_some() || start_arg == '<' || include_comment;
                if output.starts_with("%\n") {
                    if include_kaigyou {
                        format!("{start_arg}{output}{end_indent}{end_arg}")
                    } else {
                        format!("{start_arg} {output} {end_arg}")
                    }
                } else {
                    match output.trim().len() {
                        0 => format!("{start_arg}{end_arg}"),
                        // easytable
                        _ if output.starts_with(char::is_whitespace) => {
                            format!("{start_arg}\n{output}{end_arg}")
                        }
                        _num if include_kaigyou => {
                            format!("{start_arg}{start_indent}{output}{end_indent}{end_arg}")
                        }
                        _ => format!("{start_arg} {output} {end_arg}"),
                    }
                }
            }
            Rule::inline_cmd => {
                if self_text.ends_with(';') {
                    output + ";"
                } else {
                    output
                }
            }
            Rule::inline_cmd_name => self_text,
            Rule::block_cmd => {
                if self_text.ends_with(';') {
                    output + ";"
                } else {
                    output
                }
            }

            Rule::block_cmd_name => self_text,
            Rule::math_cmd => self_text.trim().to_string(),
            Rule::math_cmd_name => self_text,
            Rule::math_cmd_expr_arg => output,
            Rule::math_cmd_expr_option => format!(":?{output}"),

            // pattern
            Rule::pat_as => output,
            Rule::pat_cons => output,
            Rule::pattern => self_text, // TODO どのパターンでも中身をそのまま出力
            Rule::pat_variant => output,
            Rule::pat_list => format!("[{output}]"),
            Rule::pat_tuple => output, // TODO

            // expr
            Rule::expr => {
                if self_text.ends_with(';') {
                    output + ";"
                } else {
                    output
                }
            }
            Rule::match_expr => output, // TODO
            Rule::match_arm => output,  // TODO
            Rule::match_guard => format!("{} {output}", RESERVED_WORD.when), // TODO
            Rule::bind_stmt => output,  // TODO
            Rule::ctrl_while => output, // TODO
            Rule::ctrl_if => output,    // TODO
            Rule::ctrl_then => {
                format!("{}\n{output}", RESERVED_WORD.then)
            }
            Rule::ctrl_else => {
                format!("{}\n{output}", RESERVED_WORD.else_stmt)
            }
            Rule::lambda => output,              // TODO
            Rule::assignment => output,          // TODO
            Rule::dyadic_expr => output,         // TODO
            Rule::unary_operator_expr => output, // TODO
            Rule::unary_operator => self_text,
            // application
            Rule::application => output,
            Rule::application_args_normal => output,
            Rule::application_args_optional => {
                if self_text == ("?*") {
                    "?*".to_string()
                } else {
                    format!("?:{output}")
                }
            }
            Rule::command_application => format!("{} {output}", RESERVED_WORD.command),
            Rule::variant_constructor => output,

            // horizontal
            Rule::horizontal_single => output,
            Rule::horizontal_list => {
                let sep = format!(
                    "\n{}",
                    indent_space(self.option.tab_size as usize, new_depth)
                );
                let output = self_text
                    .split('\n')
                    .into_iter()
                    .map(|line| {
                        line.split(char::is_whitespace)
                            .filter(|line| !line.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<String>>()
                    .join(&sep);
                // output
                format!(
                    "{}{output}",
                    indent_space(self.option.tab_size as usize, new_depth)
                )
            }
            Rule::horizontal_bullet_list => output, // TODO
            Rule::horizontal_bullet => output,      // TODO
            Rule::horizontal_bullet_star => {
                " ".repeat(self.option.tab_size as usize / 2)
                    .repeat(self_text.len() - 1)
                    + &self_text
            }
            Rule::regular_text => {
                let sep = format!("\n{}", indent_space(self.option.tab_size as usize, depth));
                let output = self_text
                    .split('\n')
                    .into_iter()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<String>>()
                    .join(&sep);
                let start_space = if self_text.starts_with(char::is_whitespace) {
                    " "
                } else {
                    ""
                };
                let end_space = if self_text.ends_with(char::is_whitespace) {
                    " "
                } else {
                    ""
                };
                if output.trim().is_empty() {
                    if self_text.starts_with('\n') {
                        start_indent
                    } else {
                        start_space.to_string()
                    }
                } else {
                    let end_newline =
                        self_text.trim_end_matches(&['\t', ' ']) != self_text.trim_end();
                    match (self_text.starts_with('\n'), end_newline) {
                        (true, true) => format!("{start_indent}{output}{end_indent}"),
                        (true, false) => format!("{start_indent}{output}{end_space}"),
                        (false, true) => format!("{start_space}{output}{end_indent}"),
                        (false, false) => format!("{start_space}{output}{end_space}"),
                    }
                }
            }
            Rule::horizontal_escaped_char => self_text,
            Rule::inline_text_embedding => format!("#{output};"),

            // vertical
            Rule::vertical => output, // インデント制御のため、<> はverticalの親で処理
            Rule::block_text_embedding => format!("#{output};"),

            // constants
            Rule::const_unit => self_text,
            Rule::const_bool => self_text,
            Rule::const_int => self_text,
            Rule::const_float => self_text,
            Rule::const_length => self_text,
            Rule::const_string => self_text,

            // math
            Rule::math_single => output, // TODO
            Rule::math_list => output,   // TODO
            Rule::math_token => output,  // TODO
            Rule::math_sup => {
                if self_text.starts_with('{') {
                    format!("^{{{output}}}")
                } else {
                    format!("^{output}")
                }
            }
            Rule::math_sub => {
                if self_text.starts_with('{') {
                    format!("_{{{output}}}")
                } else {
                    format!("_{output}")
                }
            }
            Rule::math_unary => {
                if output.is_empty() {
                    self_text
                } else {
                    output
                }
            }
            Rule::math_embedding => format!("#{output}"), // TODO

            // TODO other things
            Rule::misc => " ".to_string(),
            Rule::program_saty => output.trim_start().to_string(),
            Rule::program_satyh => output.trim_start().to_string(),
            Rule::preamble => output.trim_start().to_string(),
            // TODO
            // dummy
            Rule::dummy_header => panic!("found dummy header"),
            Rule::dummy_sig_stmt => panic!("found dummy sig_stmt"),
            Rule::dummy_stmt => panic!("found dummy stmt"),
            Rule::dummy_inline_cmd_incomplete => self_text, // panic!("found dummy inline_cmd"), mdja.satyh のparseで到達する
            Rule::dummy_block_cmd_incomplete => panic!("found dummy block_cmd"),
            Rule::dummy_modvar_incomplete => panic!("found dummy modvar"),
            // _ => unreachable!(),
        }
    }
}
#[inline]
fn indent_space(unit: usize, depth: usize) -> String {
    " ".repeat(unit * depth)
}
