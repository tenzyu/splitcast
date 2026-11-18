//! Validated Studio layout and the single owner's draft, OBS proof and disk state.
use serde_json::{json, Map, Value};
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const FILE: &str = "studio-scene-v1.json";
const FILE_LIMIT: u64 = 4096;
pub(crate) const OVERLAY_ORIGIN: u16 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Size {
    pub(crate) width: u16,
    pub(crate) height: u16,
}

impl Size {
    pub(crate) fn value(self) -> Value {
        json!({"width":self.width,"height":self.height})
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CameraBox {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) size: Size,
    pub(crate) visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Overlay {
    pub(crate) text: Option<String>,
    pub(crate) x: u16,
    pub(crate) y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneLayout {
    camera: Option<CameraBox>,
    overlay: Overlay,
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid Studio scene layout or operation",
    )
}

fn exact<'a>(value: &'a Value, required: &[&str]) -> io::Result<&'a Map<String, Value>> {
    value
        .as_object()
        .filter(|fields| {
            fields.len() == required.len()
                && required.iter().all(|field| fields.contains_key(*field))
        })
        .ok_or_else(invalid)
}

fn coordinate(value: &Value, limit: u16) -> io::Result<u16> {
    value
        .as_u64()
        .and_then(|number| u16::try_from(number).ok())
        .filter(|number| *number <= limit)
        .ok_or_else(invalid)
}

impl SceneLayout {
    pub(crate) fn default_for(width: u16, height: u16, camera: bool, text: Option<String>) -> Self {
        Self {
            camera: camera.then_some(CameraBox {
                x: width - width / 4 - 8,
                y: 8,
                size: Size {
                    width: width / 4,
                    height: height / 4,
                },
                visible: true,
            }),
            overlay: Overlay {
                text,
                x: OVERLAY_ORIGIN,
                y: OVERLAY_ORIGIN,
            },
        }
    }

    pub(crate) fn parse(value: &Value) -> io::Result<Self> {
        let fields = exact(value, &["camera", "overlay"])?;
        let camera = &fields["camera"];
        let camera = if camera.is_null() {
            None
        } else {
            let camera = exact(camera, &["x", "y", "width", "height", "visible"])?;
            let size = Size {
                width: coordinate(&camera["width"], 3840)?,
                height: coordinate(&camera["height"], 2160)?,
            };
            if size.width == 0 || size.height == 0 {
                return Err(invalid());
            }
            Some(CameraBox {
                x: coordinate(&camera["x"], 3839)?,
                y: coordinate(&camera["y"], 2159)?,
                size,
                visible: camera["visible"].as_bool().ok_or_else(invalid)?,
            })
        };
        let overlay = exact(&fields["overlay"], &["text", "x", "y"])?;
        let x = coordinate(&overlay["x"], 3839)?;
        let y = coordinate(&overlay["y"], 2159)?;
        let text = if overlay["text"].is_null() {
            if (x, y) != (OVERLAY_ORIGIN, OVERLAY_ORIGIN) {
                return Err(invalid());
            }
            None
        } else {
            Some(
                overlay["text"]
                    .as_str()
                    .filter(|text| {
                        !text.trim().is_empty()
                            && text.chars().count() <= 80
                            && !text.chars().any(char::is_control)
                    })
                    .ok_or_else(invalid)?
                    .to_owned(),
            )
        };
        Ok(Self {
            camera,
            overlay: Overlay { text, x, y },
        })
    }

    pub(crate) fn validate(&self, canvas: Size, camera: bool) -> io::Result<()> {
        if self.camera.is_some() != camera
            || self.camera.as_ref().is_some_and(|camera| {
                u32::from(camera.x) + u32::from(camera.size.width) > u32::from(canvas.width)
                    || u32::from(camera.y) + u32::from(camera.size.height)
                        > u32::from(canvas.height)
            })
            || self.overlay.x >= canvas.width
            || self.overlay.y >= canvas.height
        {
            return Err(crate::request::rejected(
                "scene layout does not fit the current canvas or configured sources",
            ));
        }
        Ok(())
    }

    pub(crate) fn camera(&self) -> Option<&CameraBox> {
        self.camera.as_ref()
    }

    pub(crate) fn overlay(&self) -> &Overlay {
        &self.overlay
    }

