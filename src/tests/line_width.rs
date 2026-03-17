use super::{assert_max_line_width, default_option, option_with_line_width};
use crate::format;

#[test]
fn wraps_long_nested_application_at_default_width() {
    let text = r#"let group drawables = Drawable(fun origin -> List.fold-left (fun acc drawable -> List.append acc (render drawable origin)) [] drawables)
in

group"#;
    let expect = r#"let group drawables =
    Drawable(fun origin ->
        List.fold-left
            (fun acc drawable -> List.append acc (render drawable origin))
            []
            drawables)
in

group
"#;

    let output = format(text, default_option());
    assert_eq!(output, expect);
    assert_max_line_width(&output, 120);
}

#[test]
fn wraps_match_branch_application_at_default_width() {
    let text = r#"let render-shape shape =
  match shape with
  | Rectangle((centerx, centery), width, height) -> Anchor.fold (centerx, centery) (centerx, centery +' height *' 0.5) (centerx, centery -' height *' 0.5) (centerx +' width *' 0.5, centery) (centerx -' width *' 0.5, centery) which
in

render-shape"#;
    let expect = r#"let render-shape shape =
    match shape with
        | Rectangle((centerx, centery), width, height) ->
            Anchor.fold
                (centerx, centery)
                (centerx, centery +' height *' 0.5)
                (centerx, centery -' height *' 0.5)
                (centerx +' width *' 0.5, centery)
                (centerx -' width *' 0.5, centery)
                which
in

render-shape
"#;

    let output = format(text, default_option());
    assert_eq!(output, expect);
    assert_max_line_width(&output, 120);
}

#[test]
fn wider_line_width_keeps_long_application_flat() {
    let text = r#"let group drawables = Drawable(fun origin -> List.fold-left (fun acc drawable -> List.append acc (render drawable origin)) [] drawables)
in

group"#;
    let expect = r#"let group drawables = Drawable(fun origin -> List.fold-left (fun acc drawable -> List.append acc (render drawable origin)) [] drawables)
in

group
"#;

    let output = format(text, option_with_line_width(200));
    assert_eq!(output, expect);
    assert_max_line_width(&output, 200);
}

#[test]
fn smaller_line_width_wraps_earlier_than_default() {
    let text = r#"let render-all origin drawables = List.map (fun drawable -> render drawable origin) drawables
in

render-all"#;
    let expect = r#"let render-all origin drawables =
    List.map
        (fun drawable -> render drawable origin)
        drawables
in

render-all
"#;

    let output = format(text, option_with_line_width(60));
    assert_eq!(output, expect);
    assert_max_line_width(&output, 60);
}

#[test]
fn short_application_stays_flat_at_default_width() {
    let text = r#"let render-all origin drawables = List.map (fun drawable -> render drawable origin) drawables
in

render-all"#;
    let expect = r#"let render-all origin drawables = List.map (fun drawable -> render drawable origin) drawables
in

render-all
"#;

    let output = format(text, default_option());
    assert_eq!(output, expect);
    assert_max_line_width(&output, 120);
}
