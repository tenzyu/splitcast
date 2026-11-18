pub(crate) fn asset(path: &str) -> Option<(&'static str, &'static [u8])> {
    match path {
        "/" => Some(("text/html; charset=utf-8", include_bytes!("web/index.html"))),
        "/app.js" => Some((
            "text/javascript; charset=utf-8",
            include_bytes!("web/app.js"),
        )),
        "/style.css" => Some(("text/css; charset=utf-8", include_bytes!("web/style.css"))),
        _ => None,
    }
}
