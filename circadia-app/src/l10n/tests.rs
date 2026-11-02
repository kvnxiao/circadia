use super::*;
use fluent_bundle::FluentArgs;
use fluent_syntax::ast;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;

const ISOLATE: char = '\u{2068}';
const END_ISOLATE: char = '\u{2069}';

struct Fixture(&'static [(&'static str, &'static str)]);

impl I18nAssets for Fixture {
    fn get_files(&self, file_path: &str) -> Vec<Cow<'_, [u8]>> {
        self.0
            .iter()
            .filter(|(path, _)| *path == file_path)
            .map(|(_, source)| Cow::Borrowed(source.as_bytes()))
            .collect()
    }

    fn filenames_iter(&self) -> Box<dyn Iterator<Item = String> + '_> {
        Box::new(self.0.iter().map(|(path, _)| (*path).to_owned()))
    }
}

const FIXTURE_SHARED: &str = r#"-brand-name = { "Sample Brand" }
"#;

const FIXTURE_ENGLISH: &str = "\
-night-light = Night light
fixture-brand = { -brand-name } uses { -night-light }.
fixture-greeting = Hello
fixture-untranslated = Browse
fixture-entries =
{ $count ->
    [one] { $count } entry
   *[other] { $count } entries
}
fixture-order = { $field } ({ $value } K) can't be lower than { $other_field } ({ $other } K).
fixture-focus = { $app } was focused at { $time }
";

const FIXTURE_GERMAN: &str = "\
-night-light = Nachtmodus
fixture-brand = { -brand-name } verwendet { -night-light }.
fixture-greeting = Hallo
fixture-entries =
{ $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträge
}
fixture-order = { $other_field } ({ $other } K) darf nicht höher sein als { $field } ({ $value } K).
";

const FIXTURE: Fixture = Fixture(&[
    ("shared.ftl", FIXTURE_SHARED),
    ("en-US/circadia.ftl", FIXTURE_ENGLISH),
    ("de/circadia.ftl", FIXTURE_GERMAN),
]);

fn german() -> LanguageTag {
    LanguageTag("de".parse().expect("fixture tag parses"))
}

fn plain(text: &str) -> String {
    text.chars()
        .filter(|character| !matches!(*character, ISOLATE | END_ISOLATE))
        .collect()
}

fn signatures(source: &str) -> BTreeMap<String, BTreeSet<String>> {
    let resource = fluent_syntax::parser::parse(source).expect("catalog parses");
    resource
        .body
        .iter()
        .filter_map(|entry| match entry {
            ast::Entry::Message(message) => {
                let mut names = BTreeSet::new();
                for pattern in message
                    .value
                    .iter()
                    .chain(message.attributes.iter().map(|attribute| &attribute.value))
                {
                    pattern_variables(pattern, &mut names);
                }
                Some((message.id.name.to_owned(), names))
            }
            _ => None,
        })
        .collect()
}

fn pattern_variables(pattern: &ast::Pattern<&str>, names: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            expression_variables(expression, names);
        }
    }
}

fn expression_variables(expression: &ast::Expression<&str>, names: &mut BTreeSet<String>) {
    match expression {
        ast::Expression::Inline(inline) => inline_variables(inline, names),
        ast::Expression::Select { selector, variants } => {
            inline_variables(selector, names);
            for variant in variants {
                pattern_variables(&variant.value, names);
            }
        }
    }
}

fn inline_variables(inline: &ast::InlineExpression<&str>, names: &mut BTreeSet<String>) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            names.insert(id.name.to_owned());
        }
        ast::InlineExpression::Placeable { expression } => {
            expression_variables(expression, names);
        }
        ast::InlineExpression::FunctionReference { arguments, .. }
        | ast::InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => {
            for argument in &arguments.positional {
                inline_variables(argument, names);
            }
            for argument in &arguments.named {
                inline_variables(&argument.value, names);
            }
        }
        _ => {}
    }
}

