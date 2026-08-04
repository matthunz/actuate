use super::{Theme, TypographyKind, TypographyStyleKind};
use crate::{
    ecs::{Modifier, Modify, spawn},
    prelude::Compose,
    use_context,
};
use actuate_macros::Data;
use bevy_text::{FontSize, TextColor, TextFont};
use bevy_ui::prelude::Text as UiText;

/// Create a material UI text body.
pub fn body<'a>(content: impl Into<String>) -> Text<'a> {
    text(content).typography(TypographyKind::Body)
}

/// Create a material UI text headline.
pub fn headline<'a>(content: impl Into<String>) -> Text<'a> {
    text(content).typography(TypographyKind::Headline)
}

/// Create a material UI text label.
pub fn label<'a>(content: impl Into<String>) -> Text<'a> {
    text(content).typography(TypographyKind::Label)
}

/// Create a material UI text title.
pub fn title<'a>(content: impl Into<String>) -> Text<'a> {
    text(content).typography(TypographyKind::Title)
}

/// Create a material UI text label.
pub fn text<'a>(content: impl Into<String>) -> Text<'a> {
    Text {
        content: content.into(),
        modifier: Modifier::default(),
        typography: TypographyKind::Label,
        typography_style: TypographyStyleKind::Medium,
    }
}

/// Material UI text composable.
#[derive(Data)]
#[actuate(path = "crate")]
pub struct Text<'a> {
    content: String,
    typography: TypographyKind,
    typography_style: TypographyStyleKind,
    modifier: Modifier<'a>,
}

impl Text<'_> {
    /// Set the typography of this text.
    pub fn typography(mut self, typography: TypographyKind) -> Self {
        self.typography = typography;
        self
    }

    /// Set the typography style of this text.
    pub fn typography_style(mut self, typography_style: TypographyStyleKind) -> Self {
        self.typography_style = typography_style;
        self
    }
}

impl Compose for Text<'_> {
    fn compose(cx: crate::Scope<Self>) -> impl Compose {
        let theme = use_context::<Theme>(&cx).cloned().unwrap_or_default();

        let style = &theme.typography[cx.me().typography][cx.me().typography_style];

        spawn((
            UiText::new(cx.me().content.clone()),
            TextColor(theme.colors.text),
            TextFont {
                font_size: FontSize::Px(style.font_size),
                ..Default::default()
            },
        ))
    }
}

impl<'a> Modify<'a> for Text<'a> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
