//! The math command vocabulary offered to editors, for `$…$` / `$$…$$` completion and the
//! symbol picker.
//!
//! **Why this can be authoritative rather than a wish list.** Every other vocabulary here is
//! sourced from the const the validator reads, so a completion cannot offer something
//! `check` rejects. Math has no such const: the grammar is KaTeX's. But KaTeX is *in the
//! binary* (`crate::math`, the `katex` crate), so the equivalent guarantee is available by
//! construction — [`tests::every_command_renders`] renders each entry's probe through the
//! same code path a document uses and fails if KaTeX cannot parse it. A command that stops
//! working, or one added here by guesswork, fails the build rather than shipping a
//! suggestion that renders as a red error span for the reader.
//!
//! `snippet` is LSP snippet syntax (`$1`, `${1:x}`), used verbatim by the completion item;
//! it is present exactly when the command takes arguments or needs a closing half.
//! `category` groups the symbol picker.

use serde_json::{Value, json};

/// One offered math command: `(name, description, category, snippet)`.
///
/// `snippet` is `""` for a bare symbol, whose insert text is the name itself.
pub(crate) struct MathCommand {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) category: &'static str,
    pub(crate) snippet: &'static str,
}

const fn sym(name: &'static str, description: &'static str, category: &'static str) -> MathCommand {
    MathCommand {
        name,
        description,
        category,
        snippet: "",
    }
}

const fn snip(
    name: &'static str,
    description: &'static str,
    category: &'static str,
    snippet: &'static str,
) -> MathCommand {
    MathCommand {
        name,
        description,
        category,
        snippet,
    }
}

