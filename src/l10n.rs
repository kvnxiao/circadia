use i18n_embed::I18nAssets;
use i18n_embed::I18nEmbedError;
use i18n_embed::LanguageLoader;
use i18n_embed::fluent::FluentLanguageLoader;
use i18n_embed::unic_langid::LanguageIdentifier;
use rust_embed::RustEmbed;
#[cfg(any(debug_assertions, test))]
use std::borrow::Cow;
use std::sync::OnceLock;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Catalogs;

macro_rules! text {
    ($text:expr, $($message:tt)+) => {{
        let text: &$crate::l10n::Text = &$text;
        let message = ::i18n_embed_fl::fl!(text.loader(), $($message)+);
        #[cfg(debug_assertions)]
        let message = text.finish(message);
        message
    }};
}

pub(crate) use text;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LanguageTag(LanguageIdentifier);

impl LanguageTag {
    pub(crate) fn english() -> Self {
        Self(english())
    }

    #[cfg(debug_assertions)]
    pub(crate) fn pseudo() -> Self {
        Self("qps-Ploc".parse().unwrap_or_default())
    }

    #[cfg(debug_assertions)]
    fn is_pseudo(&self) -> bool {
        *self == Self::pseudo()
    }
}

impl std::fmt::Display for LanguageTag {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

fn english() -> LanguageIdentifier {
    "en-US".parse().unwrap_or_default()
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown interface language {0:?}")]
pub(crate) struct UnknownLanguage(String);

pub(crate) const WINDOWS: &str = "windows";

pub(crate) fn shipped() -> Vec<LanguageTag> {
    languages(&Catalogs)
}

pub(crate) fn saveable(language: &str) -> bool {
    language == WINDOWS || shipped().iter().any(|tag| tag.to_string() == language)
}

pub(crate) fn language_name(language: &str) -> String {
    // Settings renders often, so each catalog is parsed once for its name.
    static NAMES: OnceLock<Vec<(String, String)>> = OnceLock::new();
    NAMES
        .get_or_init(|| {
            shipped()
                .iter()
                .map(|tag| {
                    let name = Text::load(tag)
                        .map_or_else(|_| tag.to_string(), |text| text!(text, "language-name"));
                    (tag.to_string(), name)
                })
                .collect()
        })
        .iter()
        .find(|(tag, _)| tag == language)
        .map_or_else(|| language.to_owned(), |(_, name)| name.clone())
}

fn languages(assets: &dyn I18nAssets) -> Vec<LanguageTag> {
    let mut languages: Vec<_> = assets
        .filenames_iter()
        .filter_map(|path| {
            let (language, _) = path.split_once('/')?;
            language.parse().ok().map(LanguageTag)
        })
        .collect();
    languages.sort_by_key(ToString::to_string);
    languages.dedup();
    languages
}

pub(crate) fn resolve(
    forced: Option<&str>,
    saved: Option<&str>,
    preferred: &[String],
) -> Result<LanguageTag, UnknownLanguage> {
    select(&shipped(), forced, saved, preferred)
}

fn select(
    shipped: &[LanguageTag],
    forced: Option<&str>,
    saved: Option<&str>,
    preferred: &[String],
) -> Result<LanguageTag, UnknownLanguage> {
    let parse = |tag: &str| tag.parse().ok().map(LanguageTag);
    if let Some(forced) = forced {
        let tag = parse(forced).ok_or_else(|| UnknownLanguage(forced.to_owned()))?;
        #[cfg(debug_assertions)]
        if tag.is_pseudo() {
            return Ok(tag);
        }
        return shipped
            .contains(&tag)
            .then_some(tag)
            .ok_or_else(|| UnknownLanguage(forced.to_owned()));
    }
    let matching = |tag: &str| {
        let tag = parse(tag)?;
        shipped
            .iter()
            .find(|shipped| **shipped == tag)
            .or_else(|| {
                shipped
                    .iter()
                    .find(|shipped| shipped.0.language == tag.0.language)
            })
            .cloned()
    };
    Ok(saved
        .and_then(parse)
        .filter(|tag| shipped.contains(tag))
        .or_else(|| preferred.iter().find_map(|tag| matching(tag)))
        .unwrap_or_else(LanguageTag::english))
}

pub(crate) struct Text {
    loader: FluentLanguageLoader,
    #[cfg(debug_assertions)]
    pseudo: bool,
}

impl Text {
    pub(crate) fn load(language: &LanguageTag) -> Result<Self, I18nEmbedError> {
        Self::load_from(&Catalogs, language)
    }

    fn load_from(assets: &dyn I18nAssets, language: &LanguageTag) -> Result<Self, I18nEmbedError> {
        let loader = FluentLanguageLoader::new(env!("CARGO_PKG_NAME"), english());
        #[cfg(debug_assertions)]
        let pseudo = language.is_pseudo();
        #[cfg(debug_assertions)]
        let catalog = if pseudo {
            english()
        } else {
            language.0.clone()
        };
        #[cfg(not(debug_assertions))]
        let catalog = language.0.clone();
        loader.load_languages(assets, &[catalog])?;
        #[cfg(debug_assertions)]
        if pseudo {
            loader.with_bundles_mut(|bundle| bundle.set_transform(Some(pseudo_text)));
        }
        Ok(Self {
            loader,
            #[cfg(debug_assertions)]
            pseudo,
        })
    }

    pub(crate) fn loader(&self) -> &FluentLanguageLoader {
        &self.loader
    }

    #[cfg(debug_assertions)]
    pub(crate) fn finish(&self, message: String) -> String {
        if self.pseudo {
            return pad(&message);
        }
        message
    }
}

/// Interface text that desktop checks compare with the settings window title,
/// the tray menu, the tray tooltip, and the status line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceText {
    /// Settings window title.
    pub app_name: String,
    /// Tray item that opens settings.
    pub tray_settings: String,
    /// Command that starts manual suppression.
    pub manual_start: String,
    /// Command that stops manual suppression.
    pub manual_stop: String,
    /// Tray caption above the recent apps while their toggles are available.
    pub tray_caption: String,
    /// Tray item that quits Circadia.
    pub tray_quit: String,
    /// Text that every tray tooltip starts with.
    pub tooltip_prefix: String,
    /// Status line while Night Light change observation fails.
    pub observation_failed: String,
    /// Status line while Night Light output fails.
    pub output_unavailable: String,
    /// Title of the file picker that Browse opens.
    pub picker_title: String,
}

impl InterfaceText {
    /// Load a shipped interface language, or `qps-Ploc` in debug builds.
    ///
    /// # Errors
    /// Report a language that Circadia does not ship or a catalog that does
    /// not load.
    pub fn load(language: &str) -> std::io::Result<Self> {
        let tag = resolve(Some(language), None, &[])
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
        let text = Text::load(&tag).map_err(std::io::Error::other)?;
        let tooltip = text!(text, "tooltip", status = "");
        Ok(Self {
            app_name: text!(text, "app-name"),
            tray_settings: text!(text, "tray-settings"),
            manual_start: text!(text, "manual-title"),
            manual_stop: text!(text, "manual-stop"),
            tray_caption: text!(text, "tray-caption", reason = "none"),
            tray_quit: text!(text, "tray-quit"),
            tooltip_prefix: tooltip
                .split_once('\u{2068}')
                .map_or(tooltip.as_str(), |(prefix, _)| prefix)
                .to_owned(),
            observation_failed: text!(text, "status-observation-failed"),
            output_unavailable: text!(text, "status-output-unavailable"),
            picker_title: text!(text, "dialog-file-title"),
        })
    }
}

#[cfg(debug_assertions)]
fn pseudo_text(text: &str) -> Cow<'_, str> {
    fluent_pseudo::transform(text, false, false)
}