    pub(crate) fn value(&self) -> Value {
        let camera = self.camera.as_ref().map(|camera| json!({
            "x":camera.x,"y":camera.y,"width":camera.size.width,"height":camera.size.height,"visible":camera.visible
        }));
        json!({
            "camera":camera,"overlay":{"text":self.overlay.text,"x":self.overlay.x,"y":self.overlay.y}
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    generation: String,
    operation: String,
}

impl Edit {
    pub(crate) fn parse(args: &Map<String, Value>) -> io::Result<Self> {
        let identifier = |name| {
            args.get(name)
                .and_then(Value::as_str)
                .filter(|value| crate::protocol::valid_id(value))
                .map(str::to_owned)
                .ok_or_else(invalid)
        };
        Ok(Self {
            generation: identifier("generation")?,
            operation: identifier("operation")?,
        })
    }

    pub(crate) fn args(&self) -> Value {
        json!({"generation":self.generation,"operation":self.operation})
    }
}

pub(crate) struct Observation {
    pub(crate) canvas: Size,
    pub(crate) game: Size,
    pub(crate) camera: Option<Size>,
    pub(crate) layout: SceneLayout,
}

#[derive(Clone, Copy)]
pub(crate) enum ResultState {
    Draft,
    Applied,
    Reconciled,
    Saved,
    Reloaded,
    Failed,
    Unknown,
}

impl ResultState {
    fn name(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Applied => "applied",
            Self::Reconciled => "reconciled",
            Self::Saved => "saved",
            Self::Reloaded => "reloaded",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

enum Persistence {
    Missing,
    Known(SceneLayout),
    Unknown,
}

pub(crate) struct Scene {
    instance: String,
    revision: u64,
    operation: Option<(String, ResultState)>,
    pub(crate) draft: SceneLayout,
    pub(crate) live: Option<Observation>,
    saved: Persistence,
}

impl Scene {
    pub(crate) fn new(instance: String, cfg: &crate::config::Config) -> Self {
        let mut scene = Self {
            instance,
            revision: 0,
            operation: None,
            draft: cfg.default_scene.clone(),
            live: None,
            saved: Persistence::Missing,
        };
        if cfg.role == "production" {
            // Unusable state remains visible as unknown. Legacy defaults
            // remain available and an operator can reload or repair the file.
            let _ = scene.reload(cfg);
        }
        scene
    }

    fn generation(&self, generation: u64) -> String {
        format!("{}-{generation}-{}", self.instance, self.revision)
    }

    pub(crate) fn begin(&mut self, edit: Edit, generation: u64) -> io::Result<()> {
        if edit.generation != self.generation(generation) {
            return Err(crate::request::rejected(
                "stale scene generation; read scene before editing",
            ));
        }
        self.advance()?;
        self.operation = Some((edit.operation, ResultState::Unknown));
        Ok(())
    }

    fn advance(&mut self) -> io::Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Studio scene revision exhausted"))?;
        Ok(())
    }

    pub(crate) fn reset(&mut self) -> io::Result<()> {
        self.operation = None;
        self.live = None;
        self.advance()
    }

    pub(crate) fn result(&mut self, state: ResultState) {
        if let Some((_, result)) = &mut self.operation {
            *result = state;
        }
    }

    pub(crate) fn canvas(&self, cfg: &crate::config::Config) -> Size {
        self.live.as_ref().map_or(
            Size {
                width: cfg.width,
                height: cfg.height,
            },
            |live| live.canvas,
        )
    }

    pub(crate) fn matches_draft(&self) -> bool {
        self.live
            .as_ref()
            .is_some_and(|live| live.layout == self.draft)
    }

    pub(crate) fn snapshot(
        &self,
        cfg: &crate::config::Config,
        generation: u64,
        running: bool,
    ) -> String {
        let (saved, persistence) = match &self.saved {
            Persistence::Missing => (None, "unsaved"),
            Persistence::Known(layout) if *layout == self.draft => (Some(layout.value()), "saved"),
            Persistence::Known(layout) => (Some(layout.value()), "unsaved"),
            Persistence::Unknown => (None, "unknown"),
        };
        json!({
            "generation":self.generation(generation),
            "operation":self.operation.as_ref().map(|(id, state)| json!({"id":id,"state":state.name()})),
            "canvas":self.canvas(cfg).value(),
            "sources":self.live.as_ref().map(|live| json!({"game":live.game.value(),"camera":live.camera.map(Size::value)})),
            "mode":if running {"live"} else {"draft"},
            "draft":self.draft.value(),
            "applied":self.live.as_ref().map(|live| live.layout.value()),
            "saved":saved,
            "persistence":persistence
        }).to_string()
    }

    pub(crate) fn reload(&mut self, cfg: &crate::config::Config) -> io::Result<()> {
        match read(cfg, matches!(self.saved, Persistence::Unknown)) {
            Ok(Some(layout)) => {
                self.draft = layout.clone();
                self.saved = Persistence::Known(layout);
            }
            Ok(None) => {
                self.draft = cfg.default_scene.clone();
                self.saved = Persistence::Missing;
            }
            Err(error) => {
                self.saved = Persistence::Unknown;
                return Err(error);
            }
        }
        Ok(())
    }

    pub(crate) fn save(
        &mut self,
        directory: &Path,
        check: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<()> {
        let contents = json!({"version":1,"layout":self.draft.value()}).to_string();
        match crate::state::atomic_file_checked(&directory.join(FILE), contents, check) {
            Ok(()) => {
                self.saved = Persistence::Known(self.draft.clone());
                Ok(())
            }
            Err(crate::state::AtomicFileError::BeforeRename(error)) => {
                self.result(ResultState::Failed);
                Err(error)
            }
            Err(crate::state::AtomicFileError::AfterRename(error)) => {
                self.result(ResultState::Unknown);
                self.saved = Persistence::Unknown;
                Err(error)
            }
        }
    }
}

fn read(cfg: &crate::config::Config, confirm: bool) -> io::Result<Option<SceneLayout>> {
    let directory = crate::state::private_directory(&cfg.state_dir)?;
    let path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(FILE);
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != directory.metadata()?.uid()
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::other("unsafe Studio scene state file"));
    }
    let mut buffer = Vec::new();
    file.take(FILE_LIMIT + 1).read_to_end(&mut buffer)?;
    if buffer.len() as u64 > FILE_LIMIT {
        return Err(invalid());
    }
    let value: Value = serde_json::from_slice(&buffer).map_err(|_| invalid())?;
    let fields = exact(&value, &["version", "layout"])?;
    if fields["version"] != 1 {
        return Err(invalid());
    }
    let layout = SceneLayout::parse(&fields["layout"])?;
    layout.validate(
        Size {
            width: cfg.width,
            height: cfg.height,
        },
        cfg.camera.is_some(),
    )?;
    if confirm {
        directory.sync_all()?;
    }
    Ok(Some(layout))
}
