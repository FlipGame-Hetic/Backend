use utoipa::OpenApi;

use crate::modules::health::routes as health;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Flipper Game API",
        version = "0.1.0",
        description = "Central API for the Flipper pinball game system"
    ),
    paths(
        health::health_check,
    ),
    components(schemas(health::HealthResponse)),
    tags(
        (name = "health", description = "Health check endpoints"),
    )
)]
pub struct ApiDoc;