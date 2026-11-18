//! Single-line GPUI input with platform IME and a steady (non-animated) caret.
use std::ops::Range;

use gpui::{
    canvas, div, fill, point, prelude::*, px, rgb, size, App, Bounds, ClipboardItem, Context,
    ElementInputHandler, EntityInputHandler, FocusHandle, IntoElement, KeyDownEvent, MouseButton,
    Pixels, Point, ShapedLine, SharedString, TextRun, UTF16Selection, Window,
};
use unicode_segmentation::UnicodeSegmentation as _;

pub(crate) struct Field {
    text: String,
    placeholder: &'static str,
    focus: FocusHandle,
    anchor: usize,
    caret: usize,
    marked: Option<Range<usize>>,
    shaped: Option<(Point<Pixels>, ShapedLine)>,
    limit: usize,
}

impl Field {
    pub(crate) fn new(placeholder: &'static str, limit: usize, cx: &App) -> Self {
        Self {
            text: String::new(),
            placeholder,
            focus: cx.focus_handle().tab_stop(true),
            anchor: 0,
            caret: 0,
            marked: None,
            shaped: None,
            limit,
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn set(&mut self, text: String, cx: &mut Context<Self>) {
        if self.text != text {
            self.text = text;
            self.caret = self.text.len();
            self.anchor = self.caret;
            self.marked = None;
            cx.notify();
        }
    }

    pub(crate) fn composing(&self) -> bool {
        self.marked.is_some()
    }

    fn selected(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    fn boundary(&self, right: bool) -> usize {
        let mut positions = self.text.grapheme_indices(true).map(|(index, _)| index);
        if right {
            positions
                .find(|index| *index > self.caret)
                .unwrap_or(self.text.len())
        } else {
            positions
                .rev()
                .find(|index| *index < self.caret)
                .unwrap_or(0)
        }
    }

    fn move_caret(&mut self, position: usize, extend: bool) {
        self.caret = position;
        if !extend {
            self.anchor = position;
        }
    }

    fn replace(
        &mut self,
        selection: Range<usize>,
        text: &str,
        cx: &mut Context<Self>,
    ) -> Range<usize> {
        let spare = self.limit.saturating_sub(
            self.text.chars().count() - self.text[selection.clone()].chars().count(),
        );
        let insertion: String = text
            .chars()
            .filter(|ch| !ch.is_control())
            .take(spare)
            .collect();
        let begin = selection.start;
        self.text.replace_range(selection, &insertion);
        self.move_caret(begin + insertion.len(), false);
        self.marked = None;
        cx.notify();
        begin..self.caret
    }

    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let selected = self.selected();
        match (key.modifiers.control, key.key.as_str()) {
            (true, "a") => {
                self.anchor = 0;
                self.caret = self.text.len();
            }
            (true, "c" | "x") if !selected.is_empty() => {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    self.text[selected.clone()].to_owned(),
                ));
                if key.key == "x" {
                    self.replace(selected, "", cx);
                }
            }
            (true, "v") => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.replace(selected, &text, cx);
                }
            }
            (false, "home" | "end") => {
                self.move_caret(
                    if key.key == "home" {
                        0
                    } else {
                        self.text.len()
                    },
                    key.modifiers.shift,
                );
            }
            (false, "left" | "right") => {
                let right = key.key == "right";
                let position = if key.modifiers.shift || selected.is_empty() {
                    self.boundary(right)
                } else if right {
                    selected.end
                } else {
                    selected.start
                };
                self.move_caret(position, key.modifiers.shift);
            }
            (false, "backspace" | "delete") => {
                let range = if !selected.is_empty() {
                    selected
                } else if key.key == "backspace" {
                    self.boundary(false)..self.caret
                } else {
                    self.caret..self.boundary(true)
                };
                self.replace(range, "", cx);
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn byte_offset(text: &str, units: usize) -> usize {
        let mut remaining = units;
        for (index, ch) in text.char_indices() {
            if remaining < ch.len_utf16() {
                return index;
            }
            remaining -= ch.len_utf16();
        }
        text.len()
    }

    fn utf16(&self, range: Range<usize>) -> Range<usize> {
        let prefix = self.text[..range.start].encode_utf16().count();
        prefix..prefix + self.text[range].encode_utf16().count()
    }

    fn bytes(&self, range: Range<usize>) -> Range<usize> {
        Self::byte_offset(&self.text, range.start)
            ..Self::byte_offset(&self.text, range.end.max(range.start))
    }

    fn target(&self, range: Option<Range<usize>>) -> Range<usize> {
        range
            .map(|range| self.bytes(range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selected())
    }
}

impl EntityInputHandler for Field {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let bytes = self.bytes(range);
        *actual = Some(self.utf16(bytes.clone()));
        Some(self.text[bytes].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16(self.selected()),
            reversed: self.caret < self.anchor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone().map(|range| self.utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(self.target(range), text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let inserted = self.replace(self.target(range), text, cx);
        if let Some(range) = selected {
            let text = &self.text[inserted.clone()];
            self.anchor = inserted.start + Self::byte_offset(text, range.start);
            self.caret = inserted.start + Self::byte_offset(text, range.end);
        }
        self.marked = (!inserted.is_empty()).then_some(inserted);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (origin, line) = self.shaped.as_ref()?;
        let range = self.bytes(range);
        if line.text.as_ref() != self.text {
            return Some(bounds);
        }
        Some(Bounds::from_corners(
            point(origin.x + line.x_for_index(range.start), bounds.top()),
            point(origin.x + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let (origin, line) = self.shaped.as_ref()?;
        let offset = line.index_for_x(point.x - origin.x)?;
        self.text
            .get(..offset)
            .map(|text| text.encode_utf16().count())
    }
}

impl Render for Field {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let focus = self.focus.clone();
        let text: SharedString = if self.text.is_empty() {
            self.placeholder.into()
        } else {
            self.text.clone().into()
        };
        let empty = self.text.is_empty();
        let caret = self.caret;
        let highlight = self.marked.clone().unwrap_or_else(|| self.selected());
        div()
            .id("field")
            .track_focus(&self.focus)
            .h(px(32.))
            .p(px(6.))
            .px(px(9.))
            .rounded(px(5.))
            .border_1()
            .border_color(rgb(0x354252))
            .bg(rgb(0x141b24))
            .focus(|style| style.border_color(rgb(0x59c5b8)))
            .overflow_hidden()
            .cursor_text()
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.focus.focus(window, cx);
                    let position = if this.text.is_empty() {
                        0
                    } else {
                        this.shaped
                            .as_ref()
                            .and_then(|(origin, line)| {
                                (line.text.as_ref() == this.text)
                                    .then(|| line.closest_index_for_x(event.position.x - origin.x))
                            })
                            .unwrap_or(this.text.len())
                    };
                    this.move_caret(position, event.modifiers.shift);
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    move |_, window, _| {
                        let style = window.text_style();
                        let run = TextRun {
                            len: text.len(),
                            font: style.font(),
                            color: if empty {
                                rgb(0x8b9bad).into()
                            } else {
                                style.color
                            },
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        window.text_system().shape_line(text, px(13.), &[run], None)
                    },
                    move |bounds, line, window, cx| {
                        let focused = focus.is_focused(window);
                        let caret_x = line.x_for_index(caret);
                        let scroll = if focused {
                            (caret_x - bounds.size.width + px(4.)).max(px(0.))
                        } else {
                            px(0.)
                        };
                        let origin = point(bounds.left() - scroll, bounds.top());
                        if focused {
                            window.handle_input(
                                &focus,
                                ElementInputHandler::new(bounds, entity.clone()),
                                cx,
                            );
                            if !highlight.is_empty() {
                                let a = origin.x + line.x_for_index(highlight.start);
                                let b = origin.x + line.x_for_index(highlight.end);
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(a.min(b), origin.y),
                                        size((b - a).abs(), px(18.)),
                                    ),
                                    rgb(0x305859),
                                ));
                            }
                        }
                        let _ = line.paint(origin, px(18.), window, cx);
                        if focused && highlight.is_empty() {
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(origin.x + caret_x, origin.y + px(2.)),
                                    size(px(1.), px(15.)),
                                ),
                                rgb(0x6de7d5),
                            ));
                        }
                        entity.update(cx, |field, _| field.shaped = Some((origin, line)));
                    },
                )
                .size_full(),
            )
    }
}
