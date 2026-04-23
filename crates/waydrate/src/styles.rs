use clap::builder::styling::AnsiColor;
use clap::builder::styling::Style;

pub(crate) fn bold_green() -> Style {
    Style::new().bold().fg_color(Some(AnsiColor::Green.into()))
}
