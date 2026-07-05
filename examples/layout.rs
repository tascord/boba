//! Layout demo roughly equivalent to the `charmbracelet/lipgloss`
//! `examples/layout/main.go` showcase.
//!
//! Run with: `cargo run --example layout`

use {
    boba::{
        App, AppEvent, View,
        components::{
            border::Border,
            button::{Button, ButtonVariant},
            layer::{Compositor, CompositorLayer},
            style::{BobaStyle, blend_1d, blend_2d, has_dark_background, hex_color, light_dark},
            tabs::Tabs,
        },
        events::{EventTarget, SubscriptionPriority},
        surface::{Cell, Position, Surface, clip, fit_width, join_horizontal, join_vertical, place_filled},
        theme::Theme,
    },
    crossterm::event::{Event as CrosstermEvent, KeyCode, MouseEvent},
    futures_signals::signal::Mutable,
    ratatui::{
        Frame,
        layout::Alignment,
        prelude::Rect,
        style::{Color, Style},
    },
    std::sync::Mutex,
};

const WIDTH: usize = 96;
const COLUMN_WIDTH: usize = 28;

fn color_grid_surf(x_steps: usize, y_steps: usize) -> Surface {
    let colors =
        blend_2d(x_steps, y_steps, hex_color("#F25D94"), hex_color("#EDFF82"), hex_color("#643AFF"), hex_color("#14F9D5"));
    let mut rows = Vec::new();
    for row in colors {
        let mut cells = Vec::new();
        for color in row {
            let style = Style::default().bg(color);
            cells.push(Cell::new(" ", style));
            cells.push(Cell::new(" ", style));
        }
        rows.push(cells);
    }
    Surface { rows }
}

fn apply_gradient(text: &str, from: Color, to: Color) -> Surface {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Surface { rows: vec![vec![]] };
    }
    let n = chars.len().max(2);
    let gradients = blend_1d(n, from, to);
    let mut row = Vec::new();
    for (i, ch) in chars.iter().enumerate() {
        let style = BobaStyle::new().fg(gradients[i]).into();
        row.push(Cell::new(ch.to_string(), style));
    }
    Surface { rows: vec![row] }
}

// ─── Style Helpers ───────────────────────────────────────────────────────────

fn status_seg(text: &str, bg: Color) -> BobaStyle {
    BobaStyle::new().fg(hex_color("#FFFDF5")).bg(bg).padding_y(0).padding_x(1)
}

struct LayoutView {
    tabs: Tabs,
    ok_btn: Button,
    cancel_btn: Button,
    active_tab: Mutable<usize>,
    tabs_area: Mutex<Rect>,
    ok_btn_area: Mutex<Rect>,
    cancel_btn_area: Mutex<Rect>,
}

impl LayoutView {
    fn new() -> Self {
        let active_tab = Mutable::new(0);
        Self {
            tabs: Tabs::new(["Lip Gloss", "Blush", "Eye Shadow", "Mascara", "Foundation"])
                .id_as("main-tabs")
                .active(&active_tab),
            ok_btn: Button::new("Yes")
                .id_as("dialog-ok")
                .variant(ButtonVariant::Primary)
                .default_style(
                    BobaStyle::new()
                        .fg(hex_color("#FFF7DB"))
                        .bg(hex_color("#F25D94"))
                        .padding_y(0)
                        .padding_x(1)
                        .margin_top(1)
                        .margin_right(2),
                )
                .hovered_style(
                    BobaStyle::new()
                        .fg(hex_color("#FFF7DB"))
                        .bg(hex_color("#F25D94"))
                        .padding_y(0)
                        .padding_x(1)
                        .margin_top(1)
                        .margin_right(2)
                        .bold(),
                ),
            cancel_btn: Button::new("Maybe").id_as("dialog-cancel").variant(ButtonVariant::Secondary).default_style(
                BobaStyle::new().fg(hex_color("#FFF7DB")).bg(hex_color("#888B7E")).padding_y(0).padding_x(1).margin_top(1),
            ),
            active_tab,
            tabs_area: Mutex::new(Rect::new(0, 0, 0, 0)),
            ok_btn_area: Mutex::new(Rect::new(0, 0, 0, 0)),
            cancel_btn_area: Mutex::new(Rect::new(0, 0, 0, 0)),
        }
    }
}

