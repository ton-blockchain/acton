use clap::ValueEnum;
use owo_colors::Style;
use std::env;
use std::fmt;
use std::io::IsTerminal;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(ValueEnum, Clone, Copy, Debug, Eq, PartialEq, Default)]
#[value(rename_all = "lowercase")]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl fmt::Display for ColorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColorMode::Auto => write!(f, "auto"),
            ColorMode::Always => write!(f, "always"),
            ColorMode::Never => write!(f, "never"),
        }
    }
}

static COLOR_MODE: AtomicU8 = AtomicU8::new(ColorMode::Auto as u8);
static AUTO_COLOR_ENABLED: OnceLock<bool> = OnceLock::new();

#[must_use]
pub fn color_mode() -> ColorMode {
    match COLOR_MODE.load(Ordering::Relaxed) {
        1 => ColorMode::Always,
        2 => ColorMode::Never,
        _ => ColorMode::Auto,
    }
}

pub fn init_color_mode(mode: ColorMode) {
    COLOR_MODE.store(mode as u8, Ordering::Relaxed);
}

#[must_use]
pub fn colors_enabled() -> bool {
    match color_mode() {
        ColorMode::Auto => auto_colors_enabled(),
        ColorMode::Always => true,
        ColorMode::Never => false,
    }
}

fn auto_colors_enabled() -> bool {
    *AUTO_COLOR_ENABLED.get_or_init(detect_auto_color_support)
}

fn detect_auto_color_support() -> bool {
    if env::var_os("NO_COLOR").is_some() {
        return false;
    }

    if env_flag_enabled("CLICOLOR_FORCE") || env_flag_enabled("FORCE_COLOR") {
        return true;
    }

    if env_flag_disabled("CLICOLOR") {
        return false;
    }

    std::io::stdout().is_terminal()
}

fn env_flag_enabled(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| {
        let value = value.to_string_lossy().trim().to_ascii_lowercase();
        !value.is_empty() && value != "0" && value != "false"
    })
}

fn env_flag_disabled(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| {
        let value = value.to_string_lossy().trim().to_ascii_lowercase();
        value == "0" || value == "false"
    })
}

pub struct Styled<'a, T: ?Sized> {
    value: &'a T,
    style: Style,
}

impl<'a, T: ?Sized> Styled<'a, T> {
    #[must_use]
    pub const fn new(value: &'a T) -> Self {
        Self {
            value,
            style: Style::new(),
        }
    }

    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn black(self) -> Self {
        self.with_style(Style::black)
    }

    #[must_use]
    pub fn red(self) -> Self {
        self.with_style(Style::red)
    }

    #[must_use]
    pub fn green(self) -> Self {
        self.with_style(Style::green)
    }

    #[must_use]
    pub fn yellow(self) -> Self {
        self.with_style(Style::yellow)
    }

    #[must_use]
    pub fn blue(self) -> Self {
        self.with_style(Style::blue)
    }

    #[must_use]
    pub fn magenta(self) -> Self {
        self.with_style(Style::magenta)
    }

    #[must_use]
    pub fn purple(self) -> Self {
        self.with_style(Style::purple)
    }

    #[must_use]
    pub fn cyan(self) -> Self {
        self.with_style(Style::cyan)
    }

    #[must_use]
    pub fn white(self) -> Self {
        self.with_style(Style::white)
    }

    #[must_use]
    pub fn bright_black(self) -> Self {
        self.with_style(Style::bright_black)
    }

    #[must_use]
    pub fn bright_red(self) -> Self {
        self.with_style(Style::bright_red)
    }

    #[must_use]
    pub fn bright_green(self) -> Self {
        self.with_style(Style::bright_green)
    }

    #[must_use]
    pub fn bright_yellow(self) -> Self {
        self.with_style(Style::bright_yellow)
    }

    #[must_use]
    pub fn bright_blue(self) -> Self {
        self.with_style(Style::bright_blue)
    }

    #[must_use]
    pub fn bright_magenta(self) -> Self {
        self.with_style(Style::bright_magenta)
    }

    #[must_use]
    pub fn bright_purple(self) -> Self {
        self.with_style(Style::bright_purple)
    }

    #[must_use]
    pub fn bright_cyan(self) -> Self {
        self.with_style(Style::bright_cyan)
    }

    #[must_use]
    pub fn bright_white(self) -> Self {
        self.with_style(Style::bright_white)
    }

    #[must_use]
    pub fn on_black(self) -> Self {
        self.with_style(Style::on_black)
    }

    #[must_use]
    pub fn on_red(self) -> Self {
        self.with_style(Style::on_red)
    }

    #[must_use]
    pub fn on_green(self) -> Self {
        self.with_style(Style::on_green)
    }

