use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    div, img, point, prelude::*, px, rgb, uniform_list, App, Context, Div, Entity, FontWeight,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, Pixels, Point,
    RenderImage, Subscription, Task, Window,
};
use splitcast_rs_core::Output;
use uuid::Uuid;

use crate::{
    input::Field,
    model::{start_args, Chat, Editor, Reader, Stop, SNAPSHOT_INTERVAL},
    network::{Lane, Network, ResultEvent},
    wire::{Call, Failure, Geometry, Layout, Mode, Phase, Reply, Role, SceneAction, Status},
};

const BG: u32 = 0x10151d;
const CARD: u32 = 0x19222e;
const BORDER: u32 = 0x2c3948;
const TEXT: u32 = 0xe6eef5;
const MUTED: u32 = 0xa2b1c2;
const ACCENT: u32 = 0x6de7d5;
const WARNING: u32 = 0xf5c77b;
const VIEW_W: f32 = 480.;
const VIEW_H: f32 = 270.;

struct Fields {
    seconds: Entity<Field>,
    confirmation: Entity<Field>,
    camera: [Entity<Field>; 4],
    overlay: [Entity<Field>; 3],
}

impl Fields {
    fn new(cx: &mut Context<Studio>) -> Self {
        let mut field = |label, limit| cx.new(|cx| Field::new(label, limit, cx));
        Self {
            seconds: field("Optional", 5),
            confirmation: field("Type LIVE", 4),
            camera: [
                field("X", 5),
                field("Y", 5),
                field("Width", 5),
                field("Height", 5),
            ],
            overlay: [field("Overlay text", 80), field("X", 5), field("Y", 5)],
        }
    }

    fn sync(&self, layout: &Layout, cx: &mut App) {
        if let Some(camera) = &layout.camera {
            for (field, value) in
                self.camera
                    .iter()
                    .zip([camera.x, camera.y, camera.width, camera.height])
            {
                field.update(cx, |field, cx| field.set(value.to_string(), cx));
            }
        }
        for (field, value) in self.overlay.iter().zip([
            layout.overlay.text.clone().unwrap_or_default(),
            layout.overlay.x.to_string(),
            layout.overlay.y.to_string(),
        ]) {
            field.update(cx, |field, cx| field.set(value, cx));
        }
    }

    fn parse(&self, draft: &Layout, cx: &App) -> Option<Layout> {
        let mut result = draft.clone();
        let coordinate = |field: &Entity<Field>| {
            let field = field.read(cx);
            (!field.composing())
                .then(|| field.text().parse::<u32>().ok())
                .flatten()
        };
        if let Some(camera) = &mut result.camera {
            camera.x = coordinate(&self.camera[0])?;
            camera.y = coordinate(&self.camera[1])?;
            camera.width = coordinate(&self.camera[2])?;
            camera.height = coordinate(&self.camera[3])?;
        }
        let text = self.overlay[0].read(cx);
        if text.composing() {
            return None;
        }
        if text.text().trim().is_empty() {
            result.overlay = Default::default();
        } else {
            result.overlay.text = Some(text.text().to_owned());
            result.overlay.x = coordinate(&self.overlay[1])?;
            result.overlay.y = coordinate(&self.overlay[2])?;
        }
        Some(result)
    }
}

enum Control {
    Lifecycle,
    ReadScene,
    ChangeScene(String),
}

#[derive(Clone, Copy)]
enum Guide {
    Camera,
    Overlay,
}

struct Drag {
    guide: Guide,
    pointer: Point<Pixels>,
    start: (f32, f32),
    scale: f32,
}

pub(crate) struct Studio {
    network: Network,
    epoch: u64,
    busy: [bool; Lane::ALL.len()],
    control: Option<Control>,
    status: Option<Status>,
    status_error: Option<Failure>,
    status_revision: u64,
    next_status: Instant,
    status_fresh: bool,
    stop: Stop,
    command_note: Option<&'static str>,
    mode: Option<(Output, String)>,
    fields: Fields,
    valid_fields: bool,
    editor: Editor,
    read_scene: bool,
    editor_open: bool,
    preview_open: bool,
    chat_open: bool,
    preview: Reader,
    chat_reader: Reader,
    image: Option<Arc<RenderImage>>,
    preview_error: bool,
    chat: Chat,
    drag: Option<Drag>,
    trace: bool,
    _tasks: [Task<()>; 2],
    _subscriptions: Vec<Subscription>,
}

