#![recursion_limit = "256"]

mod agents;
mod indexer;
mod limits;
mod live;
#[cfg(target_arch = "wasm32")]
mod live_edge;
mod logic;
#[cfg(target_arch = "wasm32")]
mod markets;
mod passport;
mod skins;
mod surface;
mod vault;

pub use indexer::{apply_chain_events, refuse_testnet};
pub use logic::{dispatch, ingest_allowed, ArenaState, Incoming, Outcome};

#[cfg(target_arch = "wasm32")]
mod edge {
    use base64::Engine as _;
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
            if let Some(response) = self.enforce_rate_limit(&req).await? {
                return Ok(response);
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
            let gate = self
                .env
                .var("FE2VI_PASSPORT_GATE")
                .ok()
                .map(|value| value.to_string());
            arena.passport_gate = crate::passport::PassportGate::parse(gate.as_deref());
            arena.passport_mint = self
                .env
                .var("FE2VI_PASSPORT_MINT")
                .ok()
                .map(|value| {
                    !matches!(
                        value.to_string().trim().to_ascii_lowercase().as_str(),
                        "off" | "false" | "0"
                    )
                })
                .unwrap_or(true);
            arena.chain_network = env_string(&self.env, "FE2VI_NETWORK");
            arena.passport_contract = env_string(&self.env, "FE2VI_PASSPORT_ID");
            arena.market_contract = env_string(&self.env, "FE2VI_PASSPORT_MARKET_ID");
            arena.escrow_contract = env_string(&self.env, "FE2VI_ESCROW_ID");
            let before_book = {
                let sql = self.state.storage().sql();
                attach_player_book(&sql, &mut arena)?
            };
            let route_path = route.split('?').next().unwrap_or(route.as_str());
            if route_path.starts_with("/v1/internal/passport") {
                let key = self
                    .env
                    .var("FE2VI_PASSPORT_RELAY_KEY")
                    .ok()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                let presented = authorization.clone().unwrap_or_default();
                if key.trim().is_empty() || presented != format!("Bearer {key}") {
                    return with_cors(Response::empty()?.with_status(404), "text/plain");
                }
            }
            let policy_target = match (method, route_path) {
                ("POST", "/v1/agents/me/hyperliquid-link") => {
                    let authorized = crate::agents::authed_id(
                        &arena,
                        &Incoming {
                            method,
                            path: &route,
                            body: &body,
                            authorization: authorization.as_deref(),
                            ingest_token: ingest_token.as_deref(),
                            policy_check: None,
                        },
                    )
                    .is_some();
                    if authorized {
                        serde_json::from_str::<serde_json::Value>(&body)
                            .ok()
                            .and_then(|value| {
                                value
                                    .get("nearIntentsAgentId")
                                    .and_then(serde_json::Value::as_str)
                                    .map(str::trim)
                                    .filter(|value| crate::live::valid_near_intents_agent_id(value))
                                    .map(str::to_string)
                            })
                    } else {
                        None
                    }
                }
                ("POST", path)
                    if path.starts_with("/v1/cup/rounds/") && path.ends_with("/join") =>
                {
                    crate::agents::authed_id(
                        &arena,
                        &Incoming {
                            method,
                            path: &route,
                            body: &body,
                            authorization: authorization.as_deref(),
                            ingest_token: ingest_token.as_deref(),
                            policy_check: None,
                        },
                    )
                    .and_then(|agent_id| {
                        arena
                            .live
                            .link(&agent_id)
                            .map(|link| link.near_intents_agent_id.clone())
                    })
                    .map(|value| value.trim().to_string())
                    .filter(|value| crate::live::valid_near_intents_agent_id(value))
                }
                _ => None,
            };
            let policy_check = match policy_target {
                Some(agent_id) => Some(crate::live_edge::policy_check(&self.env, &agent_id).await),
                None => None,
            };
            let live_read = method == "GET"
                && (route.starts_with("/v1/village/")
                    || route.starts_with("/v1/cup/")
                    || route.starts_with("/v1/live/"));
            let now = worker::Date::now().as_millis() / 1000;
            let mut refreshed = false;
            if live_read
                && !arena.live.links.is_empty()
                && arena.live.last_tick_unix.saturating_add(60) < now
            {
                let _ = crate::live_edge::refresh(&mut arena).await;
                refreshed = true;
            }
            let war_secret = self
                .env
                .secret("FE2VI_WAR_SECRET")
                .ok()
                .map(|value| value.to_string())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            if method == "GET"
                && (route_path == "/v1/market/listings" || route_path == "/v1/market/sales")
            {
                return self.market_response(route_path).await;
            }
            if method == "GET" {
                let chain_now = worker::Date::now().as_millis() / 1000;
                if let Some(token_id) = route_path.strip_prefix("/v1/passports/") {
                    if !token_id.is_empty() && !token_id.contains('/') {
                        let _ = reconcile_token(&self.env, &mut arena, token_id, chain_now).await;
                    }
                }
                if route_path == "/v1/agents/me/passport/jobs" {
                    if let Some(agent_id) = crate::agents::authed_id(
                        &arena,
                        &Incoming {
                            method,
                            path: &route,
                            body: &body,
                            authorization: authorization.as_deref(),
                            ingest_token: ingest_token.as_deref(),
                            policy_check: None,
                        },
                    ) {
                        let pending: Vec<(String, String)> = arena
                            .book
                            .passport_jobs
                            .iter()
                            .filter(|job| {
                                job.agent_id == agent_id
                                    && job.kind == "bind"
                                    && job.status == "done"
                            })
                            .filter_map(|job| {
                                arena
                                    .book
                                    .passports
                                    .iter()
                                    .find(|passport| passport.passport_id == job.passport_id)
                                    .and_then(|passport| passport.token_id.clone())
                                    .map(|token_id| (job.job_id.clone(), token_id))
                            })
                            .collect();
                        for (_, token_id) in pending {
                            let _ =
                                reconcile_token(&self.env, &mut arena, &token_id, chain_now).await;
                        }
                    }
                }
                if route_path == "/v1/agents/me/passport" {
                    if let Some(agent_id) = crate::agents::authed_id(
                        &arena,
                        &Incoming {
                            method,
                            path: &route,
                            body: &body,
                            authorization: authorization.as_deref(),
                            ingest_token: ingest_token.as_deref(),
                            policy_check: None,
                        },
                    ) {
                        let token_id = arena
                            .book
                            .passports
                            .iter()
                            .find(|passport| {
                                passport.agent_id == agent_id && passport.status != "unbound"
                            })
                            .and_then(|passport| passport.token_id.clone());
                        if let Some(token_id) = token_id {
                            let _ =
                                reconcile_token(&self.env, &mut arena, &token_id, chain_now).await;
                        }
                    }
                }
            }
            if method == "GET" {
                if let Some(token_id) = route_path.strip_prefix("/v1/passports/") {
                    if !token_id.is_empty()
                        && !token_id.contains('/')
                        && !arena
                            .book
                            .passports
                            .iter()
                            .any(|passport| passport.token_id.as_deref() == Some(token_id))
                    {
                        match passport_chain_view(&self.env, token_id).await {
                            Ok(Some(observed)) => {
                                let Some(view) = observed.view else {
                                    return with_cors(
                                        Response::ok(
                                            serde_json::json!({
                                                "error": "not_found",
                                                "message": "No passport with that token id.",
                                            })
                                            .to_string(),
                                        )?
                                        .with_status(404),
                                        "application/json",
                                    );
                                };
                                let body = crate::passport::chain_passport_json(token_id, &view)
                                    .to_string();
                                return with_cors(
                                    Response::ok(body)?.with_status(200),
                                    "application/json",
                                );
                            }
                            Ok(None) => {}
                            Err(failure) => {
                                let message =
                                    format!("Could not read the passport contract ({failure}).");
                                let body = serde_json::json!({
                                    "error": "passport_lookup_unavailable",
                                    "message": message,
                                    "source": "unavailable",
                                })
                                .to_string();
                                return with_cors(
                                    Response::ok(body)?.with_status(503),
                                    "application/json",
                                );
                            }
                        }
                    }
                }
            }
            crate::logic::set_request_war_secret(war_secret);
            let outcome = dispatch(
                &mut arena,
                &Incoming {
                    method,
                    path: &route,
                    body: &body,
                    authorization: authorization.as_deref(),
                    ingest_token: ingest_token.as_deref(),
                    policy_check: policy_check.as_ref(),
                },
            );
            crate::logic::set_request_war_secret(None);
            match outcome {
                Outcome::Http {
                    status,
                    content_type,
                    mut body,
                    mut mutated,
                    fanout,
                    location,
                } => {
                    if method == "POST" {
                        if let Some(job_id) = route_path
                            .strip_prefix("/v1/internal/passport/jobs/")
                            .and_then(|rest| rest.strip_suffix("/complete"))
                        {
                            if !job_id.is_empty()
                                && !job_id.contains('/')
                                && arena.book.passport_jobs.iter().any(|job| {
                                    job.job_id == job_id
                                        && job.kind == "bind"
                                        && job.status == "done"
                                })
                            {
                                let token_id = arena
                                    .book
                                    .passport_jobs
                                    .iter()
                                    .find(|job| job.job_id == job_id)
                                    .and_then(|job| {
                                        arena.book.passports.iter().find(|passport| {
                                            passport.passport_id == job.passport_id
                                        })
                                    })
                                    .and_then(|passport| passport.token_id.clone());
                                let chain_now = worker::Date::now().as_millis() / 1000;
                                let block_hash = arena
                                    .book
                                    .passport_jobs
                                    .iter()
                                    .find(|job| job.job_id == job_id)
                                    .map(|job| job.tx_block_hash.clone())
                                    .unwrap_or_default();
                                if !block_hash.is_empty() {
                                    if let Ok(height) =
                                        receipt_block_height(&self.env, &block_hash).await
                                    {
                                        if let Some(job) = arena
                                            .book
                                            .passport_jobs
                                            .iter_mut()
                                            .find(|job| job.job_id == job_id)
                                        {
                                            job.tx_block_height = height;
                                        }
                                    }
                                }
                                let changed = match token_id.as_deref() {
                                    Some(token_id) if !block_hash.is_empty() => {
                                        match passport_chain_view_at(
                                            &self.env,
                                            token_id,
                                            Some(&block_hash),
                                        )
                                        .await
                                        {
                                            Ok(Some(observed)) => {
                                                if observed.block_height > 0 {
                                                    if let Some(job) = arena
                                                        .book
                                                        .passport_jobs
                                                        .iter_mut()
                                                        .find(|job| job.job_id == job_id)
                                                    {
                                                        job.tx_block_height = observed.block_height;
                                                    }
                                                }
                                                crate::passport::judge_completed_bind(
                                                    &mut arena,
                                                    job_id,
                                                    observed.view.as_ref(),
                                                    observed.block_height,
                                                    chain_now,
                                                )
                                            }
                                            Ok(None) | Err(_) => false,
                                        }
                                    }
                                    _ => false,
                                };
                                if changed {
                                    body = crate::passport::complete_response(&arena, job_id)
                                        .to_string();
                                    mutated = true;
                                }
                            }
                        }
                    }
                    let plan = crate::agents::plan_writes(&before_book, &arena.book);
                    if mutated || refreshed || !plan.is_empty() {
                        {
                            let sql = self.state.storage().sql();
                            apply_sql_plan(&sql, &plan)?;
                        }
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
            let network = env_string(&self.env, "FE2VI_NETWORK");
            let rpc = env_string(&self.env, "FE2VI_RPC_URL");
            if let Err(message) = indexer::allow_chain_rpc(&network, &rpc) {
                let body = serde_json::json!({"ok": false, "error": message}).to_string();
                return with_cors(Response::ok(body)?.with_status(400), "application/json");
            }
            let storage = self.state.storage();
            let mut arena: ArenaState = storage.get("arena").await?.unwrap_or_default();
            let before_book = {
                let sql = self.state.storage().sql();
                attach_player_book(&sql, &mut arena)?
            };
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
            let chain_now = worker::Date::now().as_millis() / 1000;
            let mut reconciled = 0_u32;
            let tokens: Vec<String> = arena
                .book
                .passports
                .iter()
                .filter(|passport| {
                    passport.status == "active"
                        && passport
                            .token_id
                            .as_deref()
                            .is_some_and(|token| !token.is_empty())
                })
                .filter_map(|passport| passport.token_id.clone())
                .take(8)
                .collect();
            for token_id in tokens {
                if reconcile_token(&self.env, &mut arena, &token_id, chain_now)
                    .await
                    .unwrap_or(false)
                {
                    reconciled += 1;
                }
            }
            let synced = crate::live_edge::refresh(&mut arena).await;
            let policy_revalidated =
                crate::live_edge::refresh_policies(&self.env, &mut arena).await;
            let plan = crate::agents::plan_writes(&before_book, &arena.book);
            {
                let sql = self.state.storage().sql();
                apply_sql_plan(&sql, &plan)?;
            }
            storage.put("arena", &arena).await?;
            let cron_now = worker::Date::now().as_millis() / 1000;
            let _ = storage.put("metric_cron_last", &cron_now).await;
            let body = serde_json::json!({
                "ok": true,
                "added": added,
                "indexed_height": arena.indexed_height,
                "live_synced": synced,
                "policy_revalidated": policy_revalidated,
                "passports_reconciled": reconciled,
                "network": network,
            })
            .to_string();
            with_cors(Response::ok(body)?, "application/json")
        }
    }

    fn ensure_player_sql(sql: &worker::SqlStorage) -> Result<()> {
        for statement in [
            "CREATE TABLE IF NOT EXISTS players (agent_id TEXT PRIMARY KEY, handle TEXT UNIQUE NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL, owner_contact TEXT, token_hash TEXT NOT NULL, token_prefix TEXT NOT NULL, status TEXT NOT NULL, created_unix INTEGER NOT NULL, wallets_json TEXT NOT NULL DEFAULT '{}', capabilities_json TEXT NOT NULL DEFAULT '[]', skin_id TEXT NOT NULL DEFAULT 'ingot')",
            "CREATE TABLE IF NOT EXISTS player_tokens (agent_id TEXT PRIMARY KEY, token_hash TEXT NOT NULL, token_prefix TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS player_queue (arena_id TEXT NOT NULL, agent_id TEXT NOT NULL, queued_unix INTEGER NOT NULL, PRIMARY KEY (arena_id, agent_id))",
            "CREATE TABLE IF NOT EXISTS player_matches (match_id TEXT PRIMARY KEY, arena_id TEXT NOT NULL, status TEXT NOT NULL, turn INTEGER NOT NULL, deadline_unix INTEGER NOT NULL, seats_json TEXT NOT NULL, actions_json TEXT NOT NULL, created_unix INTEGER NOT NULL DEFAULT 0, vault_json TEXT NOT NULL DEFAULT 'null')",
            "CREATE TABLE IF NOT EXISTS player_actions (match_id TEXT NOT NULL, turn INTEGER NOT NULL, agent_id TEXT NOT NULL, action_type TEXT NOT NULL, points INTEGER NOT NULL, unix INTEGER NOT NULL, intent TEXT, outcome TEXT, PRIMARY KEY (match_id, turn, agent_id))",
            "CREATE TABLE IF NOT EXISTS player_seen (agent_id TEXT NOT NULL, client_request_id TEXT NOT NULL, status INTEGER NOT NULL, body TEXT NOT NULL, match_id TEXT NOT NULL DEFAULT '', created_unix INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (agent_id, client_request_id))",
            "CREATE TABLE IF NOT EXISTS player_war_claims (agent_id TEXT NOT NULL, challenge_id TEXT NOT NULL, stage TEXT NOT NULL, points INTEGER NOT NULL, unix INTEGER NOT NULL, proof_hash TEXT NOT NULL, intent TEXT, PRIMARY KEY (agent_id, challenge_id))",
            "CREATE TABLE IF NOT EXISTS player_lobby_posts (id TEXT PRIMARY KEY, agent_id TEXT NOT NULL, kind TEXT NOT NULL, topic TEXT NOT NULL, body TEXT NOT NULL, match_id TEXT, unix INTEGER NOT NULL, hidden INTEGER NOT NULL DEFAULT 0, reports_json TEXT NOT NULL DEFAULT '[]')",
            "CREATE TABLE IF NOT EXISTS player_reactions (target_type TEXT NOT NULL, target_id TEXT NOT NULL, viewer_id TEXT NOT NULL, reaction TEXT NOT NULL, unix INTEGER NOT NULL, PRIMARY KEY (target_type, target_id, viewer_id))",
            "CREATE TABLE IF NOT EXISTS player_passports (passport_id TEXT PRIMARY KEY, token_id TEXT UNIQUE, agent_id TEXT NOT NULL, owner_near TEXT NOT NULL, creator_near TEXT NOT NULL, status TEXT NOT NULL, created_unix INTEGER NOT NULL, updated_unix INTEGER NOT NULL, last_sync_seq INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE IF NOT EXISTS player_passport_jobs (job_id TEXT PRIMARY KEY, kind TEXT NOT NULL, passport_id TEXT NOT NULL, agent_id TEXT NOT NULL, owner_near TEXT NOT NULL, agent_near TEXT NOT NULL, status TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, created_unix INTEGER NOT NULL, updated_unix INTEGER NOT NULL)",
        ] {
            sql.exec(statement, None::<Vec<worker::SqlStorageValue>>)?;
        }
        for statement in [
            "ALTER TABLE players ADD COLUMN wallets_json TEXT NOT NULL DEFAULT '{}'",
            "ALTER TABLE players ADD COLUMN capabilities_json TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE players ADD COLUMN skin_id TEXT NOT NULL DEFAULT 'ingot'",
            "ALTER TABLE players ADD COLUMN war_hits_json TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE player_matches ADD COLUMN created_unix INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE player_matches ADD COLUMN vault_json TEXT NOT NULL DEFAULT 'null'",
            "ALTER TABLE player_lobby_posts ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE player_lobby_posts ADD COLUMN reports_json TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE player_seen ADD COLUMN match_id TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE player_seen ADD COLUMN created_unix INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE player_passports ADD COLUMN career_json TEXT NOT NULL DEFAULT 'null'",
            "ALTER TABLE player_passport_jobs ADD COLUMN tx_hash TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE player_passport_jobs ADD COLUMN error TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE player_passport_jobs ADD COLUMN tx_block_hash TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE player_passport_jobs ADD COLUMN tx_block_height INTEGER NOT NULL DEFAULT 0",
        ] {
            let _ = sql.exec(statement, None::<Vec<worker::SqlStorageValue>>);
        }
        Ok(())
    }

    fn sql_count(sql: &worker::SqlStorage, table: &str) -> Result<i64> {
        #[derive(serde::Deserialize)]
        struct Count {
            n: i64,
        }
        let query = format!("SELECT COUNT(*) AS n FROM {table}");
        let count: Count = sql
            .exec(&query, None::<Vec<worker::SqlStorageValue>>)?
            .one()?;
        Ok(count.n)
    }

    fn attach_player_book(
        sql: &worker::SqlStorage,
        arena: &mut ArenaState,
    ) -> Result<crate::agents::PlayerBook> {
        ensure_player_sql(sql)?;
        if let Some(book) = load_player_book(sql)? {
            arena.book = book;
        }
        let mut before = arena.book.clone();
        migration_baseline(sql, &mut before)?;
        Ok(before)
    }

    /// Rows that still live only in the old columns are cleared on the
    /// baseline so the next plan inserts them once into the new tables.
    fn migration_baseline(
        sql: &worker::SqlStorage,
        before: &mut crate::agents::PlayerBook,
    ) -> Result<()> {
        if sql_count(sql, "player_tokens")? == 0 {
            for player in &mut before.players {
                player.token_hash.clear();
                player.token_prefix.clear();
            }
        }
        if sql_count(sql, "player_actions")? == 0 {
            for play in &mut before.matches {
                play.actions.clear();
            }
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
            #[serde(default)]
            war_hits_json: String,
        }
        #[derive(serde::Deserialize)]
        struct TokenRow {
            agent_id: String,
            token_hash: String,
            token_prefix: String,
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
            vault_json: String,
        }
        #[derive(serde::Deserialize)]
        struct ActionRow {
            match_id: String,
            turn: i64,
            agent_id: String,
            action_type: String,
            points: i64,
            unix: i64,
            intent: Option<String>,
            outcome: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct WarClaimRow {
            agent_id: String,
            challenge_id: String,
            stage: String,
            points: i64,
            unix: i64,
            proof_hash: String,
            intent: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct LobbyPostRow {
            id: String,
            agent_id: String,
            kind: String,
            topic: String,
            body: String,
            match_id: Option<String>,
            unix: i64,
            hidden: i64,
            reports_json: String,
        }
        #[derive(serde::Deserialize)]
        struct ReactionRow {
            target_type: String,
            target_id: String,
            viewer_id: String,
            reaction: String,
            unix: i64,
        }
        #[derive(serde::Deserialize)]
        struct SeenRow {
            agent_id: String,
            client_request_id: String,
            status: i64,
            body: String,
            match_id: String,
            created_unix: i64,
        }
        #[derive(serde::Deserialize)]
        struct PassportRow {
            passport_id: String,
            token_id: Option<String>,
            agent_id: String,
            owner_near: String,
            creator_near: String,
            status: String,
            created_unix: i64,
            updated_unix: i64,
            last_sync_seq: i64,
            #[serde(default)]
            career_json: String,
        }
        #[derive(serde::Deserialize)]
        struct PassportJobRow {
            job_id: String,
            kind: String,
            passport_id: String,
            agent_id: String,
            owner_near: String,
            agent_near: String,
            status: String,
            attempts: i64,
            created_unix: i64,
            updated_unix: i64,
            #[serde(default)]
            tx_hash: String,
            #[serde(default)]
            error: String,
            #[serde(default)]
            tx_block_hash: String,
            #[serde(default)]
            tx_block_height: i64,
        }
        let players = sql.exec("SELECT agent_id, handle, name, description, owner_contact, token_hash, token_prefix, status, created_unix, wallets_json, capabilities_json, skin_id, war_hits_json FROM players", None::<Vec<worker::SqlStorageValue>>)?.to_array::<PlayerRow>()?;
        let tokens = sql
            .exec(
                "SELECT agent_id, token_hash, token_prefix FROM player_tokens",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<TokenRow>()?;
        let mut token_by_agent = std::collections::HashMap::new();
        for row in tokens {
            token_by_agent.insert(row.agent_id, (row.token_hash, row.token_prefix));
        }
        let queue = sql
            .exec(
                "SELECT arena_id, agent_id, queued_unix FROM player_queue",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<QueueRow>()?;
        let matches = sql.exec("SELECT match_id, arena_id, status, turn, deadline_unix, seats_json, actions_json, created_unix, vault_json FROM player_matches", None::<Vec<worker::SqlStorageValue>>)?.to_array::<MatchRow>()?;
        let actions = sql.exec("SELECT match_id, turn, agent_id, action_type, points, unix, intent, outcome FROM player_actions", None::<Vec<worker::SqlStorageValue>>)?.to_array::<ActionRow>()?;
        let mut actions_by_match: std::collections::HashMap<String, Vec<crate::agents::Played>> =
            std::collections::HashMap::new();
        for row in actions {
            actions_by_match
                .entry(row.match_id)
                .or_default()
                .push(crate::agents::Played {
                    agent_id: row.agent_id,
                    turn: row.turn as u32,
                    action_type: row.action_type,
                    points: u32::try_from(row.points.max(0)).unwrap_or(0),
                    unix: row.unix.max(0) as u64,
                    intent: row.intent,
                    outcome: row.outcome,
                });
        }
        let war_claims = sql
            .exec(
                "SELECT agent_id, challenge_id, stage, points, unix, proof_hash, intent FROM player_war_claims",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<WarClaimRow>()?;
        let lobby = sql
            .exec(
                "SELECT id, agent_id, kind, topic, body, match_id, unix, hidden, reports_json FROM player_lobby_posts",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<LobbyPostRow>()?;
        let reactions = sql
            .exec(
                "SELECT target_type, target_id, viewer_id, reaction, unix FROM player_reactions",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<ReactionRow>()?;
        let seen = sql
            .exec(
                "SELECT agent_id, client_request_id, status, body, match_id, created_unix FROM player_seen",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<SeenRow>()?;
        let passports = sql
            .exec(
                "SELECT passport_id, token_id, agent_id, owner_near, creator_near, status, created_unix, updated_unix, last_sync_seq, career_json FROM player_passports",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<PassportRow>()?;
        let passport_jobs = sql
            .exec(
                "SELECT job_id, kind, passport_id, agent_id, owner_near, agent_near, status, attempts, created_unix, updated_unix, tx_hash, error, tx_block_hash, tx_block_height FROM player_passport_jobs",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<PassportJobRow>()?;
        Ok(Some(crate::agents::PlayerBook {
            players: players
                .into_iter()
                .map(|row| {
                    let (token_hash, token_prefix) = token_by_agent
                        .remove(&row.agent_id)
                        .unwrap_or((row.token_hash, row.token_prefix));
                    crate::agents::Player {
                        agent_id: row.agent_id,
                        handle: row.handle,
                        name: row.name,
                        description: row.description,
                        owner_contact: row.owner_contact,
                        token_hash,
                        token_prefix,
                        status: row.status,
                        created_unix: row.created_unix.max(0) as u64,
                        wallets: serde_json::from_str(&row.wallets_json).unwrap_or_default(),
                        capabilities: serde_json::from_str(&row.capabilities_json)
                            .unwrap_or_default(),
                        skin_id: row.skin_id,
                        war_hits: serde_json::from_str(&row.war_hits_json).unwrap_or_default(),
                    }
                })
                .collect(),
            queue: queue
                .into_iter()
                .map(|row| crate::agents::Queued {
                    arena_id: row.arena_id,
                    agent_id: row.agent_id,
                    queued_unix: row.queued_unix.max(0) as u64,
                })
                .collect(),
            matches: matches
                .into_iter()
                .map(|row| {
                    let mut actions = serde_json::from_str(&row.actions_json).unwrap_or_default();
                    if let Some(rows) = actions_by_match.remove(&row.match_id) {
                        if !rows.is_empty() {
                            actions = rows;
                        }
                    }
                    crate::agents::PlayMatch {
                        match_id: row.match_id,
                        arena_id: row.arena_id,
                        status: row.status,
                        turn: row.turn.max(0) as u32,
                        deadline_unix: row.deadline_unix.max(0) as u64,
                        seats: serde_json::from_str(&row.seats_json).unwrap_or_default(),
                        actions,
                        created_unix: row.created_unix.max(0) as u64,
                        vault: serde_json::from_str(&row.vault_json).unwrap_or(None),
                    }
                })
                .collect(),
            war_claims: war_claims
                .into_iter()
                .map(|row| crate::agents::WarClaim {
                    agent_id: row.agent_id,
                    challenge_id: row.challenge_id,
                    stage: row.stage,
                    points: u32::try_from(row.points.max(0)).unwrap_or(0),
                    unix: row.unix.max(0) as u64,
                    proof_hash: row.proof_hash,
                    intent: row.intent,
                })
                .collect(),
            lobby: lobby
                .into_iter()
                .map(|row| crate::agents::LobbyPost {
                    id: row.id,
                    agent_id: row.agent_id,
                    kind: row.kind,
                    topic: row.topic,
                    body: row.body,
                    match_id: row.match_id,
                    unix: row.unix.max(0) as u64,
                    hidden: row.hidden != 0,
                    reports: serde_json::from_str(&row.reports_json).unwrap_or_default(),
                })
                .collect(),
            reactions: reactions
                .into_iter()
                .map(|row| crate::agents::Reaction {
                    target_type: row.target_type,
                    target_id: row.target_id,
                    viewer_id: row.viewer_id,
                    reaction: row.reaction,
                    unix: row.unix.max(0) as u64,
                })
                .collect(),
            seen: seen
                .into_iter()
                .map(|row| crate::agents::Seen {
                    agent_id: row.agent_id,
                    client_request_id: row.client_request_id,
                    status: row.status as u16,
                    body: row.body,
                    match_id: row.match_id,
                    created_unix: row.created_unix.max(0) as u64,
                })
                .collect(),
            passports: passports
                .into_iter()
                .map(|row| crate::agents::Passport {
                    passport_id: row.passport_id,
                    token_id: row.token_id,
                    agent_id: row.agent_id,
                    owner_near: row.owner_near,
                    creator_near: row.creator_near,
                    status: row.status,
                    created_unix: row.created_unix.max(0) as u64,
                    updated_unix: row.updated_unix.max(0) as u64,
                    last_sync_seq: row.last_sync_seq.max(0) as u64,
                    career: serde_json::from_str(&row.career_json).unwrap_or(None),
                })
                .collect(),
            passport_jobs: passport_jobs
                .into_iter()
                .map(|row| crate::agents::PassportJob {
                    job_id: row.job_id,
                    kind: row.kind,
                    passport_id: row.passport_id,
                    agent_id: row.agent_id,
                    owner_near: row.owner_near,
                    agent_near: row.agent_near,
                    status: row.status,
                    attempts: u32::try_from(row.attempts.max(0)).unwrap_or(0),
                    created_unix: row.created_unix.max(0) as u64,
                    updated_unix: row.updated_unix.max(0) as u64,
                    tx_hash: row.tx_hash,
                    error: row.error,
                    tx_block_hash: row.tx_block_hash,
                    tx_block_height: row.tx_block_height.max(0) as u64,
                })
                .collect(),
        }))
    }

    fn apply_sql_plan(sql: &worker::SqlStorage, plan: &crate::agents::SqlPlan) -> Result<()> {
        for (match_id, turn, agent_id) in &plan.delete_actions {
            sql.exec(
                "DELETE FROM player_actions WHERE match_id = ? AND turn = ? AND agent_id = ?",
                Some(vec![
                    match_id.clone().into(),
                    (*turn as i64).into(),
                    agent_id.clone().into(),
                ]),
            )?;
        }
        for match_id in &plan.delete_match_ids {
            sql.exec(
                "DELETE FROM player_actions WHERE match_id = ?",
                Some(vec![match_id.clone().into()]),
            )?;
            sql.exec(
                "DELETE FROM player_matches WHERE match_id = ?",
                Some(vec![match_id.clone().into()]),
            )?;
        }
        for (arena_id, agent_id) in &plan.delete_queue {
            sql.exec(
                "DELETE FROM player_queue WHERE arena_id = ? AND agent_id = ?",
                Some(vec![arena_id.clone().into(), agent_id.clone().into()]),
            )?;
        }
        for (agent_id, request_id) in &plan.delete_seen {
            sql.exec(
                "DELETE FROM player_seen WHERE agent_id = ? AND client_request_id = ?",
                Some(vec![agent_id.clone().into(), request_id.clone().into()]),
            )?;
        }
        for (agent_id, challenge_id) in &plan.delete_war {
            sql.exec(
                "DELETE FROM player_war_claims WHERE agent_id = ? AND challenge_id = ?",
                Some(vec![agent_id.clone().into(), challenge_id.clone().into()]),
            )?;
        }
        for id in &plan.delete_lobby {
            sql.exec(
                "DELETE FROM player_lobby_posts WHERE id = ?",
                Some(vec![id.clone().into()]),
            )?;
        }
        for (target_type, target_id, viewer_id) in &plan.delete_reactions {
            sql.exec(
                "DELETE FROM player_reactions WHERE target_type = ? AND target_id = ? AND viewer_id = ?",
                Some(vec![
                    target_type.clone().into(),
                    target_id.clone().into(),
                    viewer_id.clone().into(),
                ]),
            )?;
        }
        for passport_id in &plan.delete_passport_ids {
            sql.exec(
                "DELETE FROM player_passports WHERE passport_id = ?",
                Some(vec![passport_id.clone().into()]),
            )?;
        }
        for job_id in &plan.delete_passport_job_ids {
            sql.exec(
                "DELETE FROM player_passport_jobs WHERE job_id = ?",
                Some(vec![job_id.clone().into()]),
            )?;
        }
        for agent_id in &plan.delete_player_ids {
            sql.exec(
                "DELETE FROM player_tokens WHERE agent_id = ?",
                Some(vec![agent_id.clone().into()]),
            )?;
            sql.exec(
                "DELETE FROM players WHERE agent_id = ?",
                Some(vec![agent_id.clone().into()]),
            )?;
        }
        for player in &plan.upsert_players {
            let wallets = serde_json::to_string(&player.wallets).unwrap_or_else(|_| "{}".into());
            let capabilities =
                serde_json::to_string(&player.capabilities).unwrap_or_else(|_| "[]".into());
            let war_hits = serde_json::to_string(&player.war_hits).unwrap_or_else(|_| "[]".into());
            sql.exec(
                "INSERT INTO players (agent_id, handle, name, description, owner_contact, token_hash, token_prefix, status, created_unix, wallets_json, capabilities_json, skin_id, war_hits_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(agent_id) DO UPDATE SET handle = excluded.handle, name = excluded.name, description = excluded.description, owner_contact = excluded.owner_contact, token_hash = excluded.token_hash, token_prefix = excluded.token_prefix, status = excluded.status, created_unix = excluded.created_unix, wallets_json = excluded.wallets_json, capabilities_json = excluded.capabilities_json, skin_id = excluded.skin_id, war_hits_json = excluded.war_hits_json",
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
                    wallets.into(),
                    capabilities.into(),
                    player.skin_id.clone().into(),
                    war_hits.into(),
                ]),
            )?;
            sql.exec(
                "INSERT INTO player_tokens (agent_id, token_hash, token_prefix) VALUES (?, ?, ?) ON CONFLICT(agent_id) DO UPDATE SET token_hash = excluded.token_hash, token_prefix = excluded.token_prefix",
                Some(vec![
                    player.agent_id.clone().into(),
                    player.token_hash.clone().into(),
                    player.token_prefix.clone().into(),
                ]),
            )?;
        }
        for seat in &plan.upsert_queue {
            sql.exec(
                "INSERT INTO player_queue (arena_id, agent_id, queued_unix) VALUES (?, ?, ?) ON CONFLICT(arena_id, agent_id) DO UPDATE SET queued_unix = excluded.queued_unix",
                Some(vec![
                    seat.arena_id.clone().into(),
                    seat.agent_id.clone().into(),
                    (seat.queued_unix as i64).into(),
                ]),
            )?;
        }
        for play in &plan.upsert_matches {
            let seats = serde_json::to_string(&play.seats).unwrap_or_else(|_| "[]".into());
            let actions = serde_json::to_string(&play.actions).unwrap_or_else(|_| "[]".into());
            let vault = serde_json::to_string(&play.vault).unwrap_or_else(|_| "null".into());
            sql.exec(
                "INSERT INTO player_matches (match_id, arena_id, status, turn, deadline_unix, seats_json, actions_json, created_unix, vault_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(match_id) DO UPDATE SET arena_id = excluded.arena_id, status = excluded.status, turn = excluded.turn, deadline_unix = excluded.deadline_unix, seats_json = excluded.seats_json, actions_json = excluded.actions_json, created_unix = excluded.created_unix, vault_json = excluded.vault_json",
                Some(vec![
                    play.match_id.clone().into(),
                    play.arena_id.clone().into(),
                    play.status.clone().into(),
                    (play.turn as i64).into(),
                    (play.deadline_unix as i64).into(),
                    seats.into(),
                    actions.into(),
                    (play.created_unix as i64).into(),
                    vault.into(),
                ]),
            )?;
        }
        for (match_id, action) in &plan.upsert_actions {
            sql.exec(
                "INSERT INTO player_actions (match_id, turn, agent_id, action_type, points, unix, intent, outcome) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(match_id, turn, agent_id) DO UPDATE SET action_type = excluded.action_type, points = excluded.points, unix = excluded.unix, intent = excluded.intent, outcome = excluded.outcome",
                Some(vec![
                    match_id.clone().into(),
                    (action.turn as i64).into(),
                    action.agent_id.clone().into(),
                    action.action_type.clone().into(),
                    (action.points as i64).into(),
                    (action.unix as i64).into(),
                    action.intent.clone().into(),
                    action.outcome.clone().into(),
                ]),
            )?;
        }
        for seen in &plan.upsert_seen {
            sql.exec(
                "INSERT INTO player_seen (agent_id, client_request_id, status, body, match_id, created_unix) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(agent_id, client_request_id) DO UPDATE SET status = excluded.status, body = excluded.body, match_id = excluded.match_id, created_unix = excluded.created_unix",
                Some(vec![
                    seen.agent_id.clone().into(),
                    seen.client_request_id.clone().into(),
                    (seen.status as i64).into(),
                    seen.body.clone().into(),
                    seen.match_id.clone().into(),
                    (seen.created_unix as i64).into(),
                ]),
            )?;
        }
        for claim in &plan.upsert_war {
            sql.exec(
                "INSERT INTO player_war_claims (agent_id, challenge_id, stage, points, unix, proof_hash, intent) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(agent_id, challenge_id) DO UPDATE SET stage = excluded.stage, points = excluded.points, unix = excluded.unix, proof_hash = excluded.proof_hash, intent = excluded.intent",
                Some(vec![
                    claim.agent_id.clone().into(),
                    claim.challenge_id.clone().into(),
                    claim.stage.clone().into(),
                    (claim.points as i64).into(),
                    (claim.unix as i64).into(),
                    claim.proof_hash.clone().into(),
                    claim.intent.clone().into(),
                ]),
            )?;
        }
        for post in &plan.upsert_lobby {
            let reports = serde_json::to_string(&post.reports).unwrap_or_else(|_| "[]".into());
            sql.exec(
                "INSERT INTO player_lobby_posts (id, agent_id, kind, topic, body, match_id, unix, hidden, reports_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET agent_id = excluded.agent_id, kind = excluded.kind, topic = excluded.topic, body = excluded.body, match_id = excluded.match_id, unix = excluded.unix, hidden = excluded.hidden, reports_json = excluded.reports_json",
                Some(vec![
                    post.id.clone().into(),
                    post.agent_id.clone().into(),
                    post.kind.clone().into(),
                    post.topic.clone().into(),
                    post.body.clone().into(),
                    post.match_id.clone().into(),
                    (post.unix as i64).into(),
                    (i64::from(post.hidden)).into(),
                    reports.into(),
                ]),
            )?;
        }
        for reaction in &plan.upsert_reactions {
            sql.exec(
                "INSERT INTO player_reactions (target_type, target_id, viewer_id, reaction, unix) VALUES (?, ?, ?, ?, ?) ON CONFLICT(target_type, target_id, viewer_id) DO UPDATE SET reaction = excluded.reaction, unix = excluded.unix",
                Some(vec![
                    reaction.target_type.clone().into(),
                    reaction.target_id.clone().into(),
                    reaction.viewer_id.clone().into(),
                    reaction.reaction.clone().into(),
                    (reaction.unix as i64).into(),
                ]),
            )?;
        }
        for row in &plan.upsert_passports {
            sql.exec(
                "INSERT INTO player_passports (passport_id, token_id, agent_id, owner_near, creator_near, status, created_unix, updated_unix, last_sync_seq, career_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(passport_id) DO UPDATE SET token_id = excluded.token_id, agent_id = excluded.agent_id, owner_near = excluded.owner_near, creator_near = excluded.creator_near, status = excluded.status, created_unix = excluded.created_unix, updated_unix = excluded.updated_unix, last_sync_seq = excluded.last_sync_seq, career_json = excluded.career_json",
                Some(vec![
                    row.passport_id.clone().into(),
                    row.token_id.clone().into(),
                    row.agent_id.clone().into(),
                    row.owner_near.clone().into(),
                    row.creator_near.clone().into(),
                    row.status.clone().into(),
                    (row.created_unix as i64).into(),
                    (row.updated_unix as i64).into(),
                    (row.last_sync_seq as i64).into(),
                    serde_json::to_string(&row.career).unwrap_or_else(|_| "null".into()).into(),
                ]),
            )?;
        }
        for row in &plan.upsert_passport_jobs {
            sql.exec(
                "INSERT INTO player_passport_jobs (job_id, kind, passport_id, agent_id, owner_near, agent_near, status, attempts, created_unix, updated_unix, tx_hash, error, tx_block_hash, tx_block_height) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(job_id) DO UPDATE SET kind = excluded.kind, passport_id = excluded.passport_id, agent_id = excluded.agent_id, owner_near = excluded.owner_near, agent_near = excluded.agent_near, status = excluded.status, attempts = excluded.attempts, created_unix = excluded.created_unix, updated_unix = excluded.updated_unix, tx_hash = excluded.tx_hash, error = excluded.error, tx_block_hash = excluded.tx_block_hash, tx_block_height = excluded.tx_block_height",
                Some(vec![
                    row.job_id.clone().into(),
                    row.kind.clone().into(),
                    row.passport_id.clone().into(),
                    row.agent_id.clone().into(),
                    row.owner_near.clone().into(),
                    row.agent_near.clone().into(),
                    row.status.clone().into(),
                    (row.attempts as i64).into(),
                    (row.created_unix as i64).into(),
                    (row.updated_unix as i64).into(),
                    row.tx_hash.clone().into(),
                    row.error.clone().into(),
                    row.tx_block_hash.clone().into(),
                    (row.tx_block_height as i64).into(),
                ]),
            )?;
        }
        Ok(())
    }

    fn watch_accounts(env: &Env) -> Vec<String> {
        [
            "FE2VI_PASSPORT_ID",
            "FE2VI_PASSPORT_MARKET_ID",
            "FE2VI_ESCROW_ID",
            "FE2VI_FACTORY_ID",
            "FE2VI_REGISTRY_ID",
            "FE2VI_HEIST_ID",
        ]
        .into_iter()
        .filter_map(|name| {
            let value = env_string(env, name);
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
        })
        .collect()
    }

    async fn rpc_view(
        url: &str,
        account_id: &str,
        method_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&args).map_err(|err| Error::RustError(err.to_string()))?);
        let (status, text) = rpc_exchange(
            url,
            "query",
            serde_json::json!({
                "request_type": "call_function",
                "finality": "final",
                "account_id": account_id,
                "method_name": method_name,
                "args_base64": encoded,
            }),
        )
        .await?;
        indexer::interpret_rpc_query(status, &text)
            .map_err(|failure| Error::RustError(failure.label()))
    }

    async fn rpc_exchange(
        url: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(u16, String)> {
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
        let status = response.status_code();
        let text = response.text().await?;
        Ok((status, text))
    }

    async fn rpc_call(
        url: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let (status, text) = rpc_exchange(url, method, params).await?;
        if status >= 400 {
            return Err(Error::RustError(format!("http {status}")));
        }
        serde_json::from_str(&text).map_err(|_| Error::RustError("decode".into()))
    }

    async fn reconcile_token(
        env: &Env,
        arena: &mut ArenaState,
        token_id: &str,
        now: u64,
    ) -> Result<bool> {
        let Some(index) = arena
            .book
            .passports
            .iter()
            .position(|passport| passport.token_id.as_deref() == Some(token_id))
        else {
            return Ok(false);
        };
        let Some(observed) = passport_chain_view(env, token_id).await? else {
            return Ok(false);
        };
        let Some(view) = observed.view else {
            return Ok(false);
        };
        let book = &mut arena.book;
        let changed = crate::passport::reconcile_passport(
            &mut book.passports[index],
            &view,
            now,
            observed.block_height,
            &mut book.passport_jobs,
        );
        Ok(changed)
    }

    struct ObservedPassport {
        view: Option<serde_json::Value>,
        block_height: u64,
    }

    async fn passport_chain_view(env: &Env, token_id: &str) -> Result<Option<ObservedPassport>> {
        passport_chain_view_at(env, token_id, None).await
    }

    async fn passport_chain_view_at(
        env: &Env,
        token_id: &str,
        block_id: Option<&str>,
    ) -> Result<Option<ObservedPassport>> {
        let network = env_string(env, "FE2VI_NETWORK");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        indexer::allow_chain_rpc(&network, &rpc)
            .map_err(|message| Error::RustError(message.to_string()))?;
        let account_id = env_string(env, "FE2VI_PASSPORT_ID");
        if account_id.is_empty() {
            return Err(Error::RustError(
                "passport contract is not configured".into(),
            ));
        }
        let args = serde_json::to_vec(&serde_json::json!({ "token_id": token_id }))
            .map_err(|err| Error::RustError(err.to_string()))?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(args);
        let mut params = serde_json::json!({
            "request_type": "call_function",
            "account_id": account_id,
            "method_name": "passport",
            "args_base64": encoded,
        });
        if let Some(block_id) = block_id {
            params["block_id"] = serde_json::json!(block_id);
        } else {
            params["finality"] = serde_json::json!("final");
        }
        let (status, text) = rpc_exchange(&rpc, "query", params)
            .await
            .map_err(|_| Error::RustError("transport".into()))?;
        let envelope: serde_json::Value =
            serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        let block_height = envelope
            .pointer("/result/block_height")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        match indexer::interpret_rpc_query(status, &text) {
            Ok(value) if value.is_null() => Ok(Some(ObservedPassport {
                view: None,
                block_height,
            })),
            Ok(value) => Ok(Some(ObservedPassport {
                view: Some(value),
                block_height,
            })),
            Err(failure) => Err(Error::RustError(failure.label())),
        }
    }

    async fn receipt_block_height(env: &Env, block_hash: &str) -> Result<u64> {
        let network = env_string(env, "FE2VI_NETWORK");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        indexer::allow_chain_rpc(&network, &rpc)
            .map_err(|message| Error::RustError(message.to_string()))?;
        let block = rpc_call(&rpc, "block", serde_json::json!({"block_id": block_hash})).await?;
        indexer::block_height(&block).ok_or_else(|| Error::RustError("block height missing".into()))
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

    impl SpectatorFeed {
        async fn market_response(&self, path: &str) -> Result<Response> {
            let network = env_string(&self.env, "FE2VI_NETWORK");
            let rpc = env_string(&self.env, "FE2VI_RPC_URL");
            if let Err(message) = indexer::allow_chain_rpc(&network, &rpc) {
                return market_error(403, "chain_mismatch", message);
            }
            let passport = env_string(&self.env, "FE2VI_PASSPORT_ID");
            let market = env_string(&self.env, "FE2VI_PASSPORT_MARKET_ID");
            if passport.is_empty() || market.is_empty() {
                return market_error(
                    503,
                    "market_unconfigured",
                    "Passport or market account is not configured.",
                );
            }
            if path.ends_with("/sales") {
                let settlement =
                    match rpc_view(&rpc, &market, "last_settlement", serde_json::json!({})).await {
                        Ok(value) if value.is_null() => None,
                        Ok(value) => Some(value),
                        Err(error) => {
                            return market_error(
                                503,
                                "market_unavailable",
                                &format!("Could not read the last settlement ({error})."),
                            )
                        }
                    };
                let body = crate::passport::market_from_chain(
                    &market,
                    &network,
                    &[],
                    settlement.as_ref(),
                    "chain",
                );
                let payload = serde_json::json!({
                    "sales": body["sales"],
                    "source": "chain",
                    "contract": market,
                    "network": network,
                });
                return with_cors(Response::ok(payload.to_string())?, "application/json");
            }
            let tokens = match rpc_view(
                &rpc,
                &passport,
                "nft_tokens",
                serde_json::json!({"from_index":"0","limit":32}),
            )
            .await
            {
                Ok(value) => value,
                Err(error) => {
                    return market_error(
                        503,
                        "market_unavailable",
                        &format!("Could not read passport tokens ({error})."),
                    );
                }
            };
            let Some(tokens) = tokens.as_array() else {
                return market_error(
                    503,
                    "market_unavailable",
                    "Passport token list was not an array.",
                );
            };
            let mut listed = Vec::new();
            for token in tokens {
                let Some(token_id) = token.get("token_id").and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                match rpc_view(
                    &rpc,
                    &market,
                    "get_listing",
                    serde_json::json!({"token_id": token_id}),
                )
                .await
                {
                    Ok(listing) => {
                        if let Some(row) = crate::passport::listed_passport(token, &listing) {
                            listed.push(row);
                        }
                    }
                    Err(error) => {
                        return market_error(
                            503,
                            "market_unavailable",
                            &format!("Could not read a market listing ({error})."),
                        );
                    }
                }
            }
            let settlement =
                match rpc_view(&rpc, &market, "last_settlement", serde_json::json!({})).await {
                    Ok(value) if value.is_null() => None,
                    Ok(value) => Some(value),
                    Err(error) => {
                        return market_error(
                            503,
                            "market_unavailable",
                            &format!("Could not read the last settlement ({error})."),
                        )
                    }
                };
            let body = crate::passport::market_from_chain(
                &market,
                &network,
                &listed,
                settlement.as_ref(),
                "chain",
            );
            let payload = if path.ends_with("/sales") {
                serde_json::json!({
                    "sales": body["sales"],
                    "source": "chain",
                    "contract": market,
                    "network": network,
                })
            } else {
                body
            };
            with_cors(Response::ok(payload.to_string())?, "application/json")
        }

        async fn enforce_rate_limit(&self, req: &Request) -> Result<Option<Response>> {
            let method = match req.method() {
                Method::Get => "GET",
                Method::Post => "POST",
                Method::Delete => "DELETE",
                Method::Put => "PUT",
                Method::Patch => "PATCH",
                Method::Options => return Ok(None),
                _ => "OTHER",
            };
            let authorization = req.headers().get("authorization")?;
            let ip = req.headers().get("cf-connecting-ip")?.unwrap_or_default();
            match crate::limits::classify(method, &req.path(), authorization.as_deref(), &ip) {
                crate::limits::LimitPlan::Exempt => Ok(None),
                crate::limits::LimitPlan::Register { ip } => self.limit_register(&ip).await,
                crate::limits::LimitPlan::Binding {
                    name,
                    key,
                    retry_after,
                } => self.limit_binding(name, &key, retry_after).await,
            }
        }

        async fn limit_register(&self, ip: &str) -> Result<Option<Response>> {
            let now = worker::Date::now().as_millis() / 1000;
            let open = {
                let sql = self.state.storage().sql();
                ensure_rate_sql(&sql)?;
                register_slot_open(&sql, ip, now)?
            };
            if let Some(retry_after) = open {
                return Ok(Some(rate_limited(retry_after)?));
            }
            if let Some(response) = self
                .limit_binding(
                    crate::limits::REGISTER_BINDING,
                    ip,
                    crate::limits::REGISTER_BURST_PERIOD_SECS,
                )
                .await?
            {
                return Ok(Some(response));
            }
            let sql = self.state.storage().sql();
            note_register(&sql, ip, now)?;
            Ok(None)
        }

        async fn limit_binding(
            &self,
            name: &str,
            key: &str,
            retry_after: u64,
        ) -> Result<Option<Response>> {
            let known = crate::limits::BINDINGS
                .iter()
                .any(|row| row.0 == name && row.1 > 0 && row.2 == retry_after);
            if !known {
                return limiter_unavailable();
            }
            let limiter = match self.env.rate_limiter(name) {
                Ok(limiter) => limiter,
                Err(_) => return limiter_unavailable(),
            };
            match limiter.limit(key.to_string()).await {
                Ok(outcome) if outcome.success => Ok(None),
                Ok(_) => Ok(Some(rate_limited(retry_after)?)),
                Err(_) => limiter_unavailable(),
            }
        }
    }

    fn rate_limited(retry_after: u64) -> Result<Response> {
        let mut response =
            Response::ok(r#"{"error":"rate_limited","message":"Too many requests."}"#)?
                .with_status(429);
        response
            .headers_mut()
            .set("Retry-After", &retry_after.to_string())?;
        with_cors(response, "application/json")
    }

    fn limiter_unavailable() -> Result<Option<Response>> {
        let response = Response::ok(
            r#"{"error":"unavailable","message":"The rate limiter is not available."}"#,
        )?
        .with_status(503);
        Ok(Some(with_cors(response, "application/json")?))
    }

    fn ensure_rate_sql(sql: &worker::SqlStorage) -> Result<()> {
        sql.exec(
            "CREATE TABLE IF NOT EXISTS rate_register (ip TEXT NOT NULL, bucket INTEGER NOT NULL, n INTEGER NOT NULL, PRIMARY KEY (ip, bucket))",
            None::<Vec<worker::SqlStorageValue>>,
        )?;
        Ok(())
    }

    fn register_slot_open(sql: &worker::SqlStorage, ip: &str, now: u64) -> Result<Option<u64>> {
        #[derive(serde::Deserialize)]
        struct Row {
            n: i64,
        }
        let bucket = (now / crate::limits::REGISTER_WINDOW_SECS) as i64;
        let day = (now / crate::limits::REGISTER_DAY_SECS) as i64;
        sql.exec(
            "DELETE FROM rate_register WHERE bucket < ?",
            Some(vec![(bucket - 1).into()]),
        )?;
        sql.exec(
            "CREATE TABLE IF NOT EXISTS rate_register_day (ip TEXT NOT NULL, bucket INTEGER NOT NULL, n INTEGER NOT NULL, PRIMARY KEY (ip, bucket))",
            None::<Vec<worker::SqlStorageValue>>,
        )?;
        let hour_rows = sql
            .exec(
                "SELECT n FROM rate_register WHERE ip = ? AND bucket = ?",
                Some(vec![ip.to_string().into(), bucket.into()]),
            )?
            .to_array::<Row>()?;
        let day_rows = sql
            .exec(
                "SELECT n FROM rate_register_day WHERE ip = ? AND bucket = ?",
                Some(vec![ip.to_string().into(), day.into()]),
            )?
            .to_array::<Row>()?;
        let hour_n = hour_rows.first().map(|row| row.n).unwrap_or(0);
        let day_n = day_rows.first().map(|row| row.n).unwrap_or(0);
        if day_n >= crate::limits::REGISTER_PER_DAY as i64 {
            return Ok(Some(crate::limits::register_day_retry(now)));
        }
        if hour_n >= crate::limits::REGISTER_PER_HOUR as i64 {
            return Ok(Some(crate::limits::register_retry_after(now)));
        }
        Ok(None)
    }

    fn note_register(sql: &worker::SqlStorage, ip: &str, now: u64) -> Result<()> {
        let bucket = (now / crate::limits::REGISTER_WINDOW_SECS) as i64;
        let day = (now / crate::limits::REGISTER_DAY_SECS) as i64;
        sql.exec(
            "INSERT INTO rate_register (ip, bucket, n) VALUES (?, ?, 1) ON CONFLICT(ip, bucket) DO UPDATE SET n = n + 1",
            Some(vec![ip.to_string().into(), bucket.into()]),
        )?;
        sql.exec(
            "INSERT INTO rate_register_day (ip, bucket, n) VALUES (?, ?, 1) ON CONFLICT(ip, bucket) DO UPDATE SET n = n + 1",
            Some(vec![ip.to_string().into(), day.into()]),
        )?;
        Ok(())
    }

    fn env_string(env: &Env, name: &str) -> String {
        env.var(name)
            .ok()
            .map(|value| value.to_string())
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn market_error(status: u16, code: &str, message: &str) -> Result<Response> {
        let body =
            serde_json::json!({ "error": code, "message": message, "source": "unavailable" })
                .to_string();
        with_cors(Response::ok(body)?.with_status(status), "application/json")
    }

    fn with_cors(mut response: Response, content_type: &str) -> Result<Response> {
        let headers = response.headers_mut();
        headers.set("content-type", content_type)?;
        headers.set("access-control-allow-origin", "*")?;
        headers.set("access-control-allow-methods", "GET, POST, DELETE, OPTIONS")?;
        headers.set(
            "access-control-expose-headers",
            "Retry-After, X-Fe2vi-Build",
        )?;
        headers.set("x-fe2vi-build", crate::logic::BUILD_SHA)?;
        headers.set(
            "access-control-allow-headers",
            "content-type, authorization",
        )?;
        Ok(response)
    }
}