impl LayoutView {
    fn build_document(&self) -> Surface {
        let has_dark_bg = has_dark_background();

        let subtle = light_dark(has_dark_bg, hex_color("#D9DCCF"), hex_color("#383838"));
        let highlight = light_dark(has_dark_bg, hex_color("#874BFD"), hex_color("#7D56F4"));
        let special = light_dark(has_dark_bg, hex_color("#43BF6D"), hex_color("#73F59F"));

        // ── Tabs ──
        let tabs_row = self.tabs.render_to_surface();

        // ── Title ──
        let title_style =
            BobaStyle::new().margin_left(1).margin_right(5).padding(0, 1, 0, 1).italic().fg(hex_color("#FFF7DB"));
        let title_colors =
            blend_2d(1, 5, hex_color("#F25D94"), hex_color("#643AFF"), hex_color("#EDFF82"), hex_color("#14F9D5"));
        let mut title_surfaces: Vec<Surface> = Vec::new();
        for (i, row) in title_colors.iter().enumerate() {
            let color = row[0];
            let s = title_style.margin_left((i * 2) as u16).bg(color).render("Boba Tea");
            title_surfaces.push(s);
        }
        let title = join_vertical(Position::Left, &title_surfaces);

        let desc_style = BobaStyle::new().margin_top(1);
        let info_style = BobaStyle::new().border(Border::normal().no_left().no_right().no_bottom()).border_fg(subtle);
        let divider = BobaStyle::new().padding(0, 1, 0, 1).fg(subtle).render("•");
        let url = BobaStyle::new().fg(special).render("https://github.com/tascord/boba");
        // Join the text parts first, then wrap with the top border (matches lipgloss behaviour).
        let info_content =
            join_horizontal(Position::Top, &[BobaStyle::new().render("by flora (based on Charm)"), divider, url]);
        let info = info_style.render_surface(&info_content);
        let desc = join_vertical(Position::Left, &[desc_style.render("TUI Management you wont hate"), info]);
        let title_row = join_horizontal(Position::Top, &[title, desc]);

        // ── Dialog ──
        let dialog_box = BobaStyle::new()
            .border(Border::rounded())
            .border_fg(hex_color("#874BFD"))
            .padding(1, 0, 1, 0)
            .width(WIDTH as u16);

        let question = BobaStyle::new().width(50).align(Alignment::Center, Position::Center).render_surface(
            &apply_gradient("Are you sure you want to eat marmalade?", hex_color("#EDFF82"), hex_color("#F25D94")),
        );
        let ok_surf = self.ok_btn.render_to_surface(&Theme::default());
        let cancel_surf = self.cancel_btn.render_to_surface(&Theme::default());
        let buttons = join_horizontal(Position::Top, &[ok_surf, cancel_surf]);
        let dialog_inner = dialog_box.render_surface(&join_vertical(Position::Center, &[question, buttons]));
        let dialog = place_filled(
            WIDTH,
            9,
            Position::Center,
            Position::Center,
            &dialog_inner,
            Style::default().fg(subtle),
            "l o r e m ",
        );
        let mut dialog = dialog;
        fit_width(&mut dialog, WIDTH, &Cell::blank(Style::default().fg(subtle)));

        // ── Color grid ──
        let colors = color_grid_surf(14, 8);

        // ── Lists ──
        // Header: only bottom border (matching lipgloss listHeader).
        let list_header_style =
            BobaStyle::new().border(Border::normal().no_top().no_left().no_right()).border_fg(subtle).margin_right(2);
        // List: only left border (matching lipgloss list).
        let list_style = BobaStyle::new()
            .border(Border::normal().no_top().no_bottom().no_right())
            .border_fg(subtle)
            .margin_right(1)
            .height(8)
            .width((WIDTH / 3) as u16);

        let check = BobaStyle::new().fg(special).padding_right(1).render("✓");
        let gray_done = light_dark(has_dark_bg, hex_color("#969B86"), hex_color("#696969"));
        let list_done = |text: &str| -> Surface {
            let body = BobaStyle::new().crossed_out().fg(gray_done).render(text);
            join_horizontal(Position::Top, &[check.clone(), body])
        };
        let list_item = |text: &str| -> Surface { BobaStyle::new().padding_left(2).render(text) };

        let list1 = join_vertical(Position::Left, &[
            list_header_style.render("Citrus Fruits to Try"),
            list_done("Grapefruit"),
            list_done("Yuzu"),
            list_item("Citron"),
            list_item("Kumquat"),
            list_item("Pomelo"),
        ]);
        let list1 = list_style.render_surface(&list1);

        let list2 = join_vertical(Position::Left, &[
            list_header_style.render("Actual Boba Vendors"),
            list_item("Chatime"),
            list_item("Gong Cha"),
            list_done("Teaser"),
        ]);
        let list2 = list_style.render_surface(&list2);

        let lists = join_horizontal(Position::Top, &[list1, list2, BobaStyle::new().margin_left(1).render_surface(&colors)]);

        // ── History ──
        let history_style = BobaStyle::new()
            .align(Alignment::Left, Position::Top)
            .fg(hex_color("#FAFAFA"))
            .bg(highlight)
            .margin(1, 3, 0, 0)
            .padding(1, 2, 1, 2)
            .height(19)
            .width(COLUMN_WIDTH as u16);

        let history_a = history_style.clone().align(Alignment::Right, Position::Top).render(
            "Lorem ipsum dolor sit amet consectetur adipiscing elit. Quisque faucibus ex sapien vitae pellentesque sem \
             placerat. In id cursus mi pretium tellus duis convallis. Tempus leo eu aenean sed diam urna tempor. Pulvinar \
             vivamus fringilla lacus nec metus bibendum egestas. Iaculis massa nisl malesuada lacinia integer nunc \
             posuere. Ut hendrerit semper vel class aptent taciti sociosqu. Ad litora torquent per conubia nostra inceptos \
             himenaeos.",
        );
        let history_b = history_style.clone().align(Alignment::Center, Position::Top).render(
            "Lorem ipsum dolor sit amet consectetur adipiscing elit. Quisque faucibus ex sapien vitae pellentesque sem \
             placerat. In id cursus mi pretium tellus duis convallis. Tempus leo eu aenean sed diam urna tempor. Pulvinar \
             vivamus fringilla lacus nec metus bibendum egestas. Iaculis massa nisl malesuada lacinia integer nunc \
             posuere. Ut hendrerit semper vel class aptent taciti sociosqu. Ad litora torquent per conubia nostra inceptos \
             himenaeos.",
        );
        let history_c = history_style.align(Alignment::Left, Position::Top).margin_right(0).render(
            "Lorem ipsum dolor sit amet consectetur adipiscing elit. Quisque faucibus ex sapien vitae pellentesque sem \
             placerat. In id cursus mi pretium tellus duis convallis. Tempus leo eu aenean sed diam urna tempor. Pulvinar \
             vivamus fringilla lacus nec metus bibendum egestas. Iaculis massa nisl malesuada lacinia integer nunc \
             posuere. Ut hendrerit semper vel class aptent taciti sociosqu. Ad litora torquent per conubia nostra inceptos \
             himenaeos.",
        );
        let history = join_horizontal(Position::Top, &[history_a, history_b, history_c]);

        // ── Status bar ──
        let light_dark_state = if has_dark_bg { "Dark" } else { "Light" };
        let bar_style = BobaStyle::new()
            .fg(light_dark(has_dark_bg, hex_color("#343433"), hex_color("#C1C6B2")))
            .bg(light_dark(has_dark_bg, hex_color("#D9DCCF"), hex_color("#353533")));

        let status_key = status_seg("STATUS", hex_color("#FF5F87")).render("STATUS");
        let encoding = status_seg("UTF-8", hex_color("#A550DF")).align(Alignment::Right, Position::Center).render("UTF-8");
        let fish = status_seg("🍥 Fish Cake", hex_color("#6124DF")).render("🍥 Fish Cake");
        let used = status_key.width() + encoding.width() + fish.width();
        let remaining = WIDTH.saturating_sub(used);
        let mut status_val = BobaStyle::new()
            .inherit(bar_style)
            .width(remaining as u16)
            .padding_x(1)
            .render(&format!("Ravishingly {}!", light_dark_state));
        clip(&mut status_val, remaining);
        fit_width(&mut status_val, remaining, &Cell::blank(*bar_style));

        let bar = join_horizontal(Position::Top, &[status_key, status_val, encoding, fish]);
        // Apply status bar background/foreground across the full width.
        let status_bar = BobaStyle::new().inherit(bar_style).width(WIDTH as u16).render_surface(&bar);

        // ── Assemble document ──
        let doc = join_vertical(Position::Left, &[tabs_row, title_row, dialog, lists, history, status_bar]);

        doc
    }

