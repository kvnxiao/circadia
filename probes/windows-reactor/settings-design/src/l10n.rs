use i18n_embed::LanguageLoader;
use i18n_embed::fluent::FluentLanguageLoader;
use i18n_embed::fluent::fluent_language_loader;
use i18n_embed::unic_langid::LanguageIdentifier;
use rust_embed::RustEmbed;
use std::borrow::Cow;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Catalogs;

pub(crate) static LOADER: LazyLock<FluentLanguageLoader> =
    LazyLock::new(|| fluent_language_loader!());

pub(crate) static ENGLISH: LazyLock<Option<FluentLanguageLoader>> = LazyLock::new(|| {
    let loader = fluent_language_loader!();
    loader
        .load_fallback_language(&Catalogs)
        .ok()
        .map(|()| loader)
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Language {
    English,
    German,
    Pseudo,
}

impl Language {
    pub(crate) fn parse(tag: &str) -> Option<Self> {
        match tag {
            "en-US" => Some(Self::English),
            "de" => Some(Self::German),
            "qps-Ploc" => Some(Self::Pseudo),
            _ => None,
        }
    }

    pub(crate) fn tag(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::German => "de",
            Self::Pseudo => "qps-Ploc",
        }
    }
}

pub(crate) fn load(language: Language) -> anyhow::Result<()> {
    let english: LanguageIdentifier = "en-US".parse()?;
    let languages = match language {
        Language::English | Language::Pseudo => vec![english],
        Language::German => vec!["de".parse()?, english],
    };
    LOADER.load_languages(&Catalogs, &languages)?;
    if language == Language::Pseudo {
        LOADER.with_bundles_mut(|bundle| bundle.set_transform(Some(pseudo)));
    }
    Ok(())
}

pub(crate) fn pseudo(text: &str) -> Cow<'_, str> {
    fluent_pseudo::transform_dom(text, false, true, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluent_syntax::ast;
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;

    fn catalog(language: &str) -> BTreeMap<String, BTreeSet<String>> {
        let path = format!("{language}/circadia_settings_design_probe.ftl");
        let file = Catalogs::get(&path).expect("embedded catalog exists");
        let source = std::str::from_utf8(&file.data).expect("catalog is UTF-8");
        let resource = fluent_syntax::parser::parse(source).expect("catalog parses");
        resource
            .body
            .iter()
            .filter_map(|entry| match entry {
                ast::Entry::Message(message) => Some((
                    message.id.name.to_owned(),
                    message.value.as_ref().map(variables).unwrap_or_default(),
                )),
                _ => None,
            })
            .collect()
    }

    fn variables(pattern: &ast::Pattern<&str>) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for element in &pattern.elements {
            if let ast::PatternElement::Placeable { expression } = element {
                expression_variables(expression, &mut names);
            }
        }
        names
    }

    fn expression_variables(expression: &ast::Expression<&str>, names: &mut BTreeSet<String>) {
        let inline = match expression {
            ast::Expression::Inline(inline) => inline,
            ast::Expression::Select { selector, variants } => {
                for variant in variants {
                    names.extend(variables(&variant.value));
                }
                selector
            }
        };
        match inline {
            ast::InlineExpression::VariableReference { id } => {
                names.insert(id.name.to_owned());
            }
            ast::InlineExpression::Placeable { expression } => {
                expression_variables(expression, names);
            }
            _ => {}
        }
    }

    #[test]
    fn translations_use_english_messages_and_variables() {
        let english = catalog("en-US");
        let german = catalog("de");
        assert!(
            !german.is_empty(),
            "the reordering fixture must translate messages"
        );
        for (id, names) in &german {
            assert_eq!(english.get(id), Some(names), "{id} must match English");
        }
    }

    #[test]
    fn german_reorders_placeholders_and_falls_back_per_message() {
        load(Language::German).expect("catalogs load");
        let ordered = LOADER.get_args(
            "error-order",
            std::collections::HashMap::from([
                ("field", "Morgen und Abend"),
                ("value", "1900"),
                ("other_field", "Schlafenszeit"),
                ("other", "2400"),
            ]),
        );
        let other = ordered.find("2400").expect("second value is present");
        let value = ordered.find("1900").expect("first value is present");
        assert!(
            other < value,
            "German places the Bedtime value first: {ordered}"
        );
        assert_eq!(LOADER.get("browse"), "Browse");
    }
}