/// The offered commands. Deliberately a curated working set rather than all of KaTeX: a
/// completion list is a menu, and burying `\alpha` under three hundred rarely-typed control
/// sequences costs more than the long tail gains. Grow it when authoring wants something.
pub(crate) const MATH_COMMANDS: &[MathCommand] = &[
    // ---- Greek, lowercase ----
    sym("\\alpha", "α", "Greek"),
    sym("\\beta", "β", "Greek"),
    sym("\\gamma", "γ", "Greek"),
    sym("\\delta", "δ", "Greek"),
    sym("\\epsilon", "ϵ", "Greek"),
    sym("\\varepsilon", "ε", "Greek"),
    sym("\\zeta", "ζ", "Greek"),
    sym("\\eta", "η", "Greek"),
    sym("\\theta", "θ", "Greek"),
    sym("\\vartheta", "ϑ", "Greek"),
    sym("\\iota", "ι", "Greek"),
    sym("\\kappa", "κ", "Greek"),
    sym("\\lambda", "λ", "Greek"),
    sym("\\mu", "μ", "Greek"),
    sym("\\nu", "ν", "Greek"),
    sym("\\xi", "ξ", "Greek"),
    sym("\\pi", "π", "Greek"),
    sym("\\varpi", "ϖ", "Greek"),
    sym("\\rho", "ρ", "Greek"),
    sym("\\varrho", "ϱ", "Greek"),
    sym("\\sigma", "σ", "Greek"),
    sym("\\varsigma", "ς", "Greek"),
    sym("\\tau", "τ", "Greek"),
    sym("\\upsilon", "υ", "Greek"),
    sym("\\phi", "ϕ", "Greek"),
    sym("\\varphi", "φ", "Greek"),
    sym("\\chi", "χ", "Greek"),
    sym("\\psi", "ψ", "Greek"),
    sym("\\omega", "ω", "Greek"),
    // ---- Greek, uppercase ----
    sym("\\Gamma", "Γ", "Greek"),
    sym("\\Delta", "Δ", "Greek"),
    sym("\\Theta", "Θ", "Greek"),
    sym("\\Lambda", "Λ", "Greek"),
    sym("\\Xi", "Ξ", "Greek"),
    sym("\\Pi", "Π", "Greek"),
    sym("\\Sigma", "Σ", "Greek"),
    sym("\\Upsilon", "Υ", "Greek"),
    sym("\\Phi", "Φ", "Greek"),
    sym("\\Psi", "Ψ", "Greek"),
    sym("\\Omega", "Ω", "Greek"),
    // ---- Structure ----
    snip("\\frac", "Fraction", "Structure", "\\frac{$1}{$2}"),
    snip(
        "\\dfrac",
        "Display-style fraction",
        "Structure",
        "\\dfrac{$1}{$2}",
    ),
    snip(
        "\\tfrac",
        "Text-style fraction",
        "Structure",
        "\\tfrac{$1}{$2}",
    ),
    snip(
        "\\binom",
        "Binomial coefficient",
        "Structure",
        "\\binom{$1}{$2}",
    ),
    snip("\\sqrt", "Square root", "Structure", "\\sqrt{$1}"),
    snip("\\sqrt[n]", "nth root", "Structure", "\\sqrt[$1]{$2}"),
    snip("\\overline", "Overline", "Structure", "\\overline{$1}"),
    snip("\\underline", "Underline", "Structure", "\\underline{$1}"),
    snip(
        "\\overbrace",
        "Overbrace",
        "Structure",
        "\\overbrace{$1}^{$2}",
    ),
    snip(
        "\\underbrace",
        "Underbrace",
        "Structure",
        "\\underbrace{$1}_{$2}",
    ),
    snip(
        "\\substack",
        "Stacked subscript",
        "Structure",
        "\\substack{$1 \\\\ $2}",
    ),
    snip(
        "\\stackrel",
        "Stack above a relation",
        "Structure",
        "\\stackrel{$1}{$2}",
    ),
    snip(
        "\\text",
        "Upright text inside math",
        "Structure",
        "\\text{$1}",
    ),
    snip(
        "\\operatorname",
        "Custom operator name",
        "Structure",
        "\\operatorname{$1}",
    ),
    // ---- Big operators ----
    snip("\\sum", "Summation ∑", "Operators", "\\sum_{$1}^{$2}"),
    snip("\\prod", "Product ∏", "Operators", "\\prod_{$1}^{$2}"),
    snip("\\coprod", "Coproduct ∐", "Operators", "\\coprod_{$1}^{$2}"),
    snip("\\int", "Integral ∫", "Operators", "\\int_{$1}^{$2}"),
    snip("\\iint", "Double integral ∬", "Operators", "\\iint_{$1}"),
    snip("\\iiint", "Triple integral ∭", "Operators", "\\iiint_{$1}"),
    snip("\\oint", "Contour integral ∮", "Operators", "\\oint_{$1}"),
    snip("\\bigcup", "Big union ⋃", "Operators", "\\bigcup_{$1}^{$2}"),
    snip(
        "\\bigcap",
        "Big intersection ⋂",
        "Operators",
        "\\bigcap_{$1}^{$2}",
    ),
    snip(
        "\\bigoplus",
        "Big direct sum ⨁",
        "Operators",
        "\\bigoplus_{$1}",
    ),
    snip(
        "\\bigotimes",
        "Big tensor product ⨂",
        "Operators",
        "\\bigotimes_{$1}",
    ),
    snip("\\lim", "Limit", "Operators", "\\lim_{$1 \\to $2}"),
    snip("\\limsup", "Limit superior", "Operators", "\\limsup_{$1}"),
    snip("\\liminf", "Limit inferior", "Operators", "\\liminf_{$1}"),
    snip("\\max", "Maximum", "Operators", "\\max_{$1}"),
    snip("\\min", "Minimum", "Operators", "\\min_{$1}"),
    snip("\\arg\\max", "Argmax", "Operators", "\\arg\\max_{$1}"),
    snip("\\arg\\min", "Argmin", "Operators", "\\arg\\min_{$1}"),
    sym("\\sup", "Supremum", "Operators"),
    sym("\\inf", "Infimum", "Operators"),
    // ---- Named functions ----
    sym("\\exp", "Exponential", "Functions"),
    sym("\\log", "Logarithm", "Functions"),
    sym("\\ln", "Natural logarithm", "Functions"),
    sym("\\sin", "Sine", "Functions"),
    sym("\\cos", "Cosine", "Functions"),
    sym("\\tan", "Tangent", "Functions"),
    sym("\\arcsin", "Arcsine", "Functions"),
    sym("\\arccos", "Arccosine", "Functions"),
    sym("\\arctan", "Arctangent", "Functions"),
    sym("\\sinh", "Hyperbolic sine", "Functions"),
    sym("\\cosh", "Hyperbolic cosine", "Functions"),
    sym("\\tanh", "Hyperbolic tangent", "Functions"),
    sym("\\det", "Determinant", "Functions"),
    sym("\\dim", "Dimension", "Functions"),
    sym("\\ker", "Kernel", "Functions"),
    sym("\\deg", "Degree", "Functions"),
    sym("\\gcd", "Greatest common divisor", "Functions"),
    sym("\\Pr", "Probability", "Functions"),
    // ---- Relations ----
    sym("\\leq", "≤", "Relations"),
    sym("\\geq", "≥", "Relations"),
    sym("\\neq", "≠", "Relations"),
    sym("\\approx", "≈", "Relations"),
    sym("\\sim", "∼", "Relations"),
    sym("\\simeq", "≃", "Relations"),
    sym("\\cong", "≅", "Relations"),
    sym("\\equiv", "≡", "Relations"),
    sym("\\propto", "∝", "Relations"),
    sym("\\ll", "≪", "Relations"),
    sym("\\gg", "≫", "Relations"),
    sym("\\prec", "≺", "Relations"),
    sym("\\succ", "≻", "Relations"),
    sym("\\preceq", "⪯", "Relations"),
    sym("\\succeq", "⪰", "Relations"),
    sym("\\asymp", "≍", "Relations"),
    sym("\\doteq", "≐", "Relations"),
    sym("\\perp", "⊥", "Relations"),
    sym("\\parallel", "∥", "Relations"),
    sym("\\mid", "∣", "Relations"),
    // ---- Sets and logic ----
    sym("\\in", "∈", "Sets & logic"),
    sym("\\notin", "∉", "Sets & logic"),
    sym("\\ni", "∋", "Sets & logic"),
    sym("\\subset", "⊂", "Sets & logic"),
    sym("\\supset", "⊃", "Sets & logic"),
    sym("\\subseteq", "⊆", "Sets & logic"),
    sym("\\supseteq", "⊇", "Sets & logic"),
    sym("\\cup", "∪", "Sets & logic"),
    sym("\\cap", "∩", "Sets & logic"),
    sym("\\setminus", "∖", "Sets & logic"),
    sym("\\emptyset", "∅", "Sets & logic"),
    sym("\\varnothing", "∅ (variant)", "Sets & logic"),
    sym("\\forall", "∀", "Sets & logic"),
    sym("\\exists", "∃", "Sets & logic"),
    sym("\\nexists", "∄", "Sets & logic"),
    sym("\\neg", "¬", "Sets & logic"),
    sym("\\land", "∧", "Sets & logic"),
    sym("\\lor", "∨", "Sets & logic"),
    sym("\\implies", "⟹", "Sets & logic"),
    sym("\\iff", "⟺", "Sets & logic"),
    sym("\\therefore", "∴", "Sets & logic"),
    sym("\\because", "∵", "Sets & logic"),
    sym("\\top", "⊤", "Sets & logic"),
    sym("\\bot", "⊥", "Sets & logic"),
    sym("\\vdash", "⊢", "Sets & logic"),
    sym("\\models", "⊨", "Sets & logic"),
    // ---- Binary operators ----
    sym("\\times", "×", "Operators"),
    sym("\\div", "÷", "Operators"),
    sym("\\pm", "±", "Operators"),
    sym("\\mp", "∓", "Operators"),
    sym("\\cdot", "⋅", "Operators"),
    sym("\\ast", "∗", "Operators"),
    sym("\\star", "⋆", "Operators"),
    sym("\\circ", "∘", "Operators"),
    sym("\\bullet", "∙", "Operators"),
    sym("\\oplus", "⊕", "Operators"),
    sym("\\ominus", "⊖", "Operators"),
    sym("\\otimes", "⊗", "Operators"),
    sym("\\oslash", "⊘", "Operators"),
    sym("\\odot", "⊙", "Operators"),
    sym("\\wedge", "∧", "Operators"),
    sym("\\vee", "∨", "Operators"),
    // ---- Arrows ----
    sym("\\to", "→", "Arrows"),
    sym("\\gets", "←", "Arrows"),
    sym("\\leftarrow", "←", "Arrows"),
    sym("\\rightarrow", "→", "Arrows"),
    sym("\\leftrightarrow", "↔", "Arrows"),
    sym("\\Leftarrow", "⇐", "Arrows"),
    sym("\\Rightarrow", "⇒", "Arrows"),
    sym("\\Leftrightarrow", "⇔", "Arrows"),
    sym("\\longrightarrow", "⟶", "Arrows"),
    sym("\\longleftarrow", "⟵", "Arrows"),
    sym("\\mapsto", "↦", "Arrows"),
    sym("\\longmapsto", "⟼", "Arrows"),
    sym("\\uparrow", "↑", "Arrows"),
    sym("\\downarrow", "↓", "Arrows"),
    sym("\\nearrow", "↗", "Arrows"),
    sym("\\searrow", "↘", "Arrows"),
    sym("\\hookrightarrow", "↪", "Arrows"),
    sym("\\rightharpoonup", "⇀", "Arrows"),
    snip(
        "\\xrightarrow",
        "Labelled arrow",
        "Arrows",
        "\\xrightarrow{$1}",
    ),
    snip(
        "\\xleftarrow",
        "Labelled left arrow",
        "Arrows",
        "\\xleftarrow{$1}",
    ),
    // ---- Accents and decorations ----
    snip("\\hat", "Hat accent", "Accents", "\\hat{$1}"),
    snip("\\widehat", "Wide hat", "Accents", "\\widehat{$1}"),
    snip("\\bar", "Bar accent", "Accents", "\\bar{$1}"),
    snip("\\vec", "Vector arrow", "Accents", "\\vec{$1}"),
    snip("\\tilde", "Tilde accent", "Accents", "\\tilde{$1}"),
    snip("\\widetilde", "Wide tilde", "Accents", "\\widetilde{$1}"),
    snip("\\dot", "Dot accent", "Accents", "\\dot{$1}"),
    snip("\\ddot", "Double dot accent", "Accents", "\\ddot{$1}"),
    snip("\\check", "Caron accent", "Accents", "\\check{$1}"),
    snip("\\breve", "Breve accent", "Accents", "\\breve{$1}"),
    snip("\\boldsymbol", "Bold symbol", "Accents", "\\boldsymbol{$1}"),
    // ---- Delimiters ----
    snip(
        "\\left(",
        "Auto-sized parentheses",
        "Delimiters",
        "\\left( $1 \\right)",
    ),
    snip(
        "\\left[",
        "Auto-sized brackets",
        "Delimiters",
        "\\left[ $1 \\right]",
    ),
    snip(
        "\\left\\{",
        "Auto-sized braces",
        "Delimiters",
        "\\left\\\\{ $1 \\right\\\\}",
    ),
    snip(
        "\\langle",
        "Angle brackets ⟨ ⟩",
        "Delimiters",
        "\\langle $1 \\rangle",
    ),
    snip(
        "\\lvert",
        "Absolute value | |",
        "Delimiters",
        "\\lvert $1 \\rvert",
    ),
    snip("\\lVert", "Norm ‖ ‖", "Delimiters", "\\lVert $1 \\rVert"),
    snip(
        "\\lfloor",
        "Floor ⌊ ⌋",
        "Delimiters",
        "\\lfloor $1 \\rfloor",
    ),
    snip("\\lceil", "Ceiling ⌈ ⌉", "Delimiters", "\\lceil $1 \\rceil"),
    // ---- Dots, spacing, misc ----
    sym("\\ldots", "…", "Misc"),
    sym("\\cdots", "⋯", "Misc"),
    sym("\\vdots", "⋮", "Misc"),
    sym("\\ddots", "⋱", "Misc"),
    sym("\\infty", "∞", "Misc"),
    sym("\\partial", "∂", "Misc"),
    sym("\\nabla", "∇", "Misc"),
    sym("\\hbar", "ℏ", "Misc"),
    sym("\\ell", "ℓ", "Misc"),
    sym("\\Re", "ℜ", "Misc"),
    sym("\\Im", "ℑ", "Misc"),
    sym("\\aleph", "ℵ", "Misc"),
    sym("\\angle", "∠", "Misc"),
    sym("\\triangle", "△", "Misc"),
    sym("\\square", "□", "Misc"),
    sym("\\quad", "Wide space", "Misc"),
    sym("\\qquad", "Double wide space", "Misc"),
    // ---- Fonts ----
    snip(
        "\\mathbb",
        "Blackboard bold (ℝ, ℕ)",
        "Fonts",
        "\\mathbb{$1}",
    ),
    snip("\\mathcal", "Calligraphic", "Fonts", "\\mathcal{$1}"),
    snip("\\mathfrak", "Fraktur", "Fonts", "\\mathfrak{$1}"),
    snip("\\mathbf", "Bold", "Fonts", "\\mathbf{$1}"),
    snip("\\mathrm", "Roman (upright)", "Fonts", "\\mathrm{$1}"),
    snip("\\mathit", "Italic", "Fonts", "\\mathit{$1}"),
    snip("\\mathsf", "Sans-serif", "Fonts", "\\mathsf{$1}"),
    snip("\\mathtt", "Monospace", "Fonts", "\\mathtt{$1}"),
    // ---- Environments ----
    snip(
        "\\begin{aligned}",
        "Aligned equations",
        "Environments",
        "\\begin{aligned}\n  $1 &= $2 \\\\\\\\\n  $3 &= $4\n\\end{aligned}",
    ),
    snip(
        "\\begin{cases}",
        "Case distinction",
        "Environments",
        "\\begin{cases}\n  $1 & \\text{if } $2 \\\\\\\\\n  $3 & \\text{otherwise}\n\\end{cases}",
    ),
    snip(
        "\\begin{pmatrix}",
        "Matrix in parentheses",
        "Environments",
        "\\begin{pmatrix}\n  $1 & $2 \\\\\\\\\n  $3 & $4\n\\end{pmatrix}",
    ),
    snip(
        "\\begin{bmatrix}",
        "Matrix in brackets",
        "Environments",
        "\\begin{bmatrix}\n  $1 & $2 \\\\\\\\\n  $3 & $4\n\\end{bmatrix}",
    ),
    snip(
        "\\begin{vmatrix}",
        "Determinant matrix",
        "Environments",
        "\\begin{vmatrix}\n  $1 & $2 \\\\\\\\\n  $3 & $4\n\\end{vmatrix}",
    ),
    snip(
        "\\begin{array}",
        "Array with column alignment",
        "Environments",
        "\\begin{array}{${1:cc}}\n  $2 & $3 \\\\\\\\\n  $4 & $5\n\\end{array}",
    ),
];