impl Studio {
    pub(crate) fn new(
        network: Network,
        results: flume::Receiver<ResultEvent>,
        trace: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_window_title("SplitCast Studio");
        let now = Instant::now();
        let fields = Fields::new(cx);
        let editor = Editor::default();
        fields.sync(&editor.draft, cx);
        let mut subscriptions = Vec::new();
        for field in fields.camera.iter().chain(&fields.overlay) {
            subscriptions.push(cx.observe(field, |this, _, cx| {
                this.input_changed(cx);
            }));
        }
        for field in [&fields.seconds, &fields.confirmation] {
            subscriptions.push(cx.observe(field, |_, _, cx| cx.notify()));
        }
        subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if this.readers(window) {
                cx.notify();
            }
        }));
        let view = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            view.update(cx, |this, cx| {
                if this.busy[Lane::Control.index()] || this.stop.pending() {
                    this.command_note = Some("Waiting for the pending control request");
                    cx.notify();
                    false
                } else {
                    true
                }
            })
            .unwrap_or(true)
        });
        let recv = cx.spawn_in(window, async move |view, cx| {
            while let Ok(event) = results.recv_async().await {
                if cx
                    .update(|window, cx| {
                        view.update(cx, |this, cx| this.complete(event, window, cx))
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let timer = cx.background_executor().clone();
        let poll = cx.spawn_in(window, async move |view, cx| loop {
            let updated = cx.update(|window, cx| view.update(cx, |this, cx| this.tick(window, cx)));
            if !matches!(updated, Ok(Ok(()))) {
                break;
            }
            timer.timer(Duration::from_millis(100)).await;
        });
        Self {
            network,
            epoch: 0,
            busy: [false; Lane::ALL.len()],
            control: None,
            status: None,
            status_error: None,
            status_revision: 0,
            next_status: now,
            status_fresh: false,
            stop: Stop::None,
            command_note: None,
            mode: None,
            fields,
            valid_fields: true,
            editor,
            read_scene: true,
            editor_open: true,
            preview_open: true,
            chat_open: true,
            preview: Reader::new(now),
            chat_reader: Reader::new(now),
            image: None,
            preview_error: false,
            chat: Chat::default(),
            drag: None,
            trace,
            _tasks: [recv, poll],
            _subscriptions: subscriptions,
        }
    }

    fn send(&mut self, lane: Lane, call: Call, revision: u64) -> bool {
        let busy = &mut self.busy[lane.index()];
        if *busy || !self.network.send(lane, call, self.epoch, revision) {
            return false;
        }
        *busy = true;
        true
    }

    fn refresh_status(&mut self) {
        self.next_status = Instant::now();
    }

    fn invalidate_status(&mut self) {
        self.status_revision += 1;
        self.status_fresh = false;
        self.refresh_status();
    }

    fn active_phase(&self) -> Option<Phase> {
        self.status
            .as_ref()
            .filter(|_| self.status_fresh)
            .map(|status| status.state)
    }

    fn readers(&mut self, window: &mut Window) -> bool {
        let active = window.is_window_active() && !self.stop.pending();
        let phase = self.active_phase();
        let before = self.preview.visibility(
            active && self.preview_open && self.editor_open && phase == Some(Phase::Running),
        );
        if before && !self.preview.visible {
            self.clear_image(window);
            self.preview_error = false;
        }
        before
            | self.chat_reader.visibility(
                active
                    && self.chat_open
                    && matches!(phase, Some(Phase::Ready | Phase::Running | Phase::Degraded)),
            )
    }

    fn clear_image(&mut self, window: &mut Window) {
        if let Some(image) = self.image.take() {
            if window.drop_image(image).is_err() {
                self.preview_error = true;
            }
        }
    }

    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let before = self.stop.label();
        self.stop.observe(None, now);
        if self.readers(window) || self.stop.label() != before {
            cx.notify();
        }
        if now >= self.next_status && self.send(Lane::Status, Call::Status, self.status_revision) {
            self.next_status = now
                + if self.stop.pending() {
                    Duration::from_millis(250)
                } else {
                    Duration::from_secs(2)
                };
        }
        if self.read_scene && self.scene_phase() && self.send(Lane::Control, Call::Scene, 0) {
            self.control = Some(Control::ReadScene);
            self.read_scene = false;
            // Disable writes as soon as the observation becomes in-flight.
            cx.notify();
        }
        if self.preview.due(now) && self.send(Lane::Preview, Call::Preview, self.preview.revision) {
            self.preview.next = now + SNAPSHOT_INTERVAL;
        }
        if self.chat_reader.due(now) {
            self.send(
                Lane::Chat,
                Call::Chat {
                    epoch: self.chat.epoch.clone(),
                    cursor: self.chat.cursor,
                },
                self.chat_reader.revision,
            );
        }
    }

    fn complete(&mut self, event: ResultEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.busy[event.lane.index()] = false;
        if event.epoch != self.epoch {
            if event.lane == Lane::Control {
                self.control = None;
                self.invalidate_status();
                cx.notify();
            }
            return;
        }
        let now = Instant::now();
        let changed = match event.lane {
            Lane::Status if event.revision == self.status_revision => {
                self.status(event.result, now)
            }
            Lane::Status => false,
            Lane::Stop => {
                self.stop.accept(
                    event.result.and_then(|reply| match reply {
                        Reply::Stop(reply) => Ok(reply),
                        _ => Err(Failure::Unknown),
                    }),
                    now,
                );
                self.invalidate_status();
                true
            }
            Lane::Control => {
                self.control_result(event.result, cx);
                true
            }
            Lane::Chat if self.chat_reader.accepts(event.revision) => {
                let (changed, delay) =
                    self.chat.update(event.result.and_then(|reply| match reply {
                        Reply::Chat(page) => Ok(page),
                        _ => Err(Failure::Unknown),
                    }));
                self.chat_reader.next = now + delay;
                changed
            }
            Lane::Preview => {
                // Space requests by a full second after the worker finishes,
                // including cancellations and quick hide/show cycles.
                self.preview.next = now + SNAPSHOT_INTERVAL;
                if !self.preview.accepts(event.revision) {
                    return;
                }
                match event.result {
                    Ok(Reply::Preview(mut rgba)) => {
                        for pixel in rgba.pixels_mut() {
                            pixel.0.swap(0, 2);
                        }
                        if self.image.as_ref().and_then(|image| image.as_bytes(0))
                            == Some(rgba.as_raw().as_slice())
                        {
                            false
                        } else {
                            self.clear_image(window);
                            self.image =
                                Some(Arc::new(RenderImage::new([image::Frame::new(rgba)])));
                            self.preview_error = false;
                            true
                        }
                    }
                    _ => {
                        let changed = self.image.is_some() || !self.preview_error;
                        self.clear_image(window);
                        self.preview_error = true;
                        changed
                    }
                }
            }
            Lane::Chat => false,
        };
        if self.readers(window) || changed {
            cx.notify();
        }
    }

    fn status(&mut self, result: Result<Reply, Failure>, now: Instant) -> bool {
        let previous_phase = self
            .status
            .as_ref()
            .map(|status| (status.role, status.state));
        let was_fresh = self.status_fresh;
        let old_stop = self.stop.label();
        match result {
            Ok(Reply::Status(status)) => {
                self.stop.observe(Some(&status), now);
                self.status_fresh = true;
                if let Some((_, id)) = &self.mode {
                    if status.start_id() != Some(id) {
                        self.mode = None;
                    }
                }
                let changed = self.status.as_ref() != Some(&status)
                    || self.status_error.take().is_some()
                    || !was_fresh;
                if previous_phase != Some((status.role, status.state)) || !was_fresh {
                    self.read_scene = true;
                }
                if status.state == Phase::Idle
                    && previous_phase.is_some_and(|(_, phase)| phase != Phase::Idle)
                {
                    self.chat = Chat::default();
                }
                self.status = Some(status);
                changed || self.stop.label() != old_stop
            }
            error => {
                let failure = error.err().unwrap_or(Failure::Unknown);
                let changed = self.status_error != Some(failure) || was_fresh;
                self.status_error = Some(failure);
                self.status_fresh = false;
                changed
            }
        }
    }

    fn control_result(&mut self, reply: Result<Reply, Failure>, cx: &mut Context<Self>) {
        match (self.control.take(), reply) {
            (Some(Control::Lifecycle), Ok(Reply::Changed)) => {
                self.command_note = Some("Command acknowledged · refreshing status");
                self.invalidate_status();
            }
            (Some(Control::Lifecycle), Err(error)) => {
                self.command_note = Some(error.label());
                self.invalidate_status();
            }
            (Some(Control::ReadScene), Ok(Reply::Scene(scene))) => {
                self.editor.observe(scene, None);
                if !self.editor.dirty {
                    self.fields.sync(&self.editor.draft, cx);
                }
                self.valid_fields = self.fields.parse(&self.editor.draft, cx).is_some();
            }
            (Some(Control::ChangeScene(id)), Ok(Reply::Scene(scene))) => {
                self.editor.observe(scene, Some(&id));
                if !self.editor.dirty {
                    self.fields.sync(&self.editor.draft, cx);
                }
            }
            (Some(Control::ChangeScene(id)), Err(error)) => {
                if matches!(error, Failure::Unknown) {
                    self.editor.uncertain = Some(id);
                }
                if error == Failure::Conflict {
                    self.editor.conflict = true;
                }
                self.editor.notice = Some(error.label());
            }
            (Some(Control::ReadScene), Err(error)) => self.editor.notice = Some(error.label()),
            _ => self.command_note = Some(Failure::Unknown.label()),
        }
    }

    fn lifecycle(&mut self, call: Call, cx: &mut Context<Self>) {
        if self.send(Lane::Control, call, 0) {
            self.control = Some(Control::Lifecycle);
            self.mode = None;
            self.command_note = Some("Waiting for Agent…");
            self.read_scene = false;
            cx.notify();
        }
    }

    fn stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy[Lane::Stop.index()] {
            return;
        }
        self.epoch = self.network.supersede();
        if let Some(Control::ChangeScene(ref id)) = self.control {
            self.editor.uncertain = Some(id.clone());
        }
        self.mode = None;
        self.command_note = None;
        self.stop = Stop::Sending;
        self.invalidate_status();
        self.readers(window);
        if !self.send(Lane::Stop, Call::Stop, 0) {
            self.stop = Stop::Rejected(Failure::Disconnected);
        }
        cx.notify();
    }

    fn ready_to_act(&self) -> bool {
        self.status_fresh
            && !self.busy[Lane::Control.index()]
            && !self.stop.pending()
            && self
                .status
                .as_ref()
                .is_some_and(|status| !status.remote_release_pending || status.state != Phase::Idle)
    }

    fn scene_phase(&self) -> bool {
        !self.stop.pending()
            && self.active_phase().is_some_and(|phase| match phase {
                Phase::Idle => self
                    .status
                    .as_ref()
                    .is_some_and(|status| status.role == Role::Production),
                Phase::Ready | Phase::Running => true,
                _ => false,
            })
    }

    fn scene_change(&mut self, action: SceneAction, cx: &mut Context<Self>) {
        if !self.scene_actions() {
            return;
        }
        let Some(scene) = self.editor.scene.as_ref() else {
            return;
        };
        let id = Uuid::new_v4().simple().to_string();
        let request = Call::SceneChange {
            action,
            generation: scene.generation.clone(),
            id: id.clone(),
            layout: (action == SceneAction::Apply).then(|| self.editor.draft.clone()),
        };
        if self.send(Lane::Control, request, 0) {
            self.control = Some(Control::ChangeScene(id));
            self.editor.notice = Some("Waiting for Agent scene…");
            self.read_scene = false;
            cx.notify();
        }
    }

    fn scene_actions(&self) -> bool {
        self.ready_to_act()
            && self.scene_phase()
            && self.editor.uncertain.is_none()
            && !self.read_scene
    }

    fn input_changed(&mut self, cx: &mut Context<Self>) {
        // A pending scene mutation owns the submitted values. Fields are
        // temporarily omitted from the editor until that request is settled.
        if matches!(self.control, Some(Control::ChangeScene(_))) {
            return;
        }
        let draft = self.fields.parse(&self.editor.draft, cx);
        self.valid_fields = draft.is_some();
        if let Some(draft) = draft {
            self.editor.edited(draft);
        }
        cx.notify();
    }

    fn start_args(&self, cx: &App) -> Option<serde_json::Value> {
        if !self.ready_to_act() {
            return None;
        }
        let (output, armed_id) = self.mode.as_ref()?;
        start_args(
            self.status.as_ref()?,
            *output,
            Some(armed_id),
            self.fields.seconds.read(cx).text(),
            self.fields.confirmation.read(cx).text(),
        )
    }

    fn trace(&self, window: &Window) {
        if self.trace {
            let phase = self.active_phase().map(Phase::label).unwrap_or("offline");
            let stop = self.stop.label().unwrap_or("-");
            let scene = self.editor.label();
            let preview = if self.image.is_some() { "frame" } else { "-" };
            eprintln!(
                "studio-render {phase} | {stop} | {scene} | {preview} | active={}",
                window.is_window_active()
            );
        }
    }

    fn top(&self, cx: &mut Context<Self>) -> Div {
        let ready = self.ready_to_act();
        let start = self.start_args(cx);
        let phase = self.active_phase();
        let prepared = ready && phase == Some(Phase::Idle);
        let mut current = "Connecting to Agent".to_owned();
        if let Some(error) = self.status_error {
            current = if error == Failure::Unknown {
                "Agent unavailable · status unconfirmed".into()
            } else {
                error.label().to_owned()
            };
        } else if let Some(status) = &self.status {
            let role = match status.role {
                Role::Capture => "Capture",
                Role::Production => "Production",
            };
            current = format!("{role}  ·  {}", status.state.label());
            if let Some(output) = &status.output {
                let label = match output.as_str() {
                    "recording" => "Recording",
                    "twitch-test" => "Test",
                    "twitch-live" => "Live",
                    _ => "Unknown output",
                };
                current.push_str("  ·  ");
                current.push_str(label);
            }
        }
        let mut modes = row().gap(px(7.));
        for (label, output) in [
            ("Recording", Output::Recording),
            ("Test", Output::TwitchTest),
            ("Live", Output::TwitchLive),
        ] {
            let selected = self.mode.as_ref().is_some_and(|(mode, _)| *mode == output);
            modes = modes.child(
                button(
                    label,
                    ready && phase == Some(Phase::Ready),
                    cx,
                    move |this, _, cx| {
                        if let Some(id) = this.status.as_ref().and_then(Status::start_id) {
                            this.mode = Some((output, id.to_owned()));
                            this.fields
                                .seconds
                                .update(cx, |field, cx| field.set(String::new(), cx));
                            this.fields
                                .confirmation
                                .update(cx, |field, cx| field.set(String::new(), cx));
                            cx.notify();
                        }
                    },
                )
                .when(selected, |button| {
                    button.bg(rgb(0x22534f)).text_color(rgb(ACCENT))
                }),
            );
        }
        card()
            .gap(px(11.))
            .child(
                row()
                    .justify_between()
                    .items_start()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(23.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("SplitCast Studio"),
                            )
                            .child(div().text_sm().text_color(rgb(ACCENT)).child(current)),
                    )
                    .child(
                        row()
                            .gap(px(8.))
                            .child(button("Status", true, cx, |this, _, _| {
                                this.refresh_status()
                            }))
                            .child(
                                button("Stop", true, cx, Self::stop)
                                    .bg(rgb(0x9e3544))
                                    .text_color(rgb(0xffffff))
                                    .hover(|style| style.bg(rgb(0xb03d4e)))
                                    .px(px(26.)),
                            ),
                    ),
            )
            .child(
                row()
                    .gap(px(9.))
                    .child(button("Prepare", prepared, cx, |this, _, cx| {
                        this.lifecycle(Call::Prepare { media: true }, cx)
                    }))
                    .when(
                        self.status
                            .as_ref()
                            .is_some_and(|status| status.role == Role::Production),
                        |row| {
                            row.child(button(
                                "Prepare chat / speech",
                                prepared,
                                cx,
                                |this, _, cx| this.lifecycle(Call::Prepare { media: false }, cx),
                            ))
                        },
                    )
                    .child(modes)
                    .when_some(self.mode.as_ref(), |row, (output, _)| {
                        row.child(div().w(px(88.)).child(self.fields.seconds.clone()))
                            .child(div().text_sm().text_color(rgb(MUTED)).child("sec"))
                            .when(*output == Output::TwitchLive, |row| {
                                row.child(div().w(px(104.)).child(self.fields.confirmation.clone()))
                            })
                    })
                    .child(button("Start", start.is_some(), cx, |this, _, cx| {
                        if let Some(args) = this.start_args(cx) {
                            this.lifecycle(Call::Start(args), cx);
                        }
                    })),
            )
            .child(
                div()
                    .h(px(18.))
                    .text_size(px(12.))
                    .text_color(rgb(if self.stop.pending() { WARNING } else { MUTED }))
                    .child(self.stop.label().or(self.command_note).unwrap_or("")),
            )
    }

    fn preview(&self, cx: &mut Context<Self>) -> Div {
        let label = if !self.preview_open {
            "Preview hidden"
        } else if !self.editor_open {
            "Show scene editor to use the preview"
        } else if self.preview_error {
            "OBS snapshot unavailable"
        } else if self.active_phase() != Some(Phase::Running) {
            "Preview available during an active output"
        } else {
            "Waiting for OBS snapshot"
        };
        let mut screen = div()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w(px(VIEW_W))
            .h(px(VIEW_H))
            .overflow_hidden()
            .rounded(px(6.))
            .bg(rgb(0x0e151e));
        if let Some(image) = &self.image {
            screen = screen.child(
                img(image.clone())
                    .object_fit(ObjectFit::Contain)
                    .size_full(),
            );
        } else {
            screen = screen.child(div().text_size(px(13.)).text_color(rgb(MUTED)).child(label));
        }
        if self.preview_open && self.editor_open {
            if let Some(scene) = &self.editor.scene {
                let geometry = &scene.canvas;
                if let Some(camera) = &self.editor.draft.camera {
                    screen = screen.child(self.guide(
                        Guide::Camera,
                        geometry,
                        [camera.x, camera.y, camera.width, camera.height],
                        cx,
                    ));
                }
                let overlay = &self.editor.draft.overlay;
                if overlay.text.is_some() {
                    screen = screen.child(self.guide(
                        Guide::Overlay,
                        geometry,
                        [overlay.x, overlay.y, 0, 0],
                        cx,
                    ));
                }
            }
        }
        card()
            .flex_none()
            .gap(px(9.))
            .child(
                row()
                    .justify_between()
                    .child(title("OBS Preview"))
                    .child(button(
                        if self.preview_open {
                            "Hide preview"
                        } else {
                            "Show preview"
                        },
                        true,
                        cx,
                        |this, window, cx| {
                            this.preview_open = !this.preview_open;
                            this.readers(window);
                            cx.notify();
                        },
                    )),
            )
            .child(screen)
    }

    fn guide(
        &self,
        guide: Guide,
        geometry: &Geometry,
        [x, y, width, height]: [u32; 4],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (w, h) = (geometry.width as f32, geometry.height as f32);
        let scale = (VIEW_W / w).min(VIEW_H / h);
        let left = ((VIEW_W - w * scale) / 2.) + x as f32 * scale;
        let top = ((VIEW_H - h * scale) / 2.) + y as f32 * scale;
        let (label, tint) = match guide {
            Guide::Camera => ("CameraFeed", ACCENT),
            Guide::Overlay => ("Text", WARNING),
        };
        let mut marker = div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .cursor_move()
            .border_1()
            .border_color(rgb(tint))
            .rounded(px(3.))
            .text_color(rgb(tint))
            .text_xs()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    if !matches!(this.control, Some(Control::ChangeScene(_))) {
                        this.drag = Some(Drag {
                            guide,
                            pointer: event.position,
                            start: (x as f32, y as f32),
                            scale,
                        });
                        cx.stop_propagation();
                    }
                }),
            )
            .child(div().bg(rgb(0x16252b)).px(px(4.)).child(label));
        if width > 0 {
            marker = marker
                .w(px(width as f32 * scale))
                .h(px(height as f32 * scale));
        }
        marker
    }

    fn drag(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = &self.drag else {
            return;
        };
        if !event.dragging() {
            self.drag = None;
            return;
        }
        let Some(scene) = &self.editor.scene else {
            return;
        };
        let delta = event.position - drag.pointer;
        let x = drag.start.0 + f32::from(delta.x) / drag.scale;
        let y = drag.start.1 + f32::from(delta.y) / drag.scale;
        let within = |v: f32, extent: u32, bound: u32| {
            v.round().clamp(0., bound.saturating_sub(extent) as f32) as u32
        };
        let mut layout = self.editor.draft.clone();
        match drag.guide {
            Guide::Camera => {
                if let Some(camera) = &mut layout.camera {
                    camera.x = within(x, camera.width, scene.canvas.width);
                    camera.y = within(y, camera.height, scene.canvas.height);
                }
            }
            Guide::Overlay => {
                layout.overlay.x = within(x, 1, scene.canvas.width);
                layout.overlay.y = within(y, 1, scene.canvas.height);
            }
        }
        self.editor.edited(layout);
        self.fields.sync(&self.editor.draft, cx);
        cx.notify();
    }

    fn chat(&self, cx: &mut Context<Self>) -> Div {
        let label = if !self.chat_open {
            "Chat hidden"
        } else if !self.chat_reader.visible {
            "Chat available during a prepared session"
        } else {
            self.chat.label()
        };
        card()
            .flex_1()
            .min_w_0()
            .gap(px(9.))
            .overflow_hidden()
            .child(
                row()
                    .justify_between()
                    .child(title("Twitch Chat"))
                    .child(button(
                        if self.chat_open {
                            "Hide chat"
                        } else {
                            "Show chat"
                        },
                        true,
                        cx,
                        |this, window, cx| {
                            this.chat_open = !this.chat_open;
                            this.readers(window);
                            cx.notify();
                        },
                    )),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(label)
                    .when(self.chat_reader.visible && self.chat.gap(), |div| {
                        div.child("  ·  History gap")
                    }),
            )
            .when(self.chat_open && !self.chat.messages.is_empty(), |card| {
                card.child(
                    uniform_list(
                        "chat",
                        self.chat.messages.len(),
                        cx.processor(|this, range, _, _| {
                            // The newest retained message is at the top. GPUI asks
                            // for visible rows only, with one fixed height per row.
                            range
                                .map(|index| {
                                    let message =
                                        &this.chat.messages[this.chat.messages.len() - 1 - index];
                                    div()
                                        .h(px(54.))
                                        .overflow_hidden()
                                        .border_b_1()
                                        .border_color(rgb(BORDER))
                                        .py(px(5.))
                                        .text_size(px(13.))
                                        .child(
                                            div()
                                                .truncate()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(ACCENT))
                                                .child(message.display_name.clone()),
                                        )
                                        .child(div().truncate().child(message.text.clone()))
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .h(px(244.))
                    .w_full(),
                )
            })
    }

    fn scene(&self, cx: &mut Context<Self>) -> Div {
        let can_change = self.scene_actions();
        let live = self.active_phase() == Some(Phase::Running)
            && self
                .editor
                .scene
                .as_ref()
                .is_some_and(|scene| scene.mode == Mode::Live);
        let can_apply =
            can_change && !self.editor.conflict && self.valid_fields && self.editor.valid();
        let scene = self.editor.scene.as_ref();
        let applied = scene
            .and_then(|scene| scene.applied.as_ref())
            .filter(|_| live);
        let editing = !matches!(self.control, Some(Control::ChangeScene(_)));
        let game = match scene {
            Some(scene) => format!("Fixed  ·  {} × {}", scene.canvas.width, scene.canvas.height),
            None => "Fixed".into(),
        };
        let fields = if editing {
            let game = group("GameFeed").child(div().text_sm().text_color(rgb(MUTED)).child(game));
            let camera = group("CameraFeed")
                .when_some(self.editor.draft.camera.as_ref(), |group, camera| {
                    group
                        .child(
                            row().gap(px(5.)).children(
                                ["X", "Y", "W", "H"]
                                    .into_iter()
                                    .zip(&self.fields.camera)
                                    .map(|(name, field)| labeled(name, field)),
                            ),
                        )
                        .child(
                            row()
                                .gap(px(7.))
                                .child(button(
                                    if camera.visible { "Visible" } else { "Hidden" },
                                    true,
                                    cx,
                                    |this, _, cx| {
                                        let mut layout = this.editor.draft.clone();
                                        if let Some(camera) = &mut layout.camera {
                                            camera.visible = !camera.visible;
                                        }
                                        this.editor.edited(layout);
                                        cx.notify();
                                    },
                                ))
                                .when_some(
                                    applied.and_then(|layout| layout.camera.as_ref()),
                                    |row, read| {
                                        row.child(small(format!(
                                            "OBS  {}, {}  ·  {} × {}{}",
                                            read.x,
                                            read.y,
                                            read.width,
                                            read.height,
                                            if read.visible { "" } else { " · hidden" }
                                        )))
                                    },
                                ),
                        )
                })
                .when(self.editor.draft.camera.is_none(), |group| {
                    group.child(small(if scene.is_some() {
                        "Camera unavailable"
                    } else {
                        "Awaiting Agent layout"
                    }))
                });
            let overlay = group("Text Overlay")
                .child(self.fields.overlay[0].clone())
                .child(
                    row()
                        .gap(px(5.))
                        .child(labeled("X", &self.fields.overlay[1]))
                        .child(labeled("Y", &self.fields.overlay[2]))
                        .when_some(applied.map(|layout| &layout.overlay), |row, read| {
                            row.child(small(if read.text.is_some() {
                                format!("OBS  {}, {}", read.x, read.y)
                            } else {
                                "OBS  ·  absent".into()
                            }))
                        }),
                );
            row()
                .items_start()
                .gap(px(16.))
                .child(game.w(px(155.)).flex_none())
                .child(camera.flex_1())
                .child(overlay.flex_1())
        } else {
            div().h(px(104.)).child(small("Updating owned scene…"))
        };
        let mut panel =
            card()
                .gap(px(10.))
                .child(
                    row()
                        .justify_between()
                        .child(title("Scene Editor"))
                        .child(button(
                            if self.editor_open {
                                "Hide editor"
                            } else {
                                "Show editor"
                            },
                            true,
                            cx,
                            |this, window, cx| {
                                this.editor_open = !this.editor_open;
                                this.drag = None;
                                this.readers(window);
                                cx.notify();
                            },
                        )),
                );
        if self.editor_open {
            panel = panel
                .child(fields)
                .child(
                    row()
                        .gap(px(7.))
                        .child(button(
                            "Read Agent draft",
                            self.scene_phase() && !self.busy[Lane::Control.index()],
                            cx,
                            |this, _, cx| {
                                this.read_scene = true;
                                cx.notify();
                            },
                        ))
                        .child(button("Apply", can_apply, cx, |this, _, cx| {
                            this.scene_change(SceneAction::Apply, cx)
                        }))
                        .child(button(
                            "OBS readback",
                            can_change && live,
                            cx,
                            |this, _, cx| this.scene_change(SceneAction::Reconcile, cx),
                        ))
                        .child(button(
                            "Save",
                            can_change && live && self.editor.can_save(),
                            cx,
                            |this, _, cx| this.scene_change(SceneAction::Save, cx),
                        ))
                        .child(button(
                            "Reload saved",
                            can_change && !self.editor.dirty,
                            cx,
                            |this, _, cx| this.scene_change(SceneAction::Reload, cx),
                        ))
                        .when(self.editor.dirty || self.editor.conflict, |row| {
                            row.child(button(
                                "Use Agent draft",
                                self.editor.scene.is_some() && editing,
                                cx,
                                |this, _, cx| {
                                    this.editor.replace_with_agent();
                                    this.fields.sync(&this.editor.draft, cx);
                                    this.valid_fields = true;
                                    cx.notify();
                                },
                            ))
                        })
                        .when(self.editor.conflict, |row| {
                            row.child(button(
                                "Keep local draft",
                                editing && self.editor.valid(),
                                cx,
                                |this, _, cx| {
                                    this.editor.conflict = false;
                                    cx.notify();
                                },
                            ))
                        }),
                )
                .child(small(if self.status_error.is_some() {
                    "Disconnected or unconfirmed · local edits available"
                } else if !self.valid_fields || self.editor.scene.is_some() && !self.editor.valid()
                {
                    "Draft requires valid canvas coordinates"
                } else {
                    self.editor.label()
                }))
                .when_some(self.editor.notice, |panel, notice| {
                    panel.child(small(notice))
                });
        }
        panel
    }
}

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.trace(window);
        div()
            .id("studio")
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .text_size(px(14.))
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(14.))
            .overflow_y_scroll()
            .tab_group()
            .on_mouse_move(cx.listener(Self::drag))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .child(self.top(cx))
            .child(
                row()
                    .items_stretch()
                    .gap(px(12.))
                    .child(self.preview(cx))
                    .child(self.chat(cx)),
            )
            .child(self.scene(cx))
    }
}

fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(9.))
        .border_1()
        .border_color(rgb(BORDER))
        .p(px(13.))
        .bg(rgb(CARD))
}

fn row() -> Div {
    div().flex().items_center()
}

fn title(text: &'static str) -> Div {
    div()
        .text_size(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

fn small(text: impl Into<gpui::SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text.into())
}

fn group(label: &'static str) -> Div {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(div().font_weight(FontWeight::MEDIUM).child(label))
}

fn labeled(label: &'static str, field: &Entity<Field>) -> Div {
    div()
        .w(px(67.))
        .flex()
        .flex_col()
        .child(small(label))
        .child(field.clone())
}

fn button(
    label: &'static str,
    enabled: bool,
    cx: &Context<Studio>,
    action: impl Fn(&mut Studio, &mut Window, &mut Context<Studio>) + 'static,
) -> gpui::Stateful<Div> {
    div()
        .id(label)
        .h(px(32.))
        .px(px(10.))
        .flex()
        .justify_center()
        .items_center()
        .flex_none()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .rounded(px(5.))
        .bg(rgb(if enabled { 0x293b4d } else { 0x202b37 }))
        .text_color(rgb(if enabled { TEXT } else { 0x788899 }))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x354f65)))
                .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
        })
        .child(label)
}