fn terms(source: &str) -> BTreeSet<String> {
    fluent_syntax::parser::parse(source)
        .expect("catalog parses")
        .body
        .iter()
        .filter_map(|entry| match entry {
            ast::Entry::Term(term) => Some(term.id.name.to_owned()),
            _ => None,
        })
        .collect()
}

fn parity(english: &str, translation: &str, shared: &str) -> Vec<String> {
    let english_terms = terms(english);
    let translated_terms = terms(translation);
    let shared_terms = terms(shared);
    let english = signatures(english);
    let translation = signatures(translation);
    let mut problems: Vec<String> = english
        .iter()
        .filter_map(|(id, names)| match translation.get(id) {
            None => Some(format!("{id} is missing")),
            Some(translated) if translated != names => {
                Some(format!("{id} uses {translated:?} instead of {names:?}"))
            }
            Some(_) => None,
        })
        .collect();
    problems.extend(
        translation
            .keys()
            .filter(|id| !english.contains_key(*id))
            .map(|id| format!("{id} is not an English message")),
    );
    problems.extend(
        english_terms
            .difference(&translated_terms)
            .map(|id| format!("-{id} is missing")),
    );
    problems.extend(
        translated_terms
            .intersection(&shared_terms)
            .map(|id| format!("-{id} is shared and must not be redefined")),
    );
    problems
}

fn catalog(language: &str) -> String {
    let file = Catalogs::get(&format!("{language}/circadia.ftl")).expect("catalog is embedded");
    String::from_utf8(file.data.into_owned()).expect("catalog is UTF-8")
}

fn preferred(tags: &[&str]) -> Vec<String> {
    tags.iter().map(|tag| (*tag).to_owned()).collect()
}

#[test]
fn interface_text_matches_the_english_catalog() {
    let text = InterfaceText::load("en-US").expect("English text");
    assert_eq!(text.app_name, "Circadia");
    assert_eq!(text.tray_settings, "Open Circadia settings");
    assert_eq!(text.manual_start, "Suppress for 1 hour");
    assert_eq!(text.manual_stop, "Stop suppressing");
    assert_eq!(text.tray_caption, "Suppress while focused");
    assert_eq!(text.tray_quit, "Quit Circadia");
    assert_eq!(text.tooltip_prefix, "Circadia: ");
}

#[cfg(debug_assertions)]
#[test]
fn interface_text_in_the_pseudo_locale_is_bracketed() {
    let text = InterfaceText::load("qps-Ploc").expect("pseudo-locale text");
    assert!(text.app_name.starts_with('[') && text.app_name.ends_with(']'));
    assert!(text.tooltip_prefix.starts_with('['));
    assert_ne!(text.tray_quit, "Quit Circadia");
}

