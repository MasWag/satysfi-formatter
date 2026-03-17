use super::test_tmpl;

fn test_match_tmpl(input: &str, expect: &str) {
    test_tmpl(input, expect);
    test_tmpl(expect, expect);
}

#[test]
fn structured_bind_rhs_breaks_after_arrow() {
    let text = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x = match x with | A -> let y = x in y
end"#;

    let expect = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x =
        match x with
            | A ->
                let y = x in
                y
end
"#;

    test_match_tmpl(text, expect);
}

#[test]
fn structured_bind_rhs_preserves_guard_header() {
    let text = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x = match x with | A when x < 5 -> let y = x in y
end"#;

    let expect = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x =
        match x with
            | A when x < 5 ->
                let y = x in
                y
end
"#;

    test_match_tmpl(text, expect);
}

#[test]
fn structured_lambda_rhs_breaks_after_arrow() {
    let text = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x = match x with | A -> fun y -> y
end"#;

    let expect = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x =
        match x with
            | A ->
                fun y -> y
end
"#;

    test_match_tmpl(text, expect);
}

#[test]
fn structured_and_simple_match_arms_mix_cleanly() {
    let text = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x = match x with | A -> x | B -> let y = x in y | C -> fun y -> y
end"#;

    let expect = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x =
        match x with
            | A -> x
            | B ->
                let y = x in
                y
            | C ->
                fun y -> y
end
"#;

    test_match_tmpl(text, expect);
}

#[test]
fn structured_bind_rhs_keeps_nested_match_indented() {
    let text = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x = match x with | A -> let y = x in match y with | B -> y
end"#;

    let expect = r#"@stage: persistent

module M: sig
    val f: int -> int
end = struct
    let f x =
        match x with
            | A ->
                let y = x in
                    match y with
                        | B -> y
end
"#;

    test_match_tmpl(text, expect);
}
