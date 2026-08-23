use egui::{
    Area, Color32, CursorIcon, Id, InnerResponse, Order, Panel, Pos2, Rect, Response, Sense, Ui,
    UiBuilder, Vec2, pos2, vec2,
};

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
        rect.center(),
        egui::Align2::CENTER_CENTER,
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

    let (rect, mut response) = ui.allocate_exact_size(
        galley.size() + vec2(galley.size().x * 0.1, galley.size().y),
        egui::Sense::click(),
    );

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
#[derive(Clone)]
pub struct TuiWindowState {
    pub pos: Pos2,
    pub size: Vec2,
}
pub struct TuiWindow {
    pub title: String,
    pub id: Id,
    pub state: TuiWindowState,
    pub border_type: TuiBorder,
}

impl Default for TuiWindow {
    fn default() -> Self {
        Self {
            title: Default::default(),
            id: Id::new("id"),
            state: TuiWindowState {
                pos: pos2(100.0, 200.0),
                size: vec2(800.0, 600.0),
            },
            border_type: TuiBorder::SoftLines,
        }
    }
}
impl TuiWindow {
    pub fn new(id: Id, title: &str, pos: Pos2, size: Vec2, border_type: TuiBorder) -> Self {
        Self {
            id: Id::new(id),
            title: title.to_string(),
            state: TuiWindowState { pos, size },
            border_type,
        }
    }

    pub fn show<R>(&mut self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
        let mut state = ui.data_mut(|d| {
            d.get_persisted::<TuiWindowState>(self.id)
                .unwrap_or(self.state.clone())
        });

        let inner = Area::new(self.id)
            .fixed_pos(state.pos)
            .order(Order::Foreground)
            .show(ui.ctx(), |ui| {
                ui.set_min_size(state.size);
                ui.set_max_size(state.size);

                let (inner, outer, _) = draw_tui_border(ui, Some(self.title.clone()), 1.0);

                const EDGE: f32 = 8.0;
                const HALF_EDGE: f32 = EDGE * 0.5;
                const CORNER: f32 = 16.0;
                const TITLE: f32 = 20.0;
                const MIN_SIZE: f32 = 64.0;

                let right = Rect::from_min_max(
                    pos2(outer.right() - HALF_EDGE, outer.top() + TITLE),
                    pos2(outer.right() + HALF_EDGE, outer.bottom() - CORNER),
                );

                let top = Rect::from_min_max(
                    pos2(outer.left() + CORNER, outer.top() - HALF_EDGE),
                    pos2(outer.right() - CORNER, outer.top() + HALF_EDGE),
                );

                let bottom = Rect::from_min_max(
                    pos2(outer.left() + CORNER, outer.bottom() - HALF_EDGE),
                    pos2(outer.right() - CORNER, outer.bottom() + HALF_EDGE),
                );

                let bottom_right = Rect::from_min_max(
                    pos2(outer.right() - CORNER, outer.bottom() - CORNER),
                    pos2(outer.right() + HALF_EDGE, outer.bottom() + HALF_EDGE),
                );

                let right = ui
                    .interact(right, ui.id().with("right"), Sense::drag())
                    .on_hover_cursor(CursorIcon::ResizeHorizontal);

                let top = ui
                    .interact(top, ui.id().with("top"), Sense::drag())
                    .on_hover_cursor(CursorIcon::Move);

                let bottom = ui
                    .interact(bottom, ui.id().with("bottom"), Sense::drag())
                    .on_hover_cursor(CursorIcon::ResizeVertical);

                let bottom_right = ui
                    .interact(bottom_right, ui.id().with("bottom_right"), Sense::drag())
                    .on_hover_cursor(CursorIcon::ResizeSouthEast);
                if right.dragged() {
                    state.size.x = (state.size.x + right.drag_delta().x).max(MIN_SIZE);
                }

                if bottom.dragged() {
                    state.size.y = (state.size.y + bottom.drag_delta().y).max(MIN_SIZE);
                }

                if bottom_right.dragged() {
                    state.size.x = (state.size.x + bottom_right.drag_delta().x).max(MIN_SIZE);
                    state.size.y = (state.size.y + bottom_right.drag_delta().y).max(MIN_SIZE);
                }

                if top.dragged() {
                    state.pos += top.drag_delta();
                }

                let mut child = ui.new_child(UiBuilder::new().max_rect(inner));
                child.set_clip_rect(inner);

                add_contents(&mut child)
            })
            .inner;

        ui.data_mut(|d| {
            d.insert_persisted(self.id, state);
        });

        inner
    }
}

pub struct TuiPanel {
    panel: Panel,
    _border_type: TuiBorder,
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
        self.panel = self.panel.min_size(height * (cols + 3) as f32);
        self
    }

    fn new(panel: Panel, _border_type: TuiBorder) -> Self {
        TuiPanel {
            panel,
            _border_type,
        }
    }

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        self.panel.show(ui, |ui| {
            let (inner, _, _) = draw_tui_border(ui, None, 0.0);
            // painter.rect_filled(inner, 0.0, Color32::LIGHT_GRAY);

            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));

            add_contents(&mut child)
        })
    }
}

fn draw_tui_border(ui: &mut Ui, title: Option<String>, top_margin: f32) -> (Rect, Rect, Response) {
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

    let left_margin: usize = 4;

    let background = egui::Rect::from_min_max(
        egui::pos2(rect.left() + cell_w / 2.0, rect.top() + cell_w),
        egui::pos2(
            rect.right() - (cell_w * cols_mod) - (cell_w / 2.0),
            rect.bottom() - (cell_h * rows_mod) - cell_w,
        ),
    );

    painter.rect_filled(background, 0.0, Color32::BLACK);
    let prefix = if let Some(title) = title {
        let title = format!(" {} ", title);
        format!("┌{}{}", "─".repeat(left_margin.saturating_sub(2)), title)
    } else {
        format!("┌{}", "─".repeat(left_margin.saturating_sub(2)))
    };

    let fill = cols.saturating_sub(prefix.chars().count() + 1);

    let top = format!("{}{}┐", prefix, "─".repeat(fill));

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

    let inner = egui::Rect::from_min_max(
        egui::pos2(
            rect.left() + cell_w * 2.0,
            rect.top() + cell_w * (2.0 + top_margin),
        ),
        egui::pos2(
            rect.right() - (cell_w * cols_mod) - cell_w * 2.0,
            rect.bottom() - (cell_h * rows_mod) - cell_w * 2.0,
        ),
    );
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    (inner, background, response)
}