    fn build_modal(&self) -> Surface {
        BobaStyle::new()
            .italic()
            .fg(hex_color("#FFF7DB"))
            .bg(hex_color("#F25D94"))
            .padding(1, 6, 1, 6)
            .width(WIDTH as u16)
            .align(Alignment::Center, Position::Center)
            .render("Now with Compositing!")
    }

    fn on_mouse(&self, ev: &MouseEvent) {
        if let Ok(tabs_area) = self.tabs_area.lock() {
            self.tabs.on_mouse(*tabs_area, ev);
        }
        if let Ok(ok_btn_area) = self.ok_btn_area.lock() {
            self.ok_btn.on_mouse(*ok_btn_area, ev);
        }
        if let Ok(cancel_btn_area) = self.cancel_btn_area.lock() {
            self.cancel_btn.on_mouse(*cancel_btn_area, ev);
        }
    }
}

impl View for LayoutView {
    fn mount(&self, app: &EventTarget<AppEvent>) {
        let app_for_quit = app.clone();
        app.on_key(SubscriptionPriority::High, move |ev, key| {
            if key.code == KeyCode::Char('q') {
                ev.cancel();
                app_for_quit.emit(AppEvent::Quit);
            }
        })
        .forget();
    }

    fn render(&self, ctx: &mut Frame<'_>, theme: &Theme) {
        let area = ctx.area();
        let buf = ctx.buffer_mut();

        // Clear with theme background
        let bg = theme.global_bg;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_bg(bg);
            }
        }

        let doc = self.build_document();
        let modal = self.build_modal();

        // ── Debug: print dimensions ──────────────────────────────────────────
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "DOC  visual_w={} cell_cols={} h={}", doc.width(), doc.columns(), doc.height());
        let _ = writeln!(std::io::stderr(), "MODAL visual_w={} h={}", modal.width(), modal.height());

        // ── Debug border around the document ─────────────────────────────────
        let doc_with_border = BobaStyle::new().border(Border::thick()).border_fg(hex_color("#FF0000")).render_surface(&doc);

        // Center the document in the available terminal area
        let doc_w = doc_with_border.width().min(area.width as usize);
        let doc_h = doc_with_border.height().min(area.height as usize);
        let doc_x = ((area.width as usize).saturating_sub(doc_w)) / 2;
        let doc_y = ((area.height as usize).saturating_sub(doc_h)) / 2;

        // Update component areas based on document layout
        // Document structure: tabs(3) + title(5) + dialog(9) + lists(8) + history(~19) + status(~1)
        // Tabs area: top of document
        if let Ok(mut tabs_area) = self.tabs_area.lock() {
            *tabs_area = Rect::new(doc_x as u16, doc_y as u16, doc_w as u16, 3);
        }
        // Dialog is centered at position (doc_x, doc_y + 8), with 96 width, 9 height
        let dialog_x = doc_x;
        let dialog_y = doc_y + 8;
        // Dialog inner: question (1 row) + buttons (2 rows with margin_top(1))
        // Buttons are at dialog inner bottom, after question
        // dialog_box has padding(1, 0, 1, 0) = top:1, bottom:0, left:0, right:0
        let dialog_inner_top = dialog_y + 1; // after dialog_box top padding
        let buttons_y = dialog_inner_top + 1; // after question (1 row)
        let ok_btn_width = self.ok_btn.render_to_surface(theme).columns();
        let cancel_btn_width = self.cancel_btn.render_to_surface(theme).columns();
        let buttons_width = ok_btn_width + cancel_btn_width + 2; // +2 for margin_right on ok_btn
        let buttons_start_x = dialog_x + (96usize.saturating_sub(buttons_width)) / 2;
        if let Ok(mut ok_btn_area) = self.ok_btn_area.lock() {
            *ok_btn_area = Rect::new(buttons_start_x as u16, buttons_y as u16, ok_btn_width as u16, 1);
        }
        if let Ok(mut cancel_btn_area) = self.cancel_btn_area.lock() {
            *cancel_btn_area =
                Rect::new((buttons_start_x + ok_btn_width + 2) as u16, buttons_y as u16, cancel_btn_width as u16, 1);
        }

        // Modal positioned relative to the document center (lipgloss example offsets)
        let modal_x = doc_x + 58;
        let modal_y = doc_y + 44;

        let comp = Compositor::new(vec![
            CompositorLayer::new(doc_with_border).x(doc_x as u16).y(doc_y as u16),
            CompositorLayer::new(modal).x(modal_x as u16).y(modal_y as u16),
        ]);
        comp.render_to_buf(area, buf);
    }
}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {
        App::new(LayoutView::new()).run().await.unwrap();
    });
}
