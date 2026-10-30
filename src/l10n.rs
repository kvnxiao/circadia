use fluent_bundle::FluentResource;
use i18n_embed::I18nAssets;
use i18n_embed::LanguageLoader;
use i18n_embed::fluent::FluentLanguageLoader;
use i18n_embed::unic_langid::LanguageIdentifier;
use rust_embed::RustEmbed;
#[cfg(any(debug_assertions, test))]
use std::borrow::Cow;
use std::io;
use std::sync::Arc;
use std::sync::OnceLock;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Catalogs;

macro_rules! bind_text {
    ($name:ident, $source:expr) => {
        $crate::l10n::bind_text!(@define $name, $source, $);
    };
    (@define $name:ident, $source:expr, $dollar:tt) => {
        macro_rules! $name {
            ($dollar($dollar message:tt)+) => {{
                let text: &$crate::l10n::Text = &$source;
                let message = ::i18n_embed_fl::fl!(text.loader(), $dollar($dollar message)+);
                #[cfg(debug_assertions)]
                let message = text.finish(message);
                message
            }};
        }
    };
}

pub(crate) use bind_text;

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
                    let name = Text::load(tag).map_or_else(
                        |_| tag.to_string(),
                        |text| {
                            bind_text!(fl, text);
                            fl!("language-name")
                        },
                    );
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
    pub(crate) fn load(language: &LanguageTag) -> io::Result<Self> {
        Self::load_from(&Catalogs, language)
    }

    fn load_from(assets: &dyn I18nAssets, language: &LanguageTag) -> io::Result<Self> {
        let shared = assets
            .get_files("shared.ftl")
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::other("shared translation terms are missing"))?;
        let source = String::from_utf8(shared.into_owned()).map_err(io::Error::other)?;
        let shared = Arc::new(FluentResource::try_new(source).map_err(|(_, errors)| {
            io::Error::other(format!("shared translation terms are invalid: {errors:?}"))
        })?);
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
        loader
            .load_languages(assets, &[catalog])
            .map_err(io::Error::other)?;
        loader.with_bundles_mut(|bundle| bundle.add_resource_overriding(Arc::clone(&shared)));
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
        let text = Text::load(&tag)?;
        bind_text!(fl, text);
        let tooltip = fl!("tooltip", status = "");
        Ok(Self {
            app_name: fl!("app-name"),
            tray_settings: fl!("tray-settings"),
            manual_start: fl!("manual-title"),
            manual_stop: fl!("manual-stop"),
            tray_caption: fl!("tray-caption", reason = "none"),
            tray_quit: fl!("tray-quit"),
            tooltip_prefix: tooltip
                .split_once('\u{2068}')
                .map_or(tooltip.as_str(), |(prefix, _)| prefix)
                .to_owned(),
            observation_failed: fl!("status-observation-failed"),
            output_unavailable: fl!("status-output-unavailable"),
            picker_title: fl!("dialog-file-title"),
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
mod tests;
