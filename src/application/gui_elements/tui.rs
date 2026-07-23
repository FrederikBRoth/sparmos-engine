use egui::{Color32, InnerResponse, Panel, Ui};

pub fn tui_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let display = format!("  {}", text);

    let galley = ui.painter().layout_no_wrap(
        display.clone(),
        ui.style().text_styles[&egui::TextStyle::Button].clone(),
        ui.visuals().widgets.inactive.fg_stroke.color,
    );

    let (rect, response) =
        ui.allocate_exact_size(galley.size() + egui::vec2(8.0, 4.0), egui::Sense::click());

    let prefix = if response.is_pointer_button_down_on() {
        ">"
    } else if response.hovered() {
        ">"
    } else {
        " "
    };

    let color = if response.hovered() {
        ui.visuals().widgets.hovered.fg_stroke.color
    } else {
        ui.visuals().widgets.inactive.fg_stroke.color
    };
    let text = format!("{} {}", prefix, text);

    ui.painter().text(
        rect.left_top(),
        egui::Align2::LEFT_TOP,
        text,
        ui.style().text_styles[&egui::TextStyle::Button].clone(),
        color,
    );

    response
}

pub fn toggleable_tui_button(ui: &mut egui::Ui, state: &mut bool, text: &str) -> egui::Response {
    let display = format!("  {}", text);

    let galley = ui.painter().layout_no_wrap(
        display.clone(),
        ui.style().text_styles[&egui::TextStyle::Button].clone(),
        ui.visuals().widgets.inactive.fg_stroke.color,
    );

    let (rect, mut response) =
        ui.allocate_exact_size(galley.size() + egui::vec2(8.0, 4.0), egui::Sense::click());

    if response.clicked() {
        *state = !*state;
        response.mark_changed();
    }
    let text = if *state {
        format!("[ {} ]", text)
    } else {
        text.to_string()
    };

    let color = if response.hovered() {
        ui.visuals().widgets.hovered.fg_stroke.color
    } else {
        ui.visuals().widgets.inactive.fg_stroke.color
    };

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        ui.style().text_styles[&egui::TextStyle::Button].clone(),
        color,
    );

    response
}

pub enum TuiBorder {
    HardLines,
    SoftLines,
}

pub struct TuiPanel {
    panel: Panel,
    border_type: TuiBorder,
}

impl TuiPanel {
    pub fn top(border_type: TuiBorder) -> Self {
        Self::new(
            egui::Panel::top("top_panel")
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                ),
            border_type,
        )
    }
    pub fn bottom(border_type: TuiBorder) -> Self {
        Self::new(
            egui::Panel::bottom("bottom_panel")
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                ),
            border_type,
        )
    }

    pub fn right(border_type: TuiBorder) -> Self {
        Self::new(
            egui::Panel::right("right_panel")
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                ),
            border_type,
        )
    }
    pub fn left(border_type: TuiBorder) -> Self {
        Self::new(
            egui::Panel::left("left_panel")
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                ),
            border_type,
        )
    }

    pub fn size(mut self, ui: &egui::Ui, cols: u32) -> Self {
        let (_, height) = ui.fonts_mut(|fonts| {
            let font_id = egui::TextStyle::Monospace.resolve(ui.style());
            let galley = fonts.layout_no_wrap("F".into(), font_id, ui.visuals().text_color());

            (galley.size().x, galley.size().y)
        });
        self.panel = self.panel.min_size((height * (cols + 3) as f32));
        self
    }

    fn new(panel: Panel, border_type: TuiBorder) -> Self {
        TuiPanel { panel, border_type }
    }

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        self.panel.show(ui, |ui| {
            let painter = ui.painter();

            let font = ui.style().text_styles[&egui::TextStyle::Body].clone();

            let color = egui::Color32::LIGHT_GRAY;

            let rect = ui.max_rect();

            let galley = ui.painter().layout_no_wrap("─".into(), font.clone(), color);

            let cell_w = galley.size().x;
            let cell_h = galley.size().y;

            let cols_float = rect.width() / cell_w;
            let rows_float = rect.height() / cell_h;
            let cols_mod = cols_float % 1.0;
            let rows_mod = rows_float % 1.0;
            let cols = cols_float.floor() as usize;
            let rows = rows_float.floor() as usize;

            let top = format!("┌{}┐", "─".repeat(cols.saturating_sub(2)));

            painter.text(
                rect.left_top(),
                egui::Align2::LEFT_TOP,
                top,
                font.clone(),
                color,
            );

            for y in 1..rows.saturating_sub(1) {
                let line = format!("│{}│", " ".repeat(cols.saturating_sub(2)));

                painter.text(
                    egui::pos2(rect.left(), rect.top() + y as f32 * cell_h),
                    egui::Align2::LEFT_TOP,
                    line,
                    font.clone(),
                    color,
                );
            }

            let bottom = format!("└{}┘", "─".repeat(cols.saturating_sub(2)));
            painter.text(
                egui::pos2(
                    rect.left(),
                    rect.top() + (rows.saturating_sub(1)) as f32 * cell_h,
                ),
                egui::Align2::LEFT_TOP,
                bottom,
                font.clone(),
                color,
            );

            let background = egui::Rect::from_min_max(
                egui::pos2(rect.left() + cell_w / 2.0, rect.top() + cell_w),
                egui::pos2(
                    rect.right() - (cell_w * cols_mod) - (cell_w / 2.0),
                    rect.bottom() - (cell_h * rows_mod) - cell_w,
                ),
            );

            painter.rect_filled(background, 0.0, Color32::BLACK);

            let inner = egui::Rect::from_min_max(
                egui::pos2(rect.left() + cell_w * 2.0, rect.top() + cell_w * 2.0),
                egui::pos2(
                    rect.right() - (cell_w * cols_mod) - cell_w * 2.0,
                    rect.bottom() - (cell_h * rows_mod) - cell_w * 2.0,
                ),
            );

            // painter.rect_filled(inner, 0.0, Color32::LIGHT_GRAY);

            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));

            add_contents(&mut child)
        })
    }
}
