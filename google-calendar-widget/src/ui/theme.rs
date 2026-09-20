use iced::Color;

#[derive(Debug, Clone, Copy)]
pub struct AppTheme {
    pub is_dark: bool,
    pub bg: Color,
    pub surface_alt: Color,
    pub border_light: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_dim: Color,
    pub accent: Color,
    pub today_bg: Color,
    pub today_border: Color,
    pub cell_bg: Color,
    pub cell_bg_weekend: Color,
    pub form_bg: Color,
    pub form_border: Color,
}

impl AppTheme {
    pub fn light() -> Self {
        Self {
            is_dark: false,
            bg: Color::from_rgb(0.94, 0.96, 0.99),
            surface_alt: Color::from_rgba(0.90, 0.93, 0.97, 0.6),
            border_light: Color::from_rgba(0.75, 0.80, 0.88, 0.45),
            text: Color::from_rgb(0.10, 0.14, 0.20),
            text_muted: Color::from_rgb(0.40, 0.45, 0.52),
            text_dim: Color::from_rgb(0.55, 0.60, 0.68),
            accent: Color::from_rgb(0.28, 0.50, 0.78),
            today_bg: Color::from_rgba(0.75, 0.85, 0.98, 0.6),
            today_border: Color::from_rgba(0.28, 0.50, 0.78, 0.75),
            cell_bg: Color::from_rgba(1.0, 1.0, 1.0, 0.6),
            cell_bg_weekend: Color::from_rgba(0.92, 0.95, 0.99, 0.5),
            form_bg: Color::from_rgba(0.94, 0.96, 1.0, 0.95),
            form_border: Color::from_rgb(0.65, 0.75, 0.88),
        }
    }

    pub fn dark() -> Self {
        Self {
            is_dark: true,
            bg: Color::from_rgb(0.03, 0.03, 0.04),
            surface_alt: Color::from_rgba(0.12, 0.12, 0.15, 0.6),
            border_light: Color::from_rgba(0.20, 0.20, 0.26, 0.55),
            text: Color::from_rgb(0.92, 0.92, 0.94),
            text_muted: Color::from_rgb(0.62, 0.62, 0.68),
            text_dim: Color::from_rgb(0.45, 0.45, 0.52),
            accent: Color::from_rgb(0.42, 0.65, 0.92),
            today_bg: Color::from_rgba(0.20, 0.35, 0.58, 0.5),
            today_border: Color::from_rgba(0.42, 0.65, 0.92, 0.85),
            cell_bg: Color::from_rgba(0.10, 0.10, 0.13, 0.55),
            cell_bg_weekend: Color::from_rgba(0.07, 0.07, 0.10, 0.55),
            form_bg: Color::from_rgba(0.12, 0.12, 0.16, 0.95),
            form_border: Color::from_rgb(0.30, 0.40, 0.55),
        }
    }

}