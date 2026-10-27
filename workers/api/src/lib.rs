mod logic;

pub use logic::{dispatch, ingest_allowed, ArenaState, Incoming, Outcome};

#[cfg(target_arch = "wasm32")]
mod edge {
    use worker::*;

    use crate::logic::{dispatch, ArenaState, Incoming, Outcome};

    #[event(fetch)]
    pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
        let stub = env.durable_object("SPECTATOR")?.get_by_name("arena")?;
        stub.fetch_with_request(req).await
    }

    #[durable_object]
    pub struct SpectatorFeed {
        state: State,
        env: Env,
    }

    impl DurableObject for SpectatorFeed {
        fn new(state: State, env: Env) -> Self {
            Self { state, env }
        }

        async fn fetch(&self, mut req: Request) -> Result<Response> {
            if req.method() == Method::Options {
                return with_cors(Response::empty()?.with_status(204), "text/plain");
            }
            let path = req.path();
            let method = match req.method() {
                Method::Get => "GET",
                Method::Post => "POST",
                Method::Options => "OPTIONS",
                _ => "OTHER",
            };
            let authorization = req.headers().get("authorization")?;
            let body = if method == "POST" {
                req.text().await?
            } else {
                String::new()
            };
            let ingest_token = self
                .env
                .var("FE26_INGEST_TOKEN")
                .ok()
                .map(|value| value.to_string());
            let ingest_token = ingest_token.filter(|value| !value.trim().is_empty());
            let storage = self.state.storage();
            let mut arena: ArenaState = storage.get("arena").await?.unwrap_or_default();
            let outcome = dispatch(
                &mut arena,
                &Incoming {
                    method,
                    path: &path,
                    body: &body,
                    authorization: authorization.as_deref(),
                    ingest_token: ingest_token.as_deref(),
                },
            );
            match outcome {
                Outcome::Http {
                    status,
                    content_type,
                    body,
                    mutated,
                    fanout,
                } => {
                    if mutated {
                        storage.put("arena", &arena).await?;
                    }
                    if let Some(fanout) = fanout {
                        let tag = socket_tag(&fanout.game_id);
                        for socket in self.state.get_websockets_with_tag(&tag) {
                            let _ = socket.send_with_str(&fanout.message);
                        }
                    }
                    let response = Response::ok(body)?.with_status(status);
                    with_cors(response, content_type)
                }
                Outcome::Websocket { game_id, history } => {
                    let pair = WebSocketPair::new()?;
                    let tag = socket_tag(&game_id);
                    self.state
                        .accept_websocket_with_tags(&pair.server, &[tag.as_str()]);
                    pair.server.send_with_str(&history)?;
                    Response::from_websocket(pair.client)
                }
            }
        }
    }

    fn socket_tag(game_id: &str) -> String {
        game_id
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect()
    }

    fn with_cors(mut response: Response, content_type: &str) -> Result<Response> {
        let headers = response.headers_mut();
        headers.set("content-type", content_type)?;
        headers.set("access-control-allow-origin", "*")?;
        headers.set("access-control-allow-methods", "GET, POST, OPTIONS")?;
        headers.set(
            "access-control-allow-headers",
            "content-type, authorization",
        )?;
        Ok(response)
    }
}