// Fluent encloses each value in U+2068 and U+2069. Only text outside them is
// translated, so the padding adds 40% of that text's length; a nested
// pseudo-localized value is already padded. Accenting replaces each letter
// with one character, so the transformed text is as long as the English text.
#[cfg(debug_assertions)]
fn pad(message: &str) -> String {
    let mut depth = 0_usize;
    let mut translated = 0_usize;
    for character in message.chars() {
        match character {
            '\u{2068}' => depth += 1,
            '\u{2069}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => translated += 1,
            _ => {}
        }
    }
    let padding = (translated * 2).div_ceil(5).saturating_sub(2);
    let mut padded = String::with_capacity(message.len() + padding + 2);
    padded.push('[');
    padded.push_str(message);
    padded.extend((0..padding).map(|index| if index % 4 == 0 { ' ' } else { '!' }));
    padded.push(']');
    padded
}

#[cfg(test)]
mod tests {
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

    const FIXTURE_ENGLISH: &str = "\
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
fixture-greeting = Hallo
fixture-entries =
    { $count ->
        [one] { $count } Eintrag
       *[other] { $count } Einträge
    }
fixture-order = { $other_field } ({ $other } K) darf nicht höher sein als { $field } ({ $value } K).
";

    const FIXTURE: Fixture = Fixture(&[
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
            ast::InlineExpression::FunctionReference { arguments, .. } => {
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

    fn parity(english: &str, translation: &str) -> Vec<String> {
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
            resolve(Some("qps-Ploc"), Some("en-US"), &preferred(&["en-US"]))
                .map(|tag| tag.to_string()),
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
        let shipped: Vec<_> = shipped().iter().map(ToString::to_string).collect();
        assert_eq!(shipped, ["en-US"]);
    }

    #[test]
    fn each_shipped_language_has_its_own_name() {
        assert_eq!(language_name("en-US"), "English");
    }

    #[test]
    fn every_english_message_formats_without_errors() {
        let text = Text::load(&LanguageTag::english()).expect("English catalog loads");
        let messages = signatures(&catalog("en-US"));
        assert!(!messages.is_empty());
        for (id, names) in messages {
            let mut args = FluentArgs::new();
            for name in &names {
                args.set(name.clone(), "sample");
            }
            let errors = text
                .loader()
                .with_fluent_message_and_bundle(&id, |message, bundle| {
                    let mut errors = Vec::new();
                    let pattern = message.value().expect("message has a value");
                    bundle.format_pattern(pattern, Some(&args), &mut errors);
                    errors
                        .into_iter()
                        .map(|error| format!("{error:?}"))
                        .collect::<Vec<_>>()
                })
                .expect("message exists");
            assert!(errors.is_empty(), "{id}: {errors:?}");
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    fn pseudo_locale_brackets_and_lengthens_the_complete_message() {
        let english = Text::load(&LanguageTag::english()).expect("English catalog loads");
        let pseudo = Text::load(&LanguageTag::pseudo()).expect("pseudo-locale loads");
        assert_eq!(text!(english, "tray-quit"), "Quit Circadia");
        let translated = text!(pseudo, "tray-quit");
        assert!(translated.starts_with('['), "{translated}");
        assert!(translated.ends_with(']'), "{translated}");
        assert!(
            translated.chars().count() * 5 >= "Quit Circadia".chars().count() * 7,
            "{translated}"
        );
        assert!(!translated.contains("Quit Circadia"), "{translated}");
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
    fn every_shipped_translation_matches_the_english_messages_and_arguments() {
        let english = catalog("en-US");
        for name in Catalogs::iter() {
            let language = name.split_once('/').map_or("", |(language, _)| language);
            if language != "en-US" {
                assert_eq!(
                    parity(&english, &catalog(language)),
                    Vec::<String>::new(),
                    "{language}"
                );
            }
        }
    }

    #[test]
    fn parity_reports_a_missing_message_and_changed_arguments() {
        let problems = parity(
            FIXTURE_ENGLISH,
            "fixture-greeting = Hallo\nfixture-untranslated = Durchsuchen\nfixture-entries = { $total } Einträge\nfixture-order = { $field }\n",
        );
        assert_eq!(
            problems,
            [
                "fixture-entries uses {\"total\"} instead of {\"count\"}",
                "fixture-focus is missing",
                "fixture-order uses {\"field\"} instead of {\"field\", \"other\", \"other_field\", \"value\"}",
            ]
        );
        assert_eq!(
            parity(FIXTURE_ENGLISH, FIXTURE_ENGLISH),
            Vec::<String>::new()
        );
    }
}