#[test]
fn interface_text_rejects_a_language_that_is_not_shipped() {
    assert_eq!(
        InterfaceText::load("de-DE").map_err(|error| error.kind()),
        Err(std::io::ErrorKind::InvalidInput)
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn release_builds_reject_the_pseudo_locale() {
    assert_eq!(
        resolve(Some("qps-Ploc"), Some("en-US"), &[]),
        Err(UnknownLanguage("qps-Ploc".to_owned()))
    );
    assert_eq!(
        InterfaceText::load("qps-Ploc").map_err(|error| error.kind()),
        Err(std::io::ErrorKind::InvalidInput)
    );
}

#[cfg(debug_assertions)]
#[test]
fn a_forced_pseudo_locale_wins_over_every_other_input() {
    assert_eq!(
        resolve(Some("qps-Ploc"), Some("en-US"), &preferred(&["en-US"])).map(|tag| tag.to_string()),
        Ok("qps-Ploc".to_owned())
    );
}

#[test]
fn windows_languages_without_a_catalog_resolve_to_english() {
    assert_eq!(
        resolve(None, None, &preferred(&["fr-FR", "en-GB"])).map(|tag| tag.to_string()),
        Ok("en-US".to_owned())
    );
    assert_eq!(
        resolve(None, Some("windows"), &[]).map(|tag| tag.to_string()),
        Ok("en-US".to_owned())
    );
}

#[test]
fn an_unknown_forced_language_is_rejected() {
    assert_eq!(
        resolve(Some("xx-XX"), None, &preferred(&["en-US"])),
        Err(UnknownLanguage("xx-XX".to_owned()))
    );
    assert_eq!(
        resolve(Some("not a tag"), None, &[]),
        Err(UnknownLanguage("not a tag".to_owned()))
    );
}

#[test]
fn the_first_windows_language_with_a_catalog_is_selected_and_a_saved_language_wins() {
    let shipped = languages(&FIXTURE);
    let selected = |saved, tags: &[&str]| {
        select(&shipped, None, saved, &preferred(tags)).map(|tag| tag.to_string())
    };
    assert_eq!(
        selected(None, &["fr-FR", "de-AT", "en-US"]),
        Ok("de".to_owned())
    );
    assert_eq!(selected(None, &["en-GB", "de"]), Ok("en-US".to_owned()));
    assert_eq!(selected(Some("de"), &["en-US"]), Ok("de".to_owned()));
    assert_eq!(selected(Some("windows"), &["de-DE"]), Ok("de".to_owned()));
}

#[test]
fn shipped_languages_are_the_real_catalogs() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n");
    let expected: BTreeSet<_> = fs_err::read_dir(directory)
        .expect("catalog directory")
        .map(|entry| entry.expect("catalog entry").path())
        .filter(|path| path.join("circadia.ftl").is_file())
        .map(|path| {
            path.file_name()
                .expect("language directory")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(expected.contains("en-US"));
    assert_eq!(
        shipped()
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>(),
        expected
    );
}

#[test]
fn each_shipped_language_has_its_own_name() {
    assert_eq!(language_name("en-US"), "English");
    for language in shipped() {
        let tag = language.to_string();
        let name = language_name(&tag);
        assert!(!name.is_empty() && name != tag, "{tag}: {name}");
    }
}

fn formatting_errors(text: &Text, id: &str, names: &BTreeSet<String>) -> Vec<String> {
    let args: FluentArgs<'_> = names.iter().map(|name| (name.as_str(), "sample")).collect();
    text.loader()
        .with_fluent_message_and_bundle(id, |message, bundle| {
            let mut errors = Vec::new();
            let pattern = message.value().expect("message has a value");
            bundle.format_pattern(pattern, Some(&args), &mut errors);
            errors
                .into_iter()
                .map(|error| format!("{error:?}"))
                .collect()
        })
        .expect("message exists")
}

#[test]
fn every_shipped_message_formats_without_errors() {
    for language in shipped() {
        let text = Text::load(&language).expect("catalog loads");
        let messages = signatures(&catalog(&language.to_string()));
        assert!(!messages.is_empty());
        for (id, names) in messages {
            assert_eq!(
                formatting_errors(&text, &id, &names),
                Vec::<String>::new(),
                "{id}"
            );
        }
    }
}

#[test]
fn formatting_reports_an_unknown_term_reference() {
    const FIXTURE: Fixture = Fixture(&[
        ("shared.ftl", FIXTURE_SHARED),
        ("en-US/circadia.ftl", "message = { -missing-term }"),
    ]);
    let text = Text::load_from(&FIXTURE, &LanguageTag::english()).expect("fixture loads");
    assert_ne!(
        formatting_errors(&text, "message", &BTreeSet::new()),
        Vec::<String>::new()
    );
}

#[test]
fn scoped_alias_formats_nested_messages_in_a_closure() {
    let text = Text::load(&LanguageTag::english()).expect("English catalog loads");
    bind_text!(fl, text);
    let format = || {
        fl!(
            "error-same-clock",
            field = fl!("wake-title"),
            other = fl!("bedtime-title"),
        )
    };
    assert_eq!(
        plain(&format()),
        "Wake time and Bedtime can't be the same. This change isn't applied yet."
    );
}

#[cfg(debug_assertions)]
#[test]
fn pseudo_locale_brackets_and_lengthens_the_complete_message() {
    let english = Text::load(&LanguageTag::english()).expect("English catalog loads");
    let pseudo = Text::load(&LanguageTag::pseudo()).expect("pseudo-locale loads");
    bind_text!(english_fl, english);
    bind_text!(pseudo_fl, pseudo);
    assert_eq!(english_fl!("tray-quit"), "Quit Circadia");
    let translated = pseudo_fl!("tray-quit");
    assert!(translated.starts_with('['), "{translated}");
    assert!(translated.ends_with(']'), "{translated}");
    assert!(
        translated.chars().count() * 5 >= "Quit Circadia".chars().count() * 7,
        "{translated}"
    );
    assert!(!translated.contains("Quit Circadia"), "{translated}");
    assert_eq!(english_fl!("tray-quit"), "Quit Circadia");
}

#[cfg(debug_assertions)]
#[test]
fn pseudo_locale_keeps_values_unchanged() {
    let pseudo = Text::load_from(&FIXTURE, &LanguageTag::pseudo()).expect("fixture loads");
    let message = pseudo.finish(pseudo.loader().get_args(
        "fixture-focus",
        HashMap::from([("app", "javaw.exe"), ("time", "10:30 PM")]),
    ));
    assert!(message.contains("\u{2068}javaw.exe\u{2069}"), "{message}");
    assert!(message.contains("\u{2068}10:30 PM\u{2069}"), "{message}");
    assert!(!message.contains("was focused at"), "{message}");
    assert!(
        message.starts_with('[') && message.ends_with(']'),
        "{message}"
    );
}

#[test]
fn a_message_missing_from_a_translation_uses_english() {
    let text = Text::load_from(&FIXTURE, &german()).expect("fixture loads");
    assert_eq!(text.loader().get("fixture-greeting"), "Hallo");
    assert_eq!(text.loader().get("fixture-untranslated"), "Browse");
}

#[test]
fn counts_use_the_plural_rules_of_the_language() {
    let text = Text::load_from(&FIXTURE, &LanguageTag::english()).expect("fixture loads");
    let entries = |count: u32| {
        plain(
            &text
                .loader()
                .get_args("fixture-entries", HashMap::from([("count", count)])),
        )
    };
    assert_eq!(entries(1), "1 entry");
    assert_eq!(entries(3), "3 entries");
}

#[test]
fn a_translation_places_each_named_value_in_its_own_order() {
    let text = Text::load_from(&FIXTURE, &german()).expect("fixture loads");
    let message = text.loader().get_args(
        "fixture-order",
        HashMap::from([
            ("field", "Morgen und Abend"),
            ("value", "1900"),
            ("other_field", "Schlafenszeit"),
            ("other", "2400"),
        ]),
    );
    assert_eq!(
        plain(&message),
        "Schlafenszeit (2400 K) darf nicht höher sein als Morgen und Abend (1900 K)."
    );
}

#[test]
fn every_shipped_catalog_matches_the_english_messages_arguments_and_terms() {
    let english = catalog("en-US");
    let shared = include_str!("../../i18n/shared.ftl");
    for language in shipped() {
        assert_eq!(
            parity(&english, &catalog(&language.to_string()), shared),
            Vec::<String>::new(),
            "{language}"
        );
    }
}

#[test]
fn parity_reports_a_missing_message_and_changed_arguments() {
    let problems = parity(
        FIXTURE_ENGLISH,
        "fixture-greeting = Hallo\nfixture-untranslated = Durchsuchen\nfixture-entries = { $total } Einträge\nfixture-order = { $field }\n",
        FIXTURE_SHARED,
    );
    assert_eq!(
        problems,
        [
            "fixture-brand is missing",
            "fixture-entries uses {\"total\"} instead of {\"count\"}",
            "fixture-focus is missing",
            "fixture-order uses {\"field\"} instead of {\"field\", \"other\", \"other_field\", \"value\"}",
            "-night-light is missing",
        ]
    );
    assert_eq!(
        parity(FIXTURE_ENGLISH, FIXTURE_ENGLISH, FIXTURE_SHARED),
        Vec::<String>::new()
    );
}

#[test]
fn shared_terms_are_invariant_string_literals() {
    let source = include_str!("../../i18n/shared.ftl");
    let resource = fluent_syntax::parser::parse(source).expect("shared terms parse");
    assert!(!terms(source).is_empty());
    for entry in resource.body {
        match entry {
            ast::Entry::Term(term) => {
                assert!(term.attributes.is_empty(), "{}", term.id.name);
                assert!(
                    matches!(
                        term.value.elements.as_slice(),
                        [ast::PatternElement::Placeable {
                            expression: ast::Expression::Inline(
                                ast::InlineExpression::StringLiteral { .. }
                            )
                        }]
                    ),
                    "{} must be an invariant string literal",
                    term.id.name
                );
            }
            ast::Entry::Comment(_)
            | ast::Entry::GroupComment(_)
            | ast::Entry::ResourceComment(_) => {}
            _ => panic!("shared.ftl must define only invariant terms"),
        }
    }
}

#[test]
fn shared_brand_and_locale_feature_terms_format_together() {
    let english = Text::load_from(&FIXTURE, &LanguageTag::english()).expect("fixture loads");
    let german = Text::load_from(&FIXTURE, &german()).expect("fixture loads");
    assert_eq!(
        english.loader().get("fixture-brand"),
        "Sample Brand uses Night light."
    );
    assert_eq!(
        german.loader().get("fixture-brand"),
        "Sample Brand verwendet Nachtmodus."
    );
}

#[test]
fn shared_brand_overrides_a_locale_redefinition() {
    const FIXTURE: Fixture = Fixture(&[
        (
            "shared.ftl",
            "-brand-name = { \"Renamed Brand\" }
",
        ),
        ("en-US/circadia.ftl", FIXTURE_ENGLISH),
        (
            "de/circadia.ftl",
            "-brand-name = Translated name
fixture-brand = { -brand-name }
",
        ),
    ]);
    let english = Text::load_from(&FIXTURE, &LanguageTag::english()).expect("fixture loads");
    let german = Text::load_from(&FIXTURE, &german()).expect("fixture loads");
    assert_eq!(
        english.loader().get("fixture-brand"),
        "Renamed Brand uses Night light."
    );
    assert_eq!(german.loader().get("fixture-brand"), "Renamed Brand");
}

#[test]
fn missing_or_invalid_shared_terms_prevent_loading() {
    const MISSING: Fixture = Fixture(&[("en-US/circadia.ftl", FIXTURE_ENGLISH)]);
    const INVALID: Fixture = Fixture(&[
        (
            "shared.ftl",
            "-brand-name = {
",
        ),
        ("en-US/circadia.ftl", FIXTURE_ENGLISH),
    ]);
    for fixture in [&MISSING, &INVALID] {
        assert!(Text::load_from(fixture, &LanguageTag::english()).is_err());
    }
}

#[cfg(debug_assertions)]
#[test]
fn pseudo_locale_preserves_shared_brand_names() {
    let pseudo = Text::load(&LanguageTag::pseudo()).expect("pseudo-locale loads");
    bind_text!(fl, pseudo);
    assert!(fl!("app-name").contains("Circadia"));
    let notice = fl!("banner-crash-message");
    assert!(
        notice.contains("Circadia") && notice.contains("GitHub"),
        "{notice}"
    );
}

#[test]
fn parity_rejects_missing_terms_and_shared_redefinitions() {
    assert_eq!(
        parity(
            "-night-light = Night light
message = { -night-light }
",
            "-brand-name = Translated name
message = Translated message
",
            FIXTURE_SHARED,
        ),
        [
            "-night-light is missing",
            "-brand-name is shared and must not be redefined"
        ]
    );
}

#[test]
fn parity_allows_locale_specific_term_forms() {
    assert_eq!(
        parity(
            "-night-light = Night light
message = { -night-light }
",
            "-night-light = { $case ->
[genitive] Nachtmodus
   *[other] Nachtmodus
}
-helper = Zusatz
message = { -night-light(case: \"genitive\") }
",
            FIXTURE_SHARED,
        ),
        Vec::<String>::new()
    );
}
