mod agents;
mod indexer;
mod live;
#[cfg(target_arch = "wasm32")]
mod live_edge;
mod logic;
#[cfg(target_arch = "wasm32")]
mod markets;
mod skins;
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
            if req.method() == Method::Get {
                if let Ok(url) = req.url() {
                    let market_path = url.path();
                    if market_path == "/v1/markets" || market_path.ends_with("/v1/markets") {
                        let mut dex = "xyz".to_string();
                        for (key, value) in url.query_pairs() {
                            if key == "dex" {
                                dex = value.into_owned();
                            }
                        }
                        let (body, live) = crate::markets::markets(&self.env, &dex).await;
                        return data_response(body, live);
                    }
                    if market_path == "/v1/markets/candles"
                        || market_path.ends_with("/v1/markets/candles")
                    {
                        let mut coin = String::new();
                        let mut interval = "1h".to_string();
                        let mut limit = 200u32;
                        for (key, value) in url.query_pairs() {
                            match key.as_ref() {
                                "coin" => coin = value.into_owned(),
                                "interval" => interval = value.into_owned(),
                                "limit" => {
                                    limit = value.parse::<u32>().unwrap_or(200);
                                }
                                _ => {}
                            }
                        }
                        let (body, live) =
                            crate::markets::candles(&self.env, &coin, &interval, limit).await;
                        return data_response(body, live);
                    }
                    if market_path == "/v1/accounts/hyperliquid"
                        || market_path.ends_with("/v1/accounts/hyperliquid")
                    {
                        let mut address = String::new();
                        for (key, value) in url.query_pairs() {
                            if key == "address" {
                                address = value.trim().to_ascii_lowercase();
                            }
                        }
                        let valid = address.len() == 42
                            && address.starts_with("0x")
                            && address[2..].bytes().all(|byte| byte.is_ascii_hexdigit());
                        if !valid {
                            return with_cors(
                                Response::ok(
                                    r#"{"error":"validation_error","message":"Send address as 0x plus 40 hex characters."}"#,
                                )?
                                .with_status(400),
                                "application/json",
                            );
                        }
                        let (body, live) = crate::markets::account(&address).await;
                        return account_response(body, live);
                    }
                }
            }
            // `Request::path` strips `?window=` and `?now=`. Put them back.
            let route = match req.url() {
                Ok(url) => path_with_query(&path, url.query()),
                Err(_) => path.clone(),
            };
            let method = match req.method() {
                Method::Get => "GET",
                Method::Post => "POST",
                Method::Delete => "DELETE",
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
            if method == "GET"
                && (route == "/v1/admin/status" || route.starts_with("/v1/admin/status?"))
            {
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
            let sql = self.state.storage().sql();
            if ensure_player_sql(&sql).is_ok() {
                if let Ok(Some(book)) = load_player_book(&sql) {
                    arena.book = book;
                }
            }
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
                        let sql = self.state.storage().sql();
                        let _ = ensure_player_sql(&sql)
                            .and_then(|_| save_player_book(&sql, &arena.book));
                        storage.put("arena", &arena).await?;
                    }
                    if let Some(fanout) = fanout {
                        let tag = socket_tag(&fanout.game_id);
                        for socket in self.state.get_websockets_with_tag(&tag) {
                            let _ = socket.send_with_str(&fanout.message);
                        }
                    }
                    let requests: u64 = storage.get("metric_requests").await?.unwrap_or(0);
                    let _ = storage
                        .put("metric_requests", &(requests.saturating_add(1)))
                        .await;
                    if status >= 500 {
                        let errors: u64 = storage.get("metric_errors").await?.unwrap_or(0);
                        let _ = storage
                            .put("metric_errors", &(errors.saturating_add(1)))
                            .await;
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
            let synced = crate::live_edge::refresh(&mut arena).await;
            storage.put("arena", &arena).await?;
            let cron_now = worker::Date::now().as_millis() / 1000;
            let _ = storage.put("metric_cron_last", &cron_now).await;
            let body = serde_json::json!({
                "ok": true,
                "added": added,
                "indexed_height": arena.indexed_height,
                "live_synced": synced,
                "network": "testnet",
            })
            .to_string();
            with_cors(Response::ok(body)?, "application/json")
        }
    }

    fn ensure_player_sql(sql: &worker::SqlStorage) -> Result<()> {
        for statement in [
            "CREATE TABLE IF NOT EXISTS players (agent_id TEXT PRIMARY KEY, handle TEXT UNIQUE NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL, owner_contact TEXT, token_hash TEXT NOT NULL, token_prefix TEXT NOT NULL, status TEXT NOT NULL, created_unix INTEGER NOT NULL, wallets_json TEXT NOT NULL DEFAULT '{}', capabilities_json TEXT NOT NULL DEFAULT '[]', skin_id TEXT NOT NULL DEFAULT 'ingot')",
            "CREATE TABLE IF NOT EXISTS player_queue (arena_id TEXT NOT NULL, agent_id TEXT NOT NULL, queued_unix INTEGER NOT NULL, PRIMARY KEY (arena_id, agent_id))",
            "CREATE TABLE IF NOT EXISTS player_matches (match_id TEXT PRIMARY KEY, arena_id TEXT NOT NULL, status TEXT NOT NULL, turn INTEGER NOT NULL, deadline_unix INTEGER NOT NULL, seats_json TEXT NOT NULL, actions_json TEXT NOT NULL, created_unix INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE IF NOT EXISTS player_seen (agent_id TEXT NOT NULL, client_request_id TEXT NOT NULL, status INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY (agent_id, client_request_id))",
        ] {
            sql.exec(statement, None::<Vec<worker::SqlStorageValue>>)?;
        }
        for statement in [
            "ALTER TABLE players ADD COLUMN wallets_json TEXT NOT NULL DEFAULT '{}'",
            "ALTER TABLE players ADD COLUMN capabilities_json TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE players ADD COLUMN skin_id TEXT NOT NULL DEFAULT 'ingot'",
            "ALTER TABLE player_matches ADD COLUMN created_unix INTEGER NOT NULL DEFAULT 0",
        ] {
            let _ = sql.exec(statement, None::<Vec<worker::SqlStorageValue>>);
        }
        Ok(())
    }

    fn load_player_book(sql: &worker::SqlStorage) -> Result<Option<crate::agents::PlayerBook>> {
        #[derive(serde::Deserialize)]
        struct Count {
            n: i64,
        }
        let count: Count = sql
            .exec(
                "SELECT COUNT(*) AS n FROM players",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .one()?;
        if count.n == 0 {
            return Ok(None);
        }
        #[derive(serde::Deserialize)]
        struct PlayerRow {
            agent_id: String,
            handle: String,
            name: String,
            description: String,
            owner_contact: Option<String>,
            token_hash: String,
            token_prefix: String,
            status: String,
            created_unix: i64,
            wallets_json: String,
            capabilities_json: String,
            skin_id: String,
        }
        #[derive(serde::Deserialize)]
        struct QueueRow {
            arena_id: String,
            agent_id: String,
            queued_unix: i64,
        }
        #[derive(serde::Deserialize)]
        struct MatchRow {
            match_id: String,
            arena_id: String,
            status: String,
            turn: i64,
            deadline_unix: i64,
            seats_json: String,
            actions_json: String,
            created_unix: i64,
        }
        #[derive(serde::Deserialize)]
        struct SeenRow {
            agent_id: String,
            client_request_id: String,
            status: i64,
            body: String,
        }
        let players = sql.exec("SELECT agent_id, handle, name, description, owner_contact, token_hash, token_prefix, status, created_unix, wallets_json, capabilities_json, skin_id FROM players", None::<Vec<worker::SqlStorageValue>>)?.to_array::<PlayerRow>()?;
        let queue = sql
            .exec(
                "SELECT arena_id, agent_id, queued_unix FROM player_queue",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<QueueRow>()?;
        let matches = sql.exec("SELECT match_id, arena_id, status, turn, deadline_unix, seats_json, actions_json, created_unix FROM player_matches", None::<Vec<worker::SqlStorageValue>>)?.to_array::<MatchRow>()?;
        let seen = sql
            .exec(
                "SELECT agent_id, client_request_id, status, body FROM player_seen",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<SeenRow>()?;
        Ok(Some(crate::agents::PlayerBook {
            players: players
                .into_iter()
                .map(|row| crate::agents::Player {
                    agent_id: row.agent_id,
                    handle: row.handle,
                    name: row.name,
                    description: row.description,
                    owner_contact: row.owner_contact,
                    token_hash: row.token_hash,
                    token_prefix: row.token_prefix,
                    status: row.status,
                    created_unix: row.created_unix as u64,
                    wallets: serde_json::from_str(&row.wallets_json).unwrap_or_default(),
                    capabilities: serde_json::from_str(&row.capabilities_json).unwrap_or_default(),
                    skin_id: row.skin_id,
                })
                .collect(),
            queue: queue
                .into_iter()
                .map(|row| crate::agents::Queued {
                    arena_id: row.arena_id,
                    agent_id: row.agent_id,
                    queued_unix: row.queued_unix as u64,
                })
                .collect(),
            matches: matches
                .into_iter()
                .map(|row| crate::agents::PlayMatch {
                    match_id: row.match_id,
                    arena_id: row.arena_id,
                    status: row.status,
                    turn: row.turn as u32,
                    deadline_unix: row.deadline_unix as u64,
                    seats: serde_json::from_str(&row.seats_json).unwrap_or_default(),
                    actions: serde_json::from_str(&row.actions_json).unwrap_or_default(),
                    created_unix: row.created_unix as u64,
                })
                .collect(),
            seen: seen
                .into_iter()
                .map(|row| crate::agents::Seen {
                    agent_id: row.agent_id,
                    client_request_id: row.client_request_id,
                    status: row.status as u16,
                    body: row.body,
                })
                .collect(),
        }))
    }

    fn save_player_book(sql: &worker::SqlStorage, book: &crate::agents::PlayerBook) -> Result<()> {
        for table in ["players", "player_queue", "player_matches", "player_seen"] {
            sql.exec(
                &format!("DELETE FROM {table}"),
                None::<Vec<worker::SqlStorageValue>>,
            )?;
        }
        for player in &book.players {
            sql.exec(
                "INSERT INTO players (agent_id, handle, name, description, owner_contact, token_hash, token_prefix, status, created_unix, wallets_json, capabilities_json, skin_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                Some(vec![
                    player.agent_id.clone().into(),
                    player.handle.clone().into(),
                    player.name.clone().into(),
                    player.description.clone().into(),
                    player.owner_contact.clone().into(),
                    player.token_hash.clone().into(),
                    player.token_prefix.clone().into(),
                    player.status.clone().into(),
                    (player.created_unix as i64).into(),
                    serde_json::to_string(&player.wallets).unwrap_or_else(|_| "{}".into()).into(),
                    serde_json::to_string(&player.capabilities).unwrap_or_else(|_| "[]".into()).into(),
                    player.skin_id.clone().into(),
                ]),
            )?;
        }
        for seat in &book.queue {
            sql.exec(
                "INSERT INTO player_queue (arena_id, agent_id, queued_unix) VALUES (?, ?, ?)",
                Some(vec![
                    seat.arena_id.clone().into(),
                    seat.agent_id.clone().into(),
                    (seat.queued_unix as i64).into(),
                ]),
            )?;
        }
        for play in &book.matches {
            sql.exec(
                "INSERT INTO player_matches (match_id, arena_id, status, turn, deadline_unix, seats_json, actions_json, created_unix) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                Some(vec![
                    play.match_id.clone().into(),
                    play.arena_id.clone().into(),
                    play.status.clone().into(),
                    (play.turn as i64).into(),
                    (play.deadline_unix as i64).into(),
                    serde_json::to_string(&play.seats).unwrap_or_else(|_| "[]".into()).into(),
                    serde_json::to_string(&play.actions).unwrap_or_else(|_| "[]".into()).into(),
                    (play.created_unix as i64).into(),
                ]),
            )?;
        }
        for seen in &book.seen {
            sql.exec(
                "INSERT INTO player_seen (agent_id, client_request_id, status, body) VALUES (?, ?, ?, ?)",
                Some(vec![seen.agent_id.clone().into(), seen.client_request_id.clone().into(), (seen.status as i64).into(), seen.body.clone().into()]),
            )?;
        }
        Ok(())
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

    fn data_response(body: String, live: bool) -> Result<Response> {
        let mut response = Response::ok(body)?;
        response.headers_mut().set(
            "X-Data-Source",
            if live { "hyperliquid" } else { "fixture" },
        )?;
        response
            .headers_mut()
            .set("Cache-Control", "public, max-age=10")?;
        with_cors(response, "application/json")
    }

    fn account_response(body: String, live: bool) -> Result<Response> {
        let mut response = Response::ok(body)?;
        response.headers_mut().set(
            "X-Data-Source",
            if live { "hyperliquid" } else { "unavailable" },
        )?;
        response
            .headers_mut()
            .set("Cache-Control", "public, max-age=5")?;
        with_cors(response, "application/json")
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
