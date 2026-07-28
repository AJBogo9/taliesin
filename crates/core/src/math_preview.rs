//! A Unicode preview of a math expression, for editor hovers.

/// Render `latex` to a one-line Unicode approximation. `None` when KaTeX cannot parse it.
pub fn unicode_preview(latex: &str, display: bool) -> Option<String> {
    let html = crate::math::render(latex, display);
    // No explicit error check: KaTeX (with `throw_on_error = false`) replaces the WHOLE
    // output with a bare `katex-error` span carrying no `<math>` element, and `math.rs`'s
    // engine-level `tali-math-error` fallback does the same — so "has MathML" already means
    // "parsed". `error_output_carries_no_mathml` is the canary on that assumption; if it
    // ever fires, this needs a real guard before it previews an error as if it rendered.
    let mathml = slice_between(&html, "<math", "</math>")?;
    Some(render_nodes(&parse(mathml)))
}

/// The content between the first `open` tag and its matching `close`, exclusive of both.
fn slice_between<'a>(hay: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = hay.find(open)?;
    let body = hay[start..].find('>')? + start + 1;
    let end = hay.find(close)?;
    hay.get(body..end)
}

enum Node {
    Text(String),
    Elem(String, Vec<Node>),
}

/// A tolerant MathML tree parse. KaTeX's output is machine-generated and well-formed, so
/// this only needs to be correct on that shape; anything unexpected degrades to text rather
/// than failing, since a rough preview beats none.
fn parse(mathml: &str) -> Vec<Node> {
    let mut stack: Vec<(String, Vec<Node>)> = vec![(String::new(), Vec::new())];
    let mut rest = mathml;
    loop {
        let Some(lt) = rest.find('<') else {
            push_text(&mut stack, rest);
            break;
        };
        push_text(&mut stack, &rest[..lt]);
        let after = &rest[lt..];
        let Some(gt) = after.find('>') else { break };
        let inner = &after[1..gt];
        rest = &after[gt + 1..];
        if inner.starts_with('/') {
            // Close the innermost open element. A stray close (never seen from KaTeX) is
            // dropped rather than unwinding past the root.
            if stack.len() > 1 {
                let (tag, children) = stack.pop().expect("checked len > 1");
                stack
                    .last_mut()
                    .expect("root always present")
                    .1
                    .push(Node::Elem(tag, children));
            }
        } else if inner.ends_with('/') {
            let tag = tag_name(inner);
            stack
                .last_mut()
                .expect("root always present")
                .1
                .push(Node::Elem(tag, Vec::new()));
        } else {
            stack.push((tag_name(inner), Vec::new()));
        }
    }
    // An unclosed element still contributes its children.
    while stack.len() > 1 {
        let (tag, children) = stack.pop().expect("checked len > 1");
        stack
            .last_mut()
            .expect("root always present")
            .1
            .push(Node::Elem(tag, children));
    }
    stack.pop().expect("root always present").1
}

fn push_text(stack: &mut [(String, Vec<Node>)], raw: &str) {
    if !raw.is_empty() {
        stack
            .last_mut()
            .expect("root always present")
            .1
            .push(Node::Text(decode_entities(raw)));
    }
}

fn tag_name(inner: &str) -> String {
    inner
        .split([' ', '\t', '\n', '/'])
        .next()
        .unwrap_or("")
        .to_string()
}

fn render_nodes(nodes: &[Node]) -> String {
    nodes.iter().map(render_node).collect()
}

/// The element children, in order, ignoring whitespace-only text between tags — so
/// positional lookups (`msup`'s base and exponent) are not thrown off by formatting.
fn elem_children(nodes: &[Node]) -> Vec<&Node> {
    nodes
        .iter()
        .filter(|n| !matches!(n, Node::Text(t) if t.trim().is_empty()))
        .collect()
}

fn render_node(node: &Node) -> String {
    let (tag, children) = match node {
        Node::Text(t) => return t.clone(),
        Node::Elem(tag, children) => (tag.as_str(), children),
    };
    let kids = elem_children(children);
    let part = |i: usize| kids.get(i).map(|n| render_node(n)).unwrap_or_default();
    match tag {
        // The original LaTeX, carried for accessibility. Emitting it would print the source
        // the author is already looking at, twice.
        "annotation" => String::new(),
        "mspace" => " ".to_string(),
        // `munder`/`mover`/`munderover` are the display-mode spellings of the same thing:
        // in display mode `\sum_{i=1}^n` puts its limits under and over rather than beside.
        "msup" | "mover" => format!("{}{}", part(0), script(&part(1), Script::Super)),
        "msub" | "munder" => format!("{}{}", part(0), script(&part(1), Script::Sub)),
        "msubsup" | "munderover" => format!(
            "{}{}{}",
            part(0),
            script(&part(1), Script::Sub),
            script(&part(2), Script::Super)
        ),
        "mfrac" => format!("{}/{}", group(&part(0)), group(&part(1))),
        "msqrt" => format!("√{}", group(&render_nodes(children))),
        // mi/mn/mo/mtext/mrow/semantics/math and anything unknown: their content, in order.
        _ => render_nodes(children),
    }
}

/// Parenthesize a fraction or root part that would otherwise reassociate: `\frac{a+1}{b}`
/// must not preview as `a+1/b`, which means something else.
fn group(s: &str) -> String {
    let needs = s.chars().any(|c| {
        matches!(
            c,
            '+' | '-' | '\u{2212}' | '/' | '=' | '<' | '>' | ' ' | '±'
        )
    });
    if needs {
        format!("({s})")
    } else {
        s.to_string()
    }
}