/// The vocabulary as JSON, for `taliesin vocab` and the LSP.
pub(crate) fn math_commands() -> Value {
    Value::Array(
        MATH_COMMANDS
            .iter()
            .map(|c| {
                json!({
                    "name": c.name,
                    "description": c.description,
                    "category": c.category,
                    "snippet": c.snippet,
                })
            })
            .collect(),
    )
}

/// The LaTeX an entry actually inserts, with snippet placeholders resolved to a plain
/// symbol — what [`tests::every_command_renders`] feeds KaTeX. `${1:cc}` keeps its default
/// (a column spec is not interchangeable with `x`); a bare `$1` becomes `x`.
#[cfg(test)]
fn probe(cmd: &MathCommand) -> String {
    let src = if cmd.snippet.is_empty() {
        cmd.name
    } else {
        cmd.snippet
    };
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() {
            // `${n:default}` -> default; `$n` -> `x`.
            if chars[i + 1] == '{' {
                let close = chars[i..].iter().position(|&c| c == '}').map(|p| i + p);
                if let Some(close) = close {
                    let inner: String = chars[i + 2..close].iter().collect();
                    out.push_str(inner.split_once(':').map(|(_, d)| d).unwrap_or("x"));
                    i = close + 1;
                    continue;
                }
            } else if chars[i + 1].is_ascii_digit() {
                out.push('x');
                i += 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The load-bearing test.** Every offered command must render through the SAME KaTeX
    /// the document renderer uses. Without this the list is a guess, and a guess that is
    /// wrong ships an autocompletion whose result renders as a red `tali-math-error` span
    /// for the reader — the tool actively teaching a mistake.
    #[test]
    fn every_command_renders() {
        let mut broken = Vec::new();
        for cmd in MATH_COMMANDS {
            let latex = probe(cmd);
            let html = crate::math::render(&latex, false);
            if html.contains("tali-math-error") {
                broken.push(format!("  {} -> {latex}", cmd.name));
            }
        }
        assert!(
            broken.is_empty(),
            "these math commands do not render through the bundled KaTeX:\n{}",
            broken.join("\n")
        );
    }

    /// Names are unique, so the completion list cannot show a duplicate entry.
    #[test]
    fn names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for cmd in MATH_COMMANDS {
            assert!(
                seen.insert(cmd.name),
                "duplicate math command `{}`",
                cmd.name
            );
        }
    }

    /// Every entry is a control sequence. The completion context triggers on `\`, so an
    /// entry that does not start with one could never be reached by typing.
    #[test]
    fn every_name_is_a_control_sequence() {
        for cmd in MATH_COMMANDS {
            assert!(
                cmd.name.starts_with('\\'),
                "`{}` is offered but does not start with a backslash, so `\\`-triggered \
                 completion can never surface it",
                cmd.name
            );
            assert!(
                !cmd.description.is_empty(),
                "`{}` has no description",
                cmd.name
            );
            assert!(!cmd.category.is_empty(), "`{}` has no category", cmd.name);
        }
    }

    /// The leading control sequence of `name`: `\` plus its run of letters, or `\` plus one
    /// symbol (`\left\{` -> `\left`, `\sqrt[n]` -> `\sqrt`).
    fn leading_cs(name: &str) -> &str {
        let rest = &name[1..];
        let n = rest.chars().take_while(|c| c.is_ascii_alphabetic()).count();
        if n == 0 {
            &name[..1 + rest.chars().next().map_or(0, char::len_utf8)]
        } else {
            &name[..1 + n]
        }
    }

    /// A snippet must insert the command it is filed under, or accepting the completion
    /// writes something other than what the label promised.
    ///
    /// The comparison is on the leading control sequence, not the whole label: a label may
    /// carry an argument-shape hint the insert text spells differently (`\sqrt[n]` inserts
    /// `\sqrt[$1]{$2}`), and that is the label doing its job. What must never differ is
    /// which command you get.
    #[test]
    fn a_snippet_inserts_the_command_its_label_names() {
        for cmd in MATH_COMMANDS {
            if cmd.snippet.is_empty() {
                continue;
            }
            let cs = leading_cs(cmd.name);
            assert!(
                cmd.snippet.starts_with(cs),
                "`{}`'s snippet inserts `{}`, which does not start with `{cs}`",
                cmd.name,
                cmd.snippet
            );
        }
    }

    /// The picker groups by category; a one-off category is a typo more often than a group.
    #[test]
    fn every_category_has_at_least_two_members() {
        let mut counts = std::collections::HashMap::new();
        for cmd in MATH_COMMANDS {
            *counts.entry(cmd.category).or_insert(0) += 1;
        }
        for (category, n) in counts {
            assert!(
                n >= 2,
                "category `{category}` has only {n} member (a typo?)"
            );
        }
    }
}
