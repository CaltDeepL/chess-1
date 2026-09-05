use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "chess-app API",
        version = env!("CARGO_PKG_VERSION"),
        description = "オンライン対戦チェスアプリのバックエンドAPI。\n\n\
                       `POST /auth/register` でアカウントを作成し、返却されたトークンを \
                       右上の Authorize に入力してください。",
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "ユーザー登録・ログイン"),
        (name = "games", description = "対局の作成・参加・進行"),
        (name = "users", description = "ユーザー情報"),
        (name = "system", description = "疎通確認"),
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}