enum Script {
    Super,
    Sub,
}

/// Raise or lower `s` into Unicode script characters. Falls back to `^…` / `_…` when any
/// character has no script form (Greek exponents, mostly), which stays readable and, more
/// importantly, stays honest about the structure.
fn script(s: &str, kind: Script) -> String {
    let mapped: Option<String> = s
        .chars()
        .map(|c| match kind {
            Script::Super => super_char(c),
            Script::Sub => sub_char(c),
        })
        .collect();
    match mapped {
        Some(m) if !s.is_empty() => m,
        _ => {
            let marker = match kind {
                Script::Super => '^',
                Script::Sub => '_',
            };
            let body = if s.chars().count() > 1 {
                format!("({s})")
            } else {
                s.to_string()
            };
            format!("{marker}{body}")
        }
    }
}

fn super_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        // KaTeX emits U+2212 MINUS SIGN, not ASCII hyphen; accept both.
        '-' | '\u{2212}' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'c' => 'ᶜ',
        'd' => 'ᵈ',
        'e' => 'ᵉ',
        'f' => 'ᶠ',
        'g' => 'ᵍ',
        'h' => 'ʰ',
        'i' => 'ⁱ',
        'j' => 'ʲ',
        'k' => 'ᵏ',
        'l' => 'ˡ',
        'm' => 'ᵐ',
        'n' => 'ⁿ',
        'o' => 'ᵒ',
        'p' => 'ᵖ',
        'r' => 'ʳ',
        's' => 'ˢ',
        't' => 'ᵗ',
        'u' => 'ᵘ',
        'v' => 'ᵛ',
        'w' => 'ʷ',
        'x' => 'ˣ',
        'y' => 'ʸ',
        'z' => 'ᶻ',
        _ => return None,
    })
}

fn sub_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' | '\u{2212}' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        'r' => 'ᵣ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        _ => return None,
    })
}

fn decode_entities(s: &str) -> String {
    let decoded = s
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&amp;", "&");
    // KaTeX spells LaTeX's spacing macros with typographic spaces (`\,` is U+2009 THIN
    // SPACE) and pads with zero-width joiners. In a proportional hover font those either
    // read as nothing or as a glyph-width accident, so flatten every space to U+0020 and
    // drop the invisible ones — a preview should not contain characters you cannot see.
    decoded
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
            )
        })
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // A flat text extraction of KaTeX's MathML renders `<msup><mi>x</mi><mn>2</mn></msup>`
    // as "x2", which reads as multiplication. A preview that says that is worse than none.
    #[test]
    fn a_superscript_is_raised_not_flattened_beside_its_base() {
        assert_eq!(unicode_preview("x^2", false).as_deref(), Some("x²"));
    }

    #[test]
    fn a_greek_expression_previews_as_its_glyphs() {
        assert_eq!(
            unicode_preview("\\alpha + \\beta", false).as_deref(),
            Some("α+β")
        );
    }

    #[test]
    fn a_subscript_is_lowered() {
        assert_eq!(unicode_preview("x_1", false).as_deref(), Some("x₁"));
    }

    #[test]
    fn a_fraction_becomes_a_division_not_a_juxtaposition() {
        assert_eq!(
            unicode_preview("\\frac{a}{b}", false).as_deref(),
            Some("a/b")
        );
    }

    // `a+1/b` is a different expression from `(a+1)/b`. Flattening a compound numerator
    // without parentheses would preview one as the other.
    #[test]
    fn a_compound_fraction_part_is_parenthesized() {
        assert_eq!(
            unicode_preview("\\frac{a+1}{b}", false).as_deref(),
            Some("(a+1)/b")
        );
    }

    #[test]
    fn integral_limits_ride_the_sign() {
        assert_eq!(
            unicode_preview("\\int_0^1 x\\,dx", false).as_deref(),
            Some("∫₀¹x dx")
        );
    }

    // Display mode spells the same limits `munderover` instead of `msubsup`; both must
    // preview, or every `$$…$$` in the corpus previews worse than every `$…$`.
    #[test]
    fn display_mode_limits_preview_too() {
        assert_eq!(
            unicode_preview("\\sum_{i=1}^n i", true).as_deref(),
            Some("∑ᵢ₌₁ⁿi")
        );
    }

    // Greek has no superscript form in Unicode. Better a visible `^` than a silent drop.
    #[test]
    fn an_exponent_with_no_script_form_falls_back_to_caret() {
        assert_eq!(unicode_preview("x^\\alpha", false).as_deref(), Some("x^α"));
    }

    #[test]
    fn math_katex_cannot_parse_has_no_preview() {
        assert_eq!(unicode_preview("\\frac{", false), None);
    }

    // The load-bearing assumption behind having no explicit error branch: a failed render
    // emits no MathML, so "found `<math>`" is a sound proxy for "parsed". If KaTeX ever
    // starts emitting partial MathML beside an error marker, this fails FIRST and loudly,
    // rather than a reader silently getting a confident preview of a broken expression.
    #[test]
    fn error_output_carries_no_mathml() {
        for broken in ["\\frac{", "\\begin{matrix}", "\\sqrt", "x^"] {
            let html = crate::math::render(broken, false);
            assert!(
                !html.contains("<math"),
                "{broken:?} produced MathML alongside an error: {html}"
            );
        }
    }
}
