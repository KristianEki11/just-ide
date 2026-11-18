//! Scope-to-token classification for syntax highlighting.

use std::sync::OnceLock;
use syntect::parsing::{Scope, ScopeStack};

/// High-level semantic syntax token categories for Just IDE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SyntaxToken {
    /// Default unstyled text (e.g. whitespace, unclassified tokens).
    #[default]
    Default,
    /// Language keywords and storage specifiers (e.g. `fn`, `let`, `if`, `pub`).
    Keyword,
    /// Quoted string literals.
    String,
    /// Numeric literals (integers, floats, hex).
    Number,
    /// Named constants, booleans, and language constants.
    Constant,
    /// Single-line and multi-line comments.
    Comment,
    /// Function and method names or calls.
    Function,
    /// Type names, type definitions, and primitive types.
    Type,
    /// Class, struct, trait, or interface names.
    Class,
    /// Variable and field references.
    Variable,
    /// Function or method parameters.
    Parameter,
    /// Arithmetic, logical, and assignment operators.
    Operator,
    /// Brackets, braces, parentheses, commas, semicolons.
    Punctuation,
    /// Markup / HTML / XML tags.
    Tag,
    /// Markup attributes and CSS properties.
    Attribute,
    /// Regular expression literals.
    Regex,
    /// String escape sequences (e.g. `\n`, `\t`).
    Escape,
    /// Markdown or document headings.
    Heading,
    /// Markdown or document hyperlinks.
    Link,
    /// Invalid or illegal syntax constructs.
    Invalid,
    /// Diff additions (green).
    DiffAdded,
    /// Diff deletions (red).
    DiffRemoved,
}

struct ScopePatterns {
    comment: Option<Scope>,
    string_regexp: Option<Scope>,
    string: Option<Scope>,
    constant_escape: Option<Scope>,
    constant_numeric: Option<Scope>,
    constant: Option<Scope>,
    keyword_operator: Option<Scope>,
    keyword: Option<Scope>,
    storage: Option<Scope>,
    entity_name_function: Option<Scope>,
    support_function: Option<Scope>,
    variable_function: Option<Scope>,
    meta_function_call: Option<Scope>,
    entity_name_macro: Option<Scope>,
    support_other_macro: Option<Scope>,
    entity_name_class: Option<Scope>,
    entity_name_type: Option<Scope>,
    support_type_property: Option<Scope>,
    meta_object_key: Option<Scope>,
    support_type: Option<Scope>,
    variable_parameter: Option<Scope>,
    variable: Option<Scope>,
    punctuation: Option<Scope>,
    entity_name_tag: Option<Scope>,
    entity_other_attribute_name: Option<Scope>,
    markup_heading: Option<Scope>,
    markup_underline_link: Option<Scope>,
    markup_inserted: Option<Scope>,
    markup_deleted: Option<Scope>,
    invalid: Option<Scope>,
}

fn prefix_match(prefix: Option<Scope>, scope: Scope) -> bool {
    prefix.is_some_and(|p| p.is_prefix_of(scope))
}

impl ScopePatterns {
    fn new() -> Self {
        Self {
            comment: Scope::new("comment").ok(),
            string_regexp: Scope::new("string.regexp").ok(),
            string: Scope::new("string").ok(),
            constant_escape: Scope::new("constant.character.escape").ok(),
            constant_numeric: Scope::new("constant.numeric").ok(),
            constant: Scope::new("constant").ok(),
            keyword_operator: Scope::new("keyword.operator").ok(),
            keyword: Scope::new("keyword").ok(),
            storage: Scope::new("storage").ok(),
            entity_name_function: Scope::new("entity.name.function").ok(),
            support_function: Scope::new("support.function").ok(),
            variable_function: Scope::new("variable.function").ok(),
            meta_function_call: Scope::new("meta.function-call").ok(),
            entity_name_macro: Scope::new("entity.name.function.macro").ok(),
            support_other_macro: Scope::new("support.other.macro").ok(),
            entity_name_class: Scope::new("entity.name.class").ok(),
            entity_name_type: Scope::new("entity.name.type").ok(),
            support_type_property: Scope::new("support.type.property-name").ok(),
            meta_object_key: Scope::new("meta.object-literal.key").ok(),
            support_type: Scope::new("support.type").ok(),
            variable_parameter: Scope::new("variable.parameter").ok(),
            variable: Scope::new("variable").ok(),
            punctuation: Scope::new("punctuation").ok(),
            entity_name_tag: Scope::new("entity.name.tag").ok(),
            entity_other_attribute_name: Scope::new("entity.other.attribute-name").ok(),
            markup_heading: Scope::new("markup.heading").ok(),
            markup_underline_link: Scope::new("markup.underline.link").ok(),
            markup_inserted: Scope::new("markup.inserted").ok(),
            markup_deleted: Scope::new("markup.deleted").ok(),
            invalid: Scope::new("invalid").ok(),
        }
    }

    fn classify(&self, scope: Scope) -> Option<SyntaxToken> {
        if prefix_match(self.comment, scope) {
            return Some(SyntaxToken::Comment);
        }
        if prefix_match(self.string_regexp, scope) {
            return Some(SyntaxToken::Regex);
        }
        if prefix_match(self.string, scope) {
            return Some(SyntaxToken::String);
        }
        if prefix_match(self.constant_escape, scope) {
            return Some(SyntaxToken::Escape);
        }
        if prefix_match(self.constant_numeric, scope) {
            return Some(SyntaxToken::Number);
        }
        if prefix_match(self.constant, scope) {
            return Some(SyntaxToken::Constant);
        }
        if prefix_match(self.keyword_operator, scope) {
            return Some(SyntaxToken::Operator);
        }
        if prefix_match(self.keyword, scope) || prefix_match(self.storage, scope) {
            return Some(SyntaxToken::Keyword);
        }
        if prefix_match(self.entity_name_function, scope)
            || prefix_match(self.support_function, scope)
            || prefix_match(self.variable_function, scope)
            || prefix_match(self.meta_function_call, scope)
            || prefix_match(self.entity_name_macro, scope)
            || prefix_match(self.support_other_macro, scope)
        {
            return Some(SyntaxToken::Function);
        }
        if prefix_match(self.support_type_property, scope)
            || prefix_match(self.meta_object_key, scope)
        {
            return Some(SyntaxToken::Attribute);
        }
        if prefix_match(self.entity_name_class, scope) {
            return Some(SyntaxToken::Class);
        }
        if prefix_match(self.entity_name_type, scope) || prefix_match(self.support_type, scope) {
            return Some(SyntaxToken::Type);
        }
        if prefix_match(self.variable_parameter, scope) {
            return Some(SyntaxToken::Parameter);
        }
        if prefix_match(self.variable, scope) {
            return Some(SyntaxToken::Variable);
        }
        if prefix_match(self.punctuation, scope) {
            return Some(SyntaxToken::Punctuation);
        }
        if prefix_match(self.entity_name_tag, scope) {
            return Some(SyntaxToken::Tag);
        }
        if prefix_match(self.entity_other_attribute_name, scope) {
            return Some(SyntaxToken::Attribute);
        }
        if prefix_match(self.markup_heading, scope) {
            return Some(SyntaxToken::Heading);
        }
        if prefix_match(self.markup_underline_link, scope) {
            return Some(SyntaxToken::Link);
        }
        if prefix_match(self.markup_inserted, scope) {
            return Some(SyntaxToken::DiffAdded);
        }
        if prefix_match(self.markup_deleted, scope) {
            return Some(SyntaxToken::DiffRemoved);
        }
        if prefix_match(self.invalid, scope) {
            return Some(SyntaxToken::Invalid);
        }
        None
    }
}

static PATTERNS: OnceLock<ScopePatterns> = OnceLock::new();

fn get_patterns() -> &'static ScopePatterns {
    PATTERNS.get_or_init(ScopePatterns::new)
}

/// Maps a Sublime Text scope stack to the most specific `SyntaxToken`.
///
/// Iterates from top of stack (most specific) down to root (least specific).
#[must_use]
pub fn map_scope_stack(stack: &ScopeStack) -> SyntaxToken {
    let patterns = get_patterns();
    for &scope in stack.as_slice().iter().rev() {
        if let Some(token) = patterns.classify(scope) {
            return token;
        }
    }
    SyntaxToken::Default
}