    #[must_use]
    pub fn on_yellow(self) -> Self {
        self.with_style(Style::on_yellow)
    }

    #[must_use]
    pub fn on_blue(self) -> Self {
        self.with_style(Style::on_blue)
    }

    #[must_use]
    pub fn on_magenta(self) -> Self {
        self.with_style(Style::on_magenta)
    }

    #[must_use]
    pub fn on_purple(self) -> Self {
        self.with_style(Style::on_purple)
    }

    #[must_use]
    pub fn on_cyan(self) -> Self {
        self.with_style(Style::on_cyan)
    }

    #[must_use]
    pub fn on_white(self) -> Self {
        self.with_style(Style::on_white)
    }

    #[must_use]
    pub fn bold(self) -> Self {
        self.with_style(Style::bold)
    }

    #[must_use]
    pub fn dimmed(self) -> Self {
        self.with_style(Style::dimmed)
    }

    #[must_use]
    pub fn italic(self) -> Self {
        self.with_style(Style::italic)
    }

    #[must_use]
    pub fn underline(self) -> Self {
        self.with_style(Style::underline)
    }

    #[must_use]
    pub fn strikethrough(self) -> Self {
        self.with_style(Style::strikethrough)
    }

    #[must_use]
    fn with_style(mut self, apply: impl FnOnce(Style) -> Style) -> Self {
        self.style = apply(self.style);
        self
    }

    fn fmt_inner(
        &self,
        f: &mut fmt::Formatter<'_>,
        fmt_inner: impl FnOnce(&'a T, &mut fmt::Formatter<'_>) -> fmt::Result,
    ) -> fmt::Result {
        if colors_enabled() {
            self.style.fmt_prefix(f)?;
            fmt_inner(self.value, f)?;
            self.style.fmt_suffix(f)
        } else {
            fmt_inner(self.value, f)
        }
    }
}

macro_rules! impl_fmt {
    ($($trait:path),* $(,)?) => {
        $(
            impl<'a, T> $trait for Styled<'a, T>
            where
                T: ?Sized + $trait,
            {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.fmt_inner(f, |value, writer| <T as $trait>::fmt(value, writer))
                }
            }
        )*
    };
}

impl_fmt! {
    fmt::Display,
    fmt::Debug,
    fmt::UpperHex,
    fmt::LowerHex,
    fmt::Binary,
    fmt::UpperExp,
    fmt::LowerExp,
    fmt::Octal,
    fmt::Pointer,
}

pub trait OwoColorize {
    #[must_use]
    fn style(&self, style: Style) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).style(style)
    }

    #[must_use]
    fn black(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).black()
    }

    #[must_use]
    fn red(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).red()
    }

    #[must_use]
    fn green(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).green()
    }

    #[must_use]
    fn yellow(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).yellow()
    }

    #[must_use]
    fn blue(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).blue()
    }

    #[must_use]
    fn magenta(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).magenta()
    }

    #[must_use]
    fn purple(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).purple()
    }

    #[must_use]
    fn cyan(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).cyan()
    }

    #[must_use]
    fn white(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).white()
    }

    #[must_use]
    fn bright_black(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bright_black()
    }

    #[must_use]
    fn bright_red(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bright_red()
    }

    #[must_use]
    fn bright_green(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bright_green()
    }

    #[must_use]
    fn bright_yellow(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bright_yellow()
    }

    #[must_use]
    fn bright_blue(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bright_blue()
    }

    #[must_use]
    fn bright_magenta(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bright_magenta()
    }

    #[must_use]
    fn bright_purple(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bright_purple()
    }

    #[must_use]
    fn bright_cyan(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bright_cyan()
    }

    #[must_use]
    fn bright_white(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bright_white()
    }

    #[must_use]
    fn on_black(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).on_black()
    }

    #[must_use]
    fn on_red(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).on_red()
    }

    #[must_use]
    fn on_green(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).on_green()
    }

    #[must_use]
    fn on_yellow(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).on_yellow()
    }

    #[must_use]
    fn on_blue(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).on_blue()
    }

    #[must_use]
    fn on_magenta(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).on_magenta()
    }

    #[must_use]
    fn on_purple(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).on_purple()
    }

    #[must_use]
    fn on_cyan(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).on_cyan()
    }

    #[must_use]
    fn on_white(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).on_white()
    }

    #[must_use]
    fn bold(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).bold()
    }

    #[must_use]
    fn dimmed(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).dimmed()
    }

    #[must_use]
    fn italic(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).italic()
    }

    #[must_use]
    fn underline(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).underline()
    }

    #[must_use]
    fn strikethrough(&self) -> Styled<'_, Self>
    where
        Self: Sized,
    {
        Styled::new(self).strikethrough()
    }
}

impl<T> OwoColorize for T {}
