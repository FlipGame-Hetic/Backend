use utoipa::OpenApi;

use crate::modules::health::routes as health;
use crate::modules::screen::routes as screens;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Flipper Game API",
        version = "0.1.0",
        description = "Central API for the Flipper pinball game system"
    ),
    paths(
        health::health_check,
        screens::connected_screens,
        screens::send_to_screen,
    ),
    components(schemas(
        health::HealthResponse,
        screens::ConnectedScreensResponse,
        screens::SendResponse,
        shared::screen::ScreenId,
        shared::screen::ScreenTarget,
        shared::screen::ScreenEnvelope,
    )),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "screens", description = "Screen-to-screen communication (debug & monitoring)"),
    )
)]
pub struct ApiDoc;