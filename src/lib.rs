//! Photos: a gallery of the photos and videos in folders the user gives the
//! plugin — a timeline, filters, a viewer — for RustCloud.
//!
//! The plugin serves its page (`/`, `/app.js`, `/app.css`, `/vendor/…`) and a
//! small JSON API ([`api`]). Background jobs ([`media`]) describe each file
//! through the server's ffmpeg and keep an [index] in the user's AppData;
//! thumbnails, originals and the index reach the page by links, past the
//! plugin. If the user turns them on, [`faces`] finds faces through the
//! server's neural networks and [`people`] groups them into people.

// The bindings wit-bindgen generates have functions with many arguments.
#![allow(clippy::too_many_arguments)]

wit_bindgen::generate!({ path: "wit", world: "app-with-jobs" });

mod api;
mod exif;
mod faces;
mod index;
mod media;
mod people;
mod upload;
mod vision;
mod web;

struct Photos;

impl Guest for Photos {
    fn handle(user: Option<User>, request: Request) -> Response {
        let path = request.path.as_str();
        match (request.method.as_str(), path) {
            ("GET", "/") => web::asset("text/html; charset=utf-8", include_bytes!("../ui/index.html")),
            ("GET", "/app.js") => web::asset("text/javascript; charset=utf-8", include_bytes!("../ui/app.js")),
            ("GET", "/app.css") => web::asset("text/css; charset=utf-8", include_bytes!("../ui/app.css")),
            ("GET", "/vendor/hls.min.js") => {
                web::asset("text/javascript; charset=utf-8", include_bytes!("../ui/vendor/hls.min.js"))
            }
            (method, _) if path.starts_with("/api/") => {
                if user.is_none() {
                    return web::error(401, "not logged in");
                }
                api::handle(method, &path["/api".len()..], &request.query, &request.body)
            }
            _ => web::error(404, "not found"),
        }
    }

    fn run_batch(_user: User, jobs: Vec<Job>) -> Vec<Outcome> {
        // A batch is always of one kind.
        match jobs.first().map(|job| job.kind.as_str()) {
            Some("media") => media::run_media(&jobs),
            Some("scan") => media::run_scan(&jobs),
            Some("prune") => media::run_prune(&jobs),
            Some("faces") => faces::run_faces(&jobs),
            Some("people") => people::run_people(&jobs),
            _ => jobs.iter().map(|job| Outcome::Failed(format!("unknown kind {}", job.kind))).collect(),
        }
    }
}

export!(Photos);
