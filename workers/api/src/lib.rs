mod indexer;
mod logic;
mod surface;

pub use indexer::{apply_chain_events, refuse_mainnet};
pub use logic::{dispatch, ingest_allowed, ArenaState, Incoming, Outcome};

#[cfg(target_arch = "wasm32")]
mod edge {
    use worker::*;

    use crate::indexer;
    use crate::logic::{dispatch, path_with_query, ArenaState, Incoming, Outcome};

    #[event(fetch)]
    pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
        let stub = env.durable_object("SPECTATOR")?.get_by_name("arena")?;
        stub.fetch_with_request(req).await
    }

    #[event(scheduled)]
    pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
        if let Ok(stub) = env
            .durable_object("SPECTATOR")
            .and_then(|ns| ns.get_by_name("arena"))
        {
            let mut init = RequestInit::new();
            init.with_method(Method::Post);
            if let Ok(req) = Request::new_with_init("https://fe2vi.invalid/v1/index", &init) {
                let _ = stub.fetch_with_request(req).await;
            }
        }
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
            if req.method() == Method::Post && (path == "/v1/index" || path.ends_with("/v1/index"))
            {
                return self.run_index().await;
            }
            // `Request::path` strips `?window=` and `?now=`. Put them back.
            let route = match req.url() {
                Ok(url) => path_with_query(&path, url.query()),
                Err(_) => path.clone(),
            };
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
                .var("FE2VI_INGEST_TOKEN")
                .ok()
                .map(|value| value.to_string());
            let ingest_token = ingest_token.filter(|value| !value.trim().is_empty());
            if method == "GET" && (route == "/v1/admin/status" || route.starts_with("/v1/admin/status?")) {
                let token = self
                    .env
                    .var("ADMIN_TOKEN")
                    .ok()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                let presented = authorization.clone().unwrap_or_default();
                if token.is_empty() || presented != format!("Bearer {token}") {
                    return with_cors(Response::empty()?.with_status(404), "text/plain");
                }
                let storage = self.state.storage();
                let requests: u64 = storage.get("metric_requests").await?.unwrap_or(0);
                let errors: u64 = storage.get("metric_errors").await?.unwrap_or(0);
                let cron_last: u64 = storage.get("metric_cron_last").await?.unwrap_or(0);
                let arena: ArenaState = storage.get("arena").await?.unwrap_or_default();
                let body = serde_json::json!({
                    "requests": requests,
                    "errors": errors,
                    "cron_last": cron_last,
                    "durable_object": {
                        "games": arena.games.len(),
                        "agents": arena.agents.len(),
                        "events": arena.events.len(),
                        "indexed_height": arena.indexed_height,
                    }
                })
                .to_string();
                return with_cors(Response::ok(body)?, "application/json");
            }
            let storage = self.state.storage();
            let mut arena: ArenaState = storage.get("arena").await?.unwrap_or_default();
            let outcome = dispatch(
                &mut arena,
                &Incoming {
                    method,
                    path: &route,
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
                    location,
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
                    let requests: u64 = storage.get("metric_requests").await?.unwrap_or(0);
                    let _ = storage.put("metric_requests", &(requests.saturating_add(1))).await;
                    if status >= 500 {
                        let errors: u64 = storage.get("metric_errors").await?.unwrap_or(0);
                        let _ = storage.put("metric_errors", &(errors.saturating_add(1))).await;
                    }
                    let mut response = Response::ok(body)?.with_status(status);
                    if let Some(location) = location {
                        response.headers_mut().set("Location", &location)?;
                    }
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

    impl SpectatorFeed {
        async fn run_index(&self) -> Result<Response> {
            let rpc = self
                .env
                .var("FE2VI_RPC_URL")
                .ok()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "https://rpc.testnet.near.org".to_string());
            if indexer::refuse_mainnet(&rpc).is_err() {
                return with_cors(
                    Response::ok(r#"{"ok":false,"error":"mainnet rpc refused"}"#)?.with_status(400),
                    "application/json",
                );
            }
            let storage = self.state.storage();
            let mut arena: ArenaState = storage.get("arena").await?.unwrap_or_default();
            let head_block =
                rpc_call(&rpc, "block", serde_json::json!({"finality": "final"})).await?;
            let head = indexer::block_height(&head_block).unwrap_or(0);
            let heights = indexer::plan_heights(arena.indexed_height, head);
            let watch = watch_accounts(&self.env);
            let mut found = Vec::new();
            let mut scanned = arena.indexed_height;
            for height in heights {
                let block = if height == head {
                    head_block.clone()
                } else {
                    match rpc_call(&rpc, "block", serde_json::json!({"block_id": height})).await {
                        Ok(block) => block,
                        Err(_) => break,
                    }
                };
                let mut height_ok = true;
                for hash in indexer::chunk_hashes(&block) {
                    let chunk = match rpc_call(&rpc, "chunk", serde_json::json!({"chunk_id": hash}))
                        .await
                    {
                        Ok(chunk) => chunk,
                        Err(_) => {
                            height_ok = false;
                            break;
                        }
                    };
                    for (tx_hash, signer) in indexer::chunk_tx_refs(&chunk, &watch) {
                        let status = match rpc_call(
                            &rpc,
                            "tx",
                            serde_json::json!([tx_hash, signer]),
                        )
                        .await
                        {
                            Ok(status) => status,
                            Err(_) => continue,
                        };
                        for (index, log) in indexer::logs_from_tx_status(&status)
                            .into_iter()
                            .enumerate()
                        {
                            if let Some(event) = indexer::parse_event_log(&log, height, index) {
                                found.push(event);
                            }
                        }
                    }
                    if !height_ok {
                        break;
                    }
                }
                if !height_ok {
                    break;
                }
                scanned = height;
            }
            let added = indexer::apply_chain_events(&mut arena, &found, scanned);
            storage.put("arena", &arena).await?;
            let cron_now = worker::Date::now().as_millis() / 1000;
            let _ = storage.put("metric_cron_last", &cron_now).await;
            let body = serde_json::json!({
                "ok": true,
                "added": added,
                "indexed_height": arena.indexed_height,
                "network": "testnet",
            })
            .to_string();
            with_cors(Response::ok(body)?, "application/json")
        }
    }

    fn watch_accounts(env: &Env) -> Vec<String> {
        ["FE2VI_FACTORY_ID", "FE2VI_REGISTRY_ID", "FE2VI_HEIST_ID"]
            .into_iter()
            .map(|name| {
                env.var(name)
                    .ok()
                    .map(|value| value.to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| match name {
                        "FE2VI_FACTORY_ID" => "factory.fe2vi.testnet".to_string(),
                        "FE2VI_REGISTRY_ID" => "registry.fe2vi.testnet".to_string(),
                        _ => "heist.fe2vi.testnet".to_string(),
                    })
            })
            .collect()
    }

    async fn rpc_call(
        url: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "fe2vi",
            "method": method,
            "params": params,
        })
        .to_string();
        let headers = Headers::new();
        headers.set("content-type", "application/json")?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        init.with_headers(headers);
        init.with_body(Some(wasm_bindgen::JsValue::from_str(&body)));
        let request = Request::new_with_init(url, &init)?;
        let mut response = Fetch::Request(request).send().await?;
        let text = response.text().await?;
        serde_json::from_str(&text).map_err(|err| Error::RustError(err.to_string()))
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
