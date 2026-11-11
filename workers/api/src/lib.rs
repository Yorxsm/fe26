#![recursion_limit = "256"]

mod agents;
mod auto_settle;
mod indexer;
mod limits;
mod live;
#[cfg(target_arch = "wasm32")]
mod live_edge;
mod logic;
mod market_batch;
#[cfg(target_arch = "wasm32")]
mod markets;
mod near_sign;
mod passport;
mod rules;
mod skins;
mod strategy;
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
            if method == "POST" {
                if let Some(match_id) = vault_receipt_match(route_path) {
                    let key = self
                        .env
                        .var("FE2VI_SETTLER_KEY")
                        .ok()
                        .map(|value| value.to_string())
                        .unwrap_or_default();
                    let presented = authorization.clone().unwrap_or_default();
                    if key.trim().is_empty() || presented != format!("Bearer {key}") {
                        return with_cors(Response::empty()?.with_status(404), "text/plain");
                    }
                    let (status, response_body, mutated) =
                        vault_receipt(self, &mut arena, match_id, &body).await?;
                    if mutated {
                        let plan = crate::agents::plan_writes(&before_book, &arena.book);
                        if !plan.is_empty() {
                            let sql = self.state.storage().sql();
                            apply_sql_plan(&sql, &plan)?;
                        }
                        storage.put("arena", &arena).await?;
                    }
                    return with_cors(
                        Response::ok(response_body)?.with_status(status),
                        "application/json",
                    );
                }
            }
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
            if method == "GET" && route_path == "/v1/passports" {
                return passport_list_response(&self.env).await;
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
                                let mut body =
                                    crate::passport::chain_passport_json(token_id, &view);
                                crate::strategy::stamp_public(&mut body, &arena.book, token_id);
                                let body =
                                    with_chain_playbook(&self.env, token_id, body.to_string())
                                        .await;
                                let body = overlay_recorder(&self.env, token_id, body).await;
                                return with_cors(
                                    Response::ok(body)?.with_status(200),
                                    "application/json",
                                );
                            }
                            Ok(None) => {}
                            Err(failure) => {
                                if let Some(response) =
                                    recorder_passport_response(&self.env, &arena.book, token_id)
                                        .await?
                                {
                                    return Ok(response);
                                }
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
            let chain_now = worker::Date::now().as_millis() / 1000;
            let ending =
                crate::passport::seats_pending_chain_read(&arena, chain_now, method, route_path);
            let chain_reads = if ending.is_empty() {
                None
            } else {
                Some(read_match_end_bindings(&self.env, &ending).await)
            };
            crate::passport::set_match_end_chain(chain_reads);
            crate::strategy::set_strategy_key(None);
            crate::strategy::clear_chain_owners();
            let mut signed_submit: Option<serde_json::Value> = None;
            if method == "POST" && route_path == "/v1/intents/act" {
                let request: serde_json::Value =
                    serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
                let host = intents_submit_host(&self.env);
                let key_present = !intents_api_key(&self.env).is_empty();
                match fe2vi_intents::gate_signed_submit(&request, &host, key_present) {
                    Err((status, message)) => {
                        let payload = serde_json::json!({
                            "broadcast": false,
                            "network": "testnet",
                            "error": message,
                        })
                        .to_string();
                        return with_cors(
                            Response::ok(payload)?.with_status(status),
                            "application/json",
                        );
                    }
                    Ok(ready) => signed_submit = ready,
                }
            }
            if arena.chain_network == "testnet" && strategy_request(method, route_path) {
                let secret = self
                    .env
                    .secret("FE2VI_STRATEGY_KEY")
                    .ok()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                crate::strategy::set_strategy_key(crate::strategy::key_from_secret(&secret));
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
                    pin_strategy_owner(&self.env, &mut arena, &agent_id, chain_now).await;
                }
            }
            if method == "POST" && route_path == "/v1/agents/register" {
                prepare_register_chain(&self.env, &body).await;
            } else {
                crate::rules::set_chain(None);
            }
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
            crate::rules::set_chain(None);
            crate::passport::set_match_end_chain(None);
            crate::strategy::set_strategy_key(None);
            crate::strategy::clear_chain_owners();
            crate::logic::set_request_war_secret(None);
            match outcome {
                Outcome::Http {
                    mut status,
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
                    if method == "POST" && route_path == "/v1/intents/act" && status == 200 {
                        if let Some(payload) = signed_submit {
                            let host = intents_submit_host(&self.env);
                            let key = intents_api_key(&self.env);
                            match post_tidepool(&host, &key, &payload).await {
                                Ok((http, upstream)) => {
                                    let interpreted =
                                        fe2vi_intents::interpret_tidepool_submit(http, &upstream);
                                    let broadcast =
                                        interpreted.get("broadcast").and_then(|v| v.as_bool())
                                            == Some(true);
                                    body = fe2vi_intents::redact_secrets(&interpreted.to_string());
                                    if !broadcast {
                                        status = 502;
                                    }
                                }
                                Err(()) => {
                                    body = serde_json::json!({
                                        "broadcast": false,
                                        "network": "testnet",
                                        "error": "intents_rejected",
                                    })
                                    .to_string();
                                    status = 502;
                                }
                            }
                        }
                    }
                    if status == 200 && method == "GET" {
                        if let Some(token_id) = route_path.strip_prefix("/v1/passports/") {
                            if !token_id.is_empty() && !token_id.contains('/') {
                                body = with_chain_playbook(&self.env, token_id, body).await;
                                body = overlay_recorder(&self.env, token_id, body).await;
                            }
                        }
                    }
                    if status == 200 && method == "GET" {
                        if let Some(match_id) = finished_match_id(route_path) {
                            if body.contains("awaiting_contract_view") {
                                let pot = heist_pot(self, match_id).await;
                                if let Some(next) = crate::vault::replace_pot(&body, &pot) {
                                    body = next;
                                }
                            }
                            if body.contains("\"settlementMethod\":\"settle_vault\"")
                                && body.contains("\"creator\":\"\"")
                            {
                                if let Ok(facts) = heist_pot_facts(self, match_id).await {
                                    if let Some(creator) = facts.creator {
                                        if let Some(next) =
                                            crate::vault::fill_creator(&body, &creator)
                                        {
                                            body = next;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let mut response = Response::ok(body)?.with_status(status);
                    if let Some(location) = location {
                        response.headers_mut().set("Location", &location)?;
                    }
                    with_cors(response, content_type)
                }
                Outcome::Websocket { game_id, history } => {
                    let upgrade = req.headers().get("upgrade")?;
                    if !crate::agents::is_websocket_upgrade(upgrade.as_deref()) {
                        let mut response = Response::ok(
                            r#"{"error":"upgrade_required","message":"Open this path with a WebSocket."}"#,
                        )?
                        .with_status(426);
                        response.headers_mut().set("upgrade", "websocket")?;
                        response.headers_mut().set("connection", "Upgrade")?;
                        return with_cors(response, "application/json");
                    }
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
            refresh_unbound_seat_links(&self.env, &mut arena).await;
            let recorder = env_string(&self.env, "FE2VI_PASSPORT_RECORDER_ID");
            crate::auto_settle::queue_unbound_book_binds(&mut arena, &recorder);
            crate::auto_settle::sync_vault_jobs(&mut arena);
            crate::auto_settle::attach_playbook_hashes(&mut arena);
            drive_chain_jobs(self, &mut arena).await;
            let _ = self.cached_market("/v1/market/listings").await;
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
            "CREATE TABLE IF NOT EXISTS player_strategy_versions (token_id TEXT NOT NULL, version INTEGER NOT NULL, created_unix INTEGER NOT NULL, agent_id TEXT NOT NULL, game TEXT NOT NULL, nonce_hex TEXT NOT NULL, ciphertext_b64 TEXT NOT NULL, PRIMARY KEY (token_id, version))",
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
        #[derive(serde::Deserialize)]
        struct StrategyRow {
            token_id: String,
            version: i64,
            created_unix: i64,
            agent_id: String,
            game: String,
            nonce_hex: String,
            ciphertext_b64: String,
        }
        let strategies = sql
            .exec(
                "SELECT token_id, version, created_unix, agent_id, game, nonce_hex, ciphertext_b64 FROM player_strategy_versions",
                None::<Vec<worker::SqlStorageValue>>,
            )?
            .to_array::<StrategyRow>()?;
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
            strategies: strategies
                .into_iter()
                .map(|row| crate::strategy::StrategyVersion {
                    token_id: row.token_id,
                    version: row.version.max(0) as u64,
                    created_unix: row.created_unix.max(0) as u64,
                    agent_id: row.agent_id,
                    game: row.game,
                    nonce_hex: row.nonce_hex,
                    ciphertext_b64: row.ciphertext_b64,
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
        for (token_id, version) in &plan.delete_strategies {
            sql.exec(
                "DELETE FROM player_strategy_versions WHERE token_id = ? AND version = ?",
                Some(vec![token_id.clone().into(), (*version as i64).into()]),
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
        for row in &plan.upsert_strategies {
            sql.exec(
                "INSERT INTO player_strategy_versions (token_id, version, created_unix, agent_id, game, nonce_hex, ciphertext_b64) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(token_id, version) DO UPDATE SET created_unix = excluded.created_unix, agent_id = excluded.agent_id, game = excluded.game, nonce_hex = excluded.nonce_hex, ciphertext_b64 = excluded.ciphertext_b64",
                Some(vec![
                    row.token_id.clone().into(),
                    (row.version as i64).into(),
                    (row.created_unix as i64).into(),
                    row.agent_id.clone().into(),
                    row.game.clone().into(),
                    row.nonce_hex.clone().into(),
                    row.ciphertext_b64.clone().into(),
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

    async fn drive_chain_jobs(feed: &SpectatorFeed, arena: &mut ArenaState) {
        let env = &feed.env;
        crate::auto_settle::recover_crashed(&mut arena.chain_jobs);
        if let Some(index) = crate::auto_settle::active_broadcast(&arena.chain_jobs) {
            confirm_chain_job(feed, arena, index).await;
            return;
        }
        let settler_secret = worker_secret(env, "FE2VI_SETTLER_SECRET_KEY");
        let recorder_secret = worker_secret(env, "FE2VI_RECORDER_SECRET_KEY");
        let Some(index) = crate::auto_settle::claim_ready(
            &mut arena.chain_jobs,
            !settler_secret.is_empty(),
            !recorder_secret.is_empty(),
        ) else {
            return;
        };
        let kind = arena.chain_jobs[index].kind.clone();
        if kind == "career" {
            let token = arena.chain_jobs[index].token_id.clone();
            let frozen = arena.chain_jobs[index]
                .args
                .get("update")
                .and_then(serde_json::Value::as_object)
                .is_some();
            let rpc = env_string(env, "FE2VI_RPC_URL");
            let account = env_string(env, "FE2VI_PASSPORT_RECORDER_ID");
            let Some(view) = read_passport_view(&rpc, &account, &token).await else {
                crate::auto_settle::mark_retry(&mut arena.chain_jobs[index]);
                return;
            };
            if !frozen {
                if !crate::auto_settle::freeze_career_update(&mut arena.chain_jobs[index], &view) {
                    crate::auto_settle::mark_retry(&mut arena.chain_jobs[index]);
                    return;
                }
                crate::auto_settle::release_pending(&mut arena.chain_jobs[index]);
                return;
            }
            let seq = arena.chain_jobs[index].args["update"]["seq"]
                .as_u64()
                .unwrap_or(0);
            if crate::auto_settle::career_already_applied(&view, seq) {
                crate::auto_settle::mark_done(&mut arena.chain_jobs[index]);
                return;
            }
        }
        let (secret, signer) = if kind == "settle" {
            (settler_secret, env_string(env, "FE2VI_SETTLER_ACCOUNT"))
        } else {
            let account = env_string(env, "FE2VI_RECORDER_ACCOUNT");
            (
                recorder_secret,
                if account.is_empty() {
                    "fe2virecdb1e91.testnet".into()
                } else {
                    account
                },
            )
        };
        let receiver = if kind == "settle" {
            env_string(env, "FE2VI_FACTORY_ID")
        } else {
            env_string(env, "FE2VI_PASSPORT_RECORDER_ID")
        };
        if signer.is_empty() || receiver.is_empty() {
            crate::auto_settle::mark_retry(&mut arena.chain_jobs[index]);
            return;
        }
        let method = match kind.as_str() {
            "settle" => "settle_vault",
            "playbook" => "set_playbook_hash",
            "bind" => "bind",
            _ => "sync_career",
        };
        if kind == "bind" {
            let contract = arena.chain_jobs[index]
                .args
                .get("contract")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            if contract != receiver
                || contract.contains("fe2vidisp")
                || contract.ends_with(".fe2vi.testnet")
            {
                crate::auto_settle::park_rejected(&mut arena.chain_jobs[index]);
                return;
            }
            let token = arena.chain_jobs[index]
                .args
                .get("token_id")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            let agent = arena.chain_jobs[index]
                .args
                .get("agent_id")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            let rpc = env_string(env, "FE2VI_RPC_URL");
            match rpc_view(
                &rpc,
                &contract,
                "agent_token",
                serde_json::json!({ "agent_id": agent }),
            )
            .await
            {
                Ok(value) => {
                    let current = value.as_str().unwrap_or("").trim();
                    if current == token {
                        crate::auto_settle::mark_done(&mut arena.chain_jobs[index]);
                        return;
                    }
                    if !current.is_empty() {
                        crate::auto_settle::park_rejected(&mut arena.chain_jobs[index]);
                        return;
                    }
                }
                Err(_) => {
                    crate::auto_settle::mark_retry(&mut arena.chain_jobs[index]);
                    return;
                }
            }
        }
        if kind == "settle" {
            match heist_game_present(env, &arena.chain_jobs[index].match_id).await {
                Ok(false) => {
                    let match_id = arena.chain_jobs[index].match_id.clone();
                    crate::auto_settle::record_missing_pot(arena, &match_id);
                    return;
                }
                Err(()) => {
                    crate::auto_settle::mark_retry(&mut arena.chain_jobs[index]);
                    return;
                }
                Ok(true) => {}
            }
        }
        let args = if kind == "career" {
            match crate::auto_settle::career_call_body(&arena.chain_jobs[index]) {
                Some(body) => body.to_string().into_bytes(),
                None => {
                    crate::auto_settle::mark_retry(&mut arena.chain_jobs[index]);
                    return;
                }
            }
        } else if kind == "bind" {
            serde_json::json!({
                "token_id": arena.chain_jobs[index].args.get("token_id").cloned().unwrap_or(serde_json::Value::Null),
                "agent_id": arena.chain_jobs[index].args.get("agent_id").cloned().unwrap_or(serde_json::Value::Null),
            })
            .to_string()
            .into_bytes()
        } else {
            arena.chain_jobs[index].args.to_string().into_bytes()
        };
        let gas = match kind.as_str() {
            "settle" => 300_000_000_000_000,
            "playbook" => 30_000_000_000_000,
            _ => 50_000_000_000_000,
        };
        match broadcast_call(env, &signer, &secret, &receiver, method, &args, gas).await {
            Ok(hash) => crate::auto_settle::mark_broadcast(&mut arena.chain_jobs[index], &hash),
            Err(()) => crate::auto_settle::mark_retry(&mut arena.chain_jobs[index]),
        }
    }

    async fn confirm_chain_job(feed: &SpectatorFeed, arena: &mut ArenaState, index: usize) {
        let env = &feed.env;
        let hash = arena.chain_jobs[index].tx_hash.clone();
        let kind = arena.chain_jobs[index].kind.clone();
        let match_id = arena.chain_jobs[index].match_id.clone();
        let signer = if kind == "settle" {
            env_string(env, "FE2VI_SETTLER_ACCOUNT")
        } else {
            let account = env_string(env, "FE2VI_RECORDER_ACCOUNT");
            if account.is_empty() {
                "fe2virecdb1e91.testnet".into()
            } else {
                account
            }
        };
        let rpc = env_string(env, "FE2VI_RPC_URL");
        let Ok((status, text)) = rpc_exchange(
            &rpc,
            "EXPERIMENTAL_tx_status",
            serde_json::json!({
                "tx_hash": hash,
                "sender_account_id": signer,
                "wait_until": "FINAL",
            }),
        )
        .await
        else {
            return;
        };
        if text.contains("does not exist") || status == 404 {
            return;
        }
        if text.contains("Failure") {
            crate::auto_settle::park_rejected(&mut arena.chain_jobs[index]);
            return;
        }
        if !text.contains("SuccessValue") {
            return;
        }
        if kind == "settle" {
            let Ok(result) = crate::vault::read_tx_response(status, &text) else {
                return;
            };
            let Some(play_index) = arena
                .book
                .matches
                .iter()
                .position(|play| play.match_id == match_id)
            else {
                return;
            };
            let Some(mut pot) = arena.book.matches[play_index]
                .vault
                .as_ref()
                .and_then(|vault| vault.pot.clone())
            else {
                return;
            };
            let Ok(facts) = heist_pot_facts(feed, &match_id).await else {
                return;
            };
            if pot.creator.is_empty() {
                if let Some(creator) = facts.creator {
                    pot.creator = creator;
                }
            }
            let factory = env_string(env, "FE2VI_FACTORY_ID");
            let heist = env_string(env, "FE2VI_HEIST_ID");
            if factory.is_empty() || heist.is_empty() {
                return;
            }
            match crate::vault::confirm_vault_receipt(
                &pot,
                &hash,
                &result,
                facts.pot_yocto,
                &factory,
                &heist,
            ) {
                Ok(next) => {
                    if let Some(vault) = arena.book.matches[play_index].vault.as_mut() {
                        vault.pot = Some(next.clone());
                    }
                    crate::passport::credit_passports(&mut arena.book.passports, &next);
                    crate::auto_settle::mark_done(&mut arena.chain_jobs[index]);
                    crate::auto_settle::sync_vault_jobs(arena);
                    crate::auto_settle::attach_playbook_hashes(arena);
                }
                Err("transaction is not final success") => {}
                Err(_) => crate::auto_settle::park_rejected(&mut arena.chain_jobs[index]),
            }
            return;
        }
        crate::auto_settle::mark_done(&mut arena.chain_jobs[index]);
        crate::auto_settle::sync_vault_jobs(arena);
        crate::auto_settle::attach_playbook_hashes(arena);
    }

    async fn broadcast_call(
        env: &Env,
        signer: &str,
        secret: &str,
        receiver: &str,
        method: &str,
        args: &[u8],
        gas: u64,
    ) -> Result<String, ()> {
        let network = env_string(env, "FE2VI_NETWORK");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        if indexer::allow_chain_rpc(&network, &rpc).is_err() || !rpc.contains("testnet") {
            return Err(());
        }
        let public = crate::near_sign::public_key(secret).map_err(|_| ())?;
        let (_status, key_text) = rpc_exchange(
            &rpc,
            "query",
            serde_json::json!({
                "request_type": "view_access_key",
                "finality": "final",
                "account_id": signer,
                "public_key": public,
            }),
        )
        .await
        .map_err(|_| ())?;
        let nonce = serde_json::from_str::<serde_json::Value>(&key_text)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/result/nonce")
                    .and_then(|item| item.as_u64())
            })
            .ok_or(())?
            .saturating_add(1);
        let (_status, block_text) =
            rpc_exchange(&rpc, "block", serde_json::json!({"finality": "final"}))
                .await
                .map_err(|_| ())?;
        let block_hash = serde_json::from_str::<serde_json::Value>(&block_text)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/result/header/hash")
                    .and_then(|item| item.as_str())
                    .map(str::to_string)
            })
            .ok_or(())?;
        let signed = crate::near_sign::sign_function_call(
            signer,
            secret,
            receiver,
            method,
            args,
            gas,
            0,
            nonce,
            &block_hash,
        )
        .map_err(|_| ())?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(signed);
        let (_status, sent) =
            rpc_exchange(&rpc, "broadcast_tx_commit", serde_json::json!([encoded]))
                .await
                .map_err(|_| ())?;
        serde_json::from_str::<serde_json::Value>(&sent)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/result/transaction/hash")
                    .or_else(|| value.pointer("/result/transaction_outcome/id"))
                    .and_then(|item| item.as_str())
                    .map(str::to_string)
            })
            .filter(|hash| !hash.is_empty())
            .ok_or(())
    }

    fn worker_secret(env: &Env, name: &str) -> String {
        env.secret(name)
            .ok()
            .map(|value| value.to_string())
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                env.var(name)
                    .ok()
                    .map(|value| value.to_string())
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    async fn vault_receipt(
        feed: &SpectatorFeed,
        arena: &mut ArenaState,
        match_id: &str,
        body: &str,
    ) -> Result<(u16, String, bool)> {
        let request: serde_json::Value =
            serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let (tx_hash, sender) = match crate::vault::parse_receipt_request(&request) {
            Ok(pair) => pair,
            Err((status, message)) => {
                return Ok((
                    status,
                    serde_json::json!({"error":"validation_error","message":message,"settled":false})
                        .to_string(),
                    false,
                ));
            }
        };
        let Some(index) = arena
            .book
            .matches
            .iter()
            .position(|play| play.match_id == match_id)
        else {
            return Ok((
                404,
                r#"{"error":"not_found","message":"No match with that id."}"#.into(),
                false,
            ));
        };
        let Some(pot) = arena.book.matches[index]
            .vault
            .as_ref()
            .and_then(|vault| vault.pot.clone())
        else {
            return Ok((
                409,
                r#"{"error":"not_ready","message":"This match has no vault result."}"#.into(),
                false,
            ));
        };
        if pot.settled && pot.tx_hash == tx_hash {
            let mut same = pot;
            if same.creator.is_empty() {
                if let Ok(facts) = heist_pot_facts(feed, match_id).await {
                    if let Some(creator) = facts.creator.filter(|account| !account.is_empty()) {
                        same.creator = creator;
                    }
                }
            }
            crate::vault::normalize_vault_pot(&mut same);
            let changed = arena.book.matches[index]
                .vault
                .as_ref()
                .and_then(|vault| vault.pot.as_ref())
                != Some(&same);
            if changed {
                if let Some(vault) = arena.book.matches[index].vault.as_mut() {
                    vault.pot = Some(same.clone());
                }
            }
            return Ok((
                200,
                serde_json::to_string(&same).unwrap_or_else(|_| "{}".into()),
                changed,
            ));
        }
        let network = env_string(&feed.env, "FE2VI_NETWORK");
        let rpc = env_string(&feed.env, "FE2VI_RPC_URL");
        if let Err(message) = indexer::allow_chain_rpc(&network, &rpc) {
            return Ok((
                503,
                serde_json::json!({"error":"chain_unavailable","message":message}).to_string(),
                false,
            ));
        }
        let (http_status, text) = match rpc_exchange(
            &rpc,
            "EXPERIMENTAL_tx_status",
            serde_json::json!({
                "tx_hash": tx_hash,
                "sender_account_id": sender,
                "wait_until": "FINAL",
            }),
        )
        .await
        {
            Ok(pair) => pair,
            Err(_) => {
                return Ok((
                    503,
                    r#"{"error":"chain_unavailable","message":"The settlement receipt could not be read.","settled":false}"#
                        .into(),
                    false,
                ));
            }
        };
        let result = match crate::vault::read_tx_response(http_status, &text) {
            Ok(result) => result,
            Err((status, message)) => {
                let error = match status {
                    503 => "chain_unavailable",
                    404 => "not_found",
                    _ => "validation_error",
                };
                return Ok((
                    status,
                    serde_json::json!({"error":error,"message":message,"settled":false})
                        .to_string(),
                    false,
                ));
            }
        };
        let facts = match heist_pot_facts(feed, match_id).await {
            Ok(facts) => facts,
            Err(()) => {
                return Ok((
                    503,
                    r#"{"error":"chain_unavailable","message":"The pot could not be read.","settled":false}"#
                        .into(),
                    false,
                ));
            }
        };
        let factory = env_string(&feed.env, "FE2VI_FACTORY_ID");
        let heist = env_string(&feed.env, "FE2VI_HEIST_ID");
        if factory.is_empty() || heist.is_empty() {
            return Ok((
                503,
                r#"{"error":"chain_unavailable","message":"The pot could not be read.","settled":false}"#
                    .into(),
                false,
            ));
        }
        let mut pot = pot;
        if pot.creator.is_empty() {
            if let Some(creator) = facts.creator {
                pot.creator = creator;
            }
        }
        match crate::vault::confirm_vault_receipt(
            &pot,
            &tx_hash,
            &result,
            facts.pot_yocto,
            &factory,
            &heist,
        ) {
            Ok(next) => {
                if let Some(vault) = arena.book.matches[index].vault.as_mut() {
                    vault.pot = Some(next.clone());
                }
                crate::passport::credit_passports(&mut arena.book.passports, &next);
                crate::auto_settle::finish_settle_job(&mut arena.chain_jobs, match_id, &tx_hash);
                crate::auto_settle::sync_vault_jobs(arena);
                crate::auto_settle::attach_playbook_hashes(arena);
                Ok((
                    200,
                    serde_json::to_string(&next).unwrap_or_else(|_| "{}".into()),
                    true,
                ))
            }
            Err(message) => Ok((
                409,
                serde_json::json!({"error":"settlement_rejected","message":message,"settled":false})
                    .to_string(),
                false,
            )),
        }
    }

    struct PotFacts {
        creator: Option<String>,
        pot_yocto: u128,
    }

    async fn heist_game_present(env: &Env, match_id: &str) -> Result<bool, ()> {
        let heist = env_string(env, "FE2VI_HEIST_ID");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        if heist.is_empty() || rpc.is_empty() {
            return Err(());
        }
        let args = serde_json::json!({ "game_id": match_id });
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&args).map_err(|_| ())?);
        let (status, text) = rpc_exchange(
            &rpc,
            "query",
            serde_json::json!({
                "request_type": "call_function",
                "finality": "final",
                "account_id": heist,
                "method_name": "get_heist",
                "args_base64": encoded,
            }),
        )
        .await
        .map_err(|_| ())?;
        if status >= 400 {
            return Err(());
        }
        let view = contract_result_text(&text).map_err(|_| ())?;
        Ok(view.trim() != "null")
    }

    async fn heist_pot_facts(feed: &SpectatorFeed, match_id: &str) -> Result<PotFacts, ()> {
        let heist = env_string(&feed.env, "FE2VI_HEIST_ID");
        let rpc = env_string(&feed.env, "FE2VI_RPC_URL");
        if heist.is_empty() || rpc.is_empty() {
            return Err(());
        }
        let args = serde_json::json!({ "game_id": match_id });
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&args).map_err(|_| ())?);
        let (status, text) = rpc_exchange(
            &rpc,
            "query",
            serde_json::json!({
                "request_type": "call_function",
                "finality": "final",
                "account_id": heist,
                "method_name": "get_heist",
                "args_base64": encoded,
            }),
        )
        .await
        .map_err(|_| ())?;
        if status >= 400 {
            return Err(());
        }
        let view = contract_result_text(&text).map_err(|_| ())?;
        if view.trim() == "null" {
            return Err(());
        }
        let creator = pull_json_string(&view, "creator");
        let pot_yocto = crate::vault::pull_u128(&view, "pot_yocto").ok_or(())?;
        Ok(PotFacts { creator, pot_yocto })
    }

    async fn heist_pot(feed: &SpectatorFeed, match_id: &str) -> crate::vault::PotAuthorization {
        let mut pot = crate::vault::contract_pot(match_id, "null", None);
        pot.outcome = "not_linked".to_string();
        let heist = env_string(&feed.env, "FE2VI_HEIST_ID");
        if heist.is_empty() {
            pot.outcome = "contract_unconfigured".to_string();
            pot.reason = "FE2VI_HEIST_ID is empty, so resolve was not accepted.".to_string();
            pot.contract_would_accept_resolve = false;
            pot.pays = false;
            return pot;
        }
        let network = env_string(&feed.env, "FE2VI_NETWORK");
        let rpc = env_string(&feed.env, "FE2VI_RPC_URL");
        if let Err(message) = indexer::allow_chain_rpc(&network, &rpc) {
            pot.outcome = "contract_unreadable".to_string();
            pot.reason = message.to_string();
            pot.contract_would_accept_resolve = false;
            pot.pays = false;
            return pot;
        }
        let args = serde_json::json!({ "game_id": match_id });
        let encoded = match serde_json::to_vec(&args) {
            Ok(bytes) => base64::engine::general_purpose::STANDARD.encode(bytes),
            Err(_) => {
                pot.outcome = "contract_unreadable".to_string();
                pot.reason = "The heist view could not be encoded.".to_string();
                return pot;
            }
        };
        let (http_status, text) = match rpc_exchange(
            &rpc,
            "query",
            serde_json::json!({
                "request_type": "call_function",
                "finality": "final",
                "account_id": heist,
                "method_name": "get_heist",
                "args_base64": encoded,
            }),
        )
        .await
        {
            Ok(pair) => pair,
            Err(_) => {
                pot.outcome = "contract_unreadable".to_string();
                pot.reason = "The heist view could not be read.".to_string();
                pot.contract_would_accept_resolve = false;
                pot.pays = false;
                return pot;
            }
        };
        if http_status >= 400 {
            pot.outcome = "contract_unreadable".to_string();
            pot.reason = "The heist view could not be read.".to_string();
            pot.contract_would_accept_resolve = false;
            pot.pays = false;
            return pot;
        }
        let view = match contract_result_text(&text) {
            Ok(view) => view,
            Err(()) => {
                pot.outcome = "contract_unreadable".to_string();
                pot.reason = "The heist view could not be read.".to_string();
                pot.contract_would_accept_resolve = false;
                pot.pays = false;
                return pot;
            }
        };
        if view.trim() == "null" {
            return crate::vault::contract_pot(match_id, "null", None);
        }
        let resolved = view.contains("\"resolved\":true");
        let delisted = view.contains("\"delisted\":true");
        let now_ns = if resolved || delisted {
            None
        } else {
            match heist_block_time(&rpc, &text).await {
                Some(now_ns) => Some(now_ns),
                None => return crate::vault::contract_pot(match_id, &view, None),
            }
        };
        crate::vault::contract_pot(match_id, &view, now_ns)
    }

    async fn refresh_unbound_seat_links(env: &Env, arena: &mut ArenaState) {
        let mut agents = Vec::new();
        for play in &arena.book.matches {
            if play.arena_id != "vault" {
                continue;
            }
            let Some(pot) = play.vault.as_ref().and_then(|vault| vault.pot.as_ref()) else {
                continue;
            };
            if pot.settled || pot.pays {
                continue;
            }
            for link in &pot.seat_links {
                if link.token_id.is_empty() && !agents.iter().any(|id| id == &link.agent_id) {
                    agents.push(link.agent_id.clone());
                }
            }
        }
        if agents.is_empty() {
            return;
        }
        let reads = read_match_end_bindings(env, &agents).await;
        for play in &mut arena.book.matches {
            let Some(pot) = play.vault.as_mut().and_then(|vault| vault.pot.as_mut()) else {
                continue;
            };
            if pot.settled || pot.pays {
                continue;
            }
            crate::passport::refresh_unbound_links(&mut pot.seat_links, &reads);
        }
    }

    async fn read_match_end_bindings(
        env: &Env,
        agent_ids: &[String],
    ) -> std::collections::HashMap<String, crate::passport::ChainSeatBinding> {
        let mut out = std::collections::HashMap::new();
        let network = env_string(env, "FE2VI_NETWORK");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        let passport = env_string(env, "FE2VI_PASSPORT_ID");
        let recorder = env_string(env, "FE2VI_PASSPORT_RECORDER_ID");
        let mut contracts = Vec::new();
        if !recorder.is_empty() {
            contracts.push(recorder);
        }
        if !passport.is_empty() && !contracts.iter().any(|account| account == &passport) {
            contracts.push(passport);
        }
        let blocked = contracts.is_empty()
            || rpc.is_empty()
            || indexer::allow_chain_rpc(&network, &rpc).is_err();
        for agent_id in agent_ids {
            if blocked {
                out.insert(
                    agent_id.clone(),
                    crate::passport::ChainSeatBinding::Unverified,
                );
                continue;
            }
            let mut reads = Vec::new();
            for contract in &contracts {
                match rpc_view(
                    &rpc,
                    contract,
                    "agent_token",
                    serde_json::json!({ "agent_id": agent_id }),
                )
                .await
                {
                    Ok(value) => {
                        let token = value.as_str().map(str::trim).unwrap_or("").to_string();
                        reads.push(crate::passport::AgentTokenRead::Token(token));
                        if reads.iter().any(|read| {
                            matches!(read, crate::passport::AgentTokenRead::Token(token) if !token.is_empty())
                        }) {
                            break;
                        }
                    }
                    Err(_) => reads.push(crate::passport::AgentTokenRead::Failed),
                }
            }
            out.insert(
                agent_id.clone(),
                crate::passport::merge_agent_token_reads(&reads),
            );
        }
        out
    }

    fn intents_api_key(env: &Env) -> String {
        let from_secret = env
            .secret("NEAR_INTENTS_AGENT_API_KEY")
            .ok()
            .map(|value| value.to_string())
            .unwrap_or_default();
        if !from_secret.trim().is_empty() {
            return from_secret.trim().to_string();
        }
        env.var("NEAR_INTENTS_AGENT_API_KEY")
            .ok()
            .map(|value| value.to_string())
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn intents_submit_host(env: &Env) -> String {
        let host = env
            .var("NEAR_INTENTS_AGENT_API_URL")
            .ok()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "https://www.usetidepool.tech".to_string());
        host.trim().trim_end_matches('/').to_string()
    }

    async fn post_tidepool(
        host: &str,
        key: &str,
        payload: &serde_json::Value,
    ) -> Result<(u16, serde_json::Value), ()> {
        if key.is_empty() || fe2vi_intents::intents_host_allowed(host).is_err() {
            return Err(());
        }
        let url = format!("{host}/v1/submit-intent");
        let headers = Headers::new();
        if headers
            .set("authorization", &format!("Bearer {key}"))
            .is_err()
            || headers.set("content-type", "application/json").is_err()
            || headers.set("accept", "application/json").is_err()
        {
            return Err(());
        }
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        init.with_headers(headers);
        init.with_body(Some(wasm_bindgen::JsValue::from_str(&payload.to_string())));
        let request = Request::new_with_init(&url, &init).map_err(|_| ())?;
        let mut response = Fetch::Request(request).send().await.map_err(|_| ())?;
        let status = response.status_code();
        let text = response.text().await.map_err(|_| ())?;
        let text = fe2vi_intents::redact_secrets(&text);
        let value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        Ok((status, value))
    }

    async fn passport_list_response(env: &Env) -> Result<Response> {
        let network = env_string(env, "FE2VI_NETWORK");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        if let Err(message) = indexer::allow_chain_rpc(&network, &rpc) {
            let body = serde_json::json!({
                "error": "passport_lookup_unavailable",
                "message": message,
                "source": "unavailable",
            })
            .to_string();
            return with_cors(Response::ok(body)?.with_status(503), "application/json");
        }
        let mut contracts = Vec::new();
        let main = env_string(env, "FE2VI_PASSPORT_ID");
        if !main.is_empty() {
            contracts.push(main);
        }
        let recorder = env_string(env, "FE2VI_PASSPORT_RECORDER_ID");
        if !recorder.is_empty() && !contracts.iter().any(|account| account == &recorder) {
            contracts.push(recorder);
        }
        let mut passports = Vec::new();
        let mut read_one = false;
        for account in contracts {
            let Ok(tokens) = nft_token_page(&rpc, &account).await else {
                continue;
            };
            read_one = true;
            for token in tokens {
                if let Some(row) = crate::passport::passport_list_row(&account, &token, None) {
                    passports.push(row);
                }
            }
        }
        if !read_one {
            let body = serde_json::json!({
                "error": "passport_lookup_unavailable",
                "message": "Could not read the passport contracts.",
                "source": "unavailable",
            })
            .to_string();
            return with_cors(Response::ok(body)?.with_status(503), "application/json");
        }
        let body = serde_json::json!({ "passports": passports, "source": "chain" }).to_string();
        with_cors(Response::ok(body)?.with_status(200), "application/json")
    }

    async fn recorder_passport_response(
        env: &Env,
        book: &crate::agents::PlayerBook,
        token_id: &str,
    ) -> Result<Option<Response>> {
        let account = env_string(env, "FE2VI_PASSPORT_RECORDER_ID");
        if account.is_empty() {
            return Ok(None);
        }
        let network = env_string(env, "FE2VI_NETWORK");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        if indexer::allow_chain_rpc(&network, &rpc).is_err() {
            return Ok(None);
        }
        let Some(view) = read_passport_view(&rpc, &account, token_id).await else {
            return Ok(None);
        };
        let playbook = read_playbook(&rpc, &account, token_id).await;
        let mut body = crate::passport::chain_passport_json(token_id, &view);
        crate::strategy::stamp_public(&mut body, book, token_id);
        crate::passport::merge_recorder(&mut body, &account, &view, playbook.as_ref());
        let response = with_cors(
            Response::ok(body.to_string())?.with_status(200),
            "application/json",
        )?;
        Ok(Some(response))
    }

    async fn overlay_recorder(env: &Env, token_id: &str, body: String) -> String {
        let account = env_string(env, "FE2VI_PASSPORT_RECORDER_ID");
        if account.is_empty() {
            return body;
        }
        let network = env_string(env, "FE2VI_NETWORK");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        if indexer::allow_chain_rpc(&network, &rpc).is_err() {
            return body;
        }
        let Some(view) = read_passport_view(&rpc, &account, token_id).await else {
            return body;
        };
        let playbook = read_playbook(&rpc, &account, token_id).await;
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&body) else {
            return body;
        };
        crate::passport::merge_recorder(&mut value, &account, &view, playbook.as_ref());
        value.to_string()
    }

    async fn read_passport_view(
        rpc: &str,
        account_id: &str,
        token_id: &str,
    ) -> Option<serde_json::Value> {
        let args = serde_json::json!({ "token_id": token_id });
        let (status, text) = rpc_view_raw(rpc, account_id, "passport", args).await.ok()?;
        if text.contains("no token") {
            return None;
        }
        indexer::interpret_rpc_query(status, &text)
            .ok()
            .filter(|value| !value.is_null())
    }

    async fn read_playbook(
        rpc: &str,
        account_id: &str,
        token_id: &str,
    ) -> Option<serde_json::Value> {
        let (status, text) = rpc_view_raw(
            rpc,
            account_id,
            "playbook_latest",
            serde_json::json!({ "token_id": token_id }),
        )
        .await
        .ok()?;
        indexer::interpret_rpc_query(status, &text)
            .ok()
            .filter(|value| value.is_object())
    }

    async fn nft_token_page(rpc: &str, account_id: &str) -> Result<Vec<serde_json::Value>, ()> {
        let mut rows = Vec::new();
        let mut from = 0_u64;
        for _ in 0..4 {
            let (status, text) = rpc_view_raw(
                rpc,
                account_id,
                "nft_tokens",
                serde_json::json!({ "from_index": from.to_string(), "limit": 50 }),
            )
            .await?;
            let value = indexer::interpret_rpc_query(status, &text).map_err(|_| ())?;
            let Some(page) = value.as_array() else {
                break;
            };
            let count = page.len() as u64;
            rows.extend(page.iter().cloned());
            if count < 50 {
                break;
            }
            from = from.saturating_add(count);
        }
        Ok(rows)
    }

    async fn rpc_view_raw(
        rpc: &str,
        account_id: &str,
        method_name: &str,
        args: serde_json::Value,
    ) -> Result<(u16, String), ()> {
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&args).map_err(|_| ())?);
        rpc_exchange(
            rpc,
            "query",
            serde_json::json!({
                "request_type": "call_function",
                "finality": "final",
                "account_id": account_id,
                "method_name": method_name,
                "args_base64": encoded,
            }),
        )
        .await
        .map_err(|_| ())
    }

    async fn with_chain_playbook(env: &Env, token_id: &str, body: String) -> String {
        let Some(hash) = chain_playbook_hash(env, token_id).await else {
            return body;
        };
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&body) else {
            return body;
        };
        crate::strategy::apply_chain_playbook(&mut value, Some(&hash));
        value.to_string()
    }

    async fn chain_playbook_hash(env: &Env, token_id: &str) -> Option<String> {
        let network = env_string(env, "FE2VI_NETWORK");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        if indexer::allow_chain_rpc(&network, &rpc).is_err() {
            return None;
        }
        let account_id = env_string(env, "FE2VI_PASSPORT_ID");
        if account_id.is_empty() {
            return None;
        }
        let value = rpc_view(
            &rpc,
            &account_id,
            "playbook_latest",
            serde_json::json!({ "token_id": token_id }),
        )
        .await
        .ok()?;
        value
            .get("hash")
            .and_then(serde_json::Value::as_str)
            .filter(|hash| hash.len() == 64)
            .map(str::to_string)
    }

    async fn prepare_register_chain(env: &Env, body: &str) {
        let request: serde_json::Value =
            serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let account = request
            .get("nearAccount")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let token_id = request
            .get("tokenId")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let rpc = env_string(env, "FE2VI_RPC_URL");
        if account.is_empty() || token_id.is_empty() || rpc.is_empty() || !rpc.contains("testnet") {
            crate::rules::set_chain(None);
            return;
        }
        let keys = access_keys(&rpc, &account).await.unwrap_or_default();
        let contracts = [
            env_string(env, "FE2VI_PASSPORT_RECORDER_ID"),
            env_string(env, "FE2VI_PASSPORT_ID"),
        ];
        for contract in contracts {
            if contract.is_empty() {
                continue;
            }
            if let Some(view) = read_passport_view(&rpc, &contract, &token_id).await {
                let owner = view
                    .get("owner_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let bound = view
                    .get("bound_agent")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string();
                crate::rules::set_chain(Some(crate::rules::ChainPassport {
                    contract,
                    owner_id: owner,
                    bound_agent: bound,
                    keys,
                    found: true,
                }));
                return;
            }
        }
        crate::rules::set_chain(Some(crate::rules::ChainPassport {
            contract: String::new(),
            owner_id: String::new(),
            bound_agent: String::new(),
            keys,
            found: false,
        }));
    }

    async fn access_keys(rpc: &str, account: &str) -> Result<Vec<String>, ()> {
        let (status, text) = rpc_exchange(
            rpc,
            "query",
            serde_json::json!({
                "request_type": "view_access_key_list",
                "finality": "final",
                "account_id": account,
            }),
        )
        .await
        .map_err(|_| ())?;
        if status >= 400 {
            return Err(());
        }
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| ())?;
        let rows = value
            .pointer("/result/keys")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(rows
            .iter()
            .filter_map(|row| {
                row.get("public_key")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .collect())
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

    fn strategy_request(method: &str, path: &str) -> bool {
        if path == "/v1/agents/me/passport/strategy" && (method == "GET" || method == "POST") {
            return true;
        }
        method == "POST"
            && path
                .strip_prefix("/v1/matches/")
                .and_then(|rest| rest.strip_suffix("/actions"))
                .is_some_and(|id| !id.is_empty() && !id.contains('/'))
    }

    async fn pin_strategy_owner(env: &Env, arena: &mut ArenaState, agent_id: &str, now: u64) {
        let Some(token_id) = arena
            .book
            .passports
            .iter()
            .find(|passport| {
                passport.agent_id == agent_id
                    && passport.status != "unbound"
                    && passport
                        .token_id
                        .as_deref()
                        .is_some_and(|token| !token.is_empty())
            })
            .and_then(|passport| passport.token_id.clone())
        else {
            return;
        };
        let _ = reconcile_token(env, arena, &token_id, now).await;
        let Ok(Some(owner)) = nft_owner(env, &token_id).await else {
            return;
        };
        crate::strategy::set_chain_owner(&token_id, &owner);
    }

    async fn nft_owner(env: &Env, token_id: &str) -> Result<Option<String>> {
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
        let value = rpc_view(
            &rpc,
            &account_id,
            "nft_token",
            serde_json::json!({ "token_id": token_id }),
        )
        .await?;
        if value.is_null() {
            return Ok(None);
        }
        let owner = value
            .get("owner_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if owner.is_empty() {
            Ok(None)
        } else {
            Ok(Some(owner))
        }
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

    async fn heist_block_time(rpc: &str, query_text: &str) -> Option<u64> {
        let hash = pull_json_string(query_text, "block_hash")?;
        let (status, block) = rpc_exchange(rpc, "block", serde_json::json!({ "block_id": hash }))
            .await
            .ok()?;
        if status >= 400 {
            return None;
        }
        crate::vault::pull_u64(&block, "timestamp")
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
            self.cached_market(path).await
        }

        async fn cached_market(&self, path: &str) -> Result<Response> {
            let now = worker::Date::now().as_millis() / 1000;
            let storage = self.state.storage();
            let stored = storage
                .get::<crate::market_batch::MarketSnapshot>("market_snapshot")
                .await
                .ok()
                .flatten();
            if !crate::market_batch::should_refresh(stored.as_ref(), now) {
                return market_from_snapshot(self, path, stored.as_ref(), false).await;
            }
            match load_market_body(&self.env).await {
                Ok(body) => {
                    let snapshot = crate::market_batch::MarketSnapshot {
                        fetched_unix: now,
                        backoff_until: 0,
                        body: Some(body),
                    };
                    let _ = storage.put("market_snapshot", &snapshot).await;
                    market_from_snapshot(self, path, Some(&snapshot), false).await
                }
                Err(MarketFail::RateLimited) => {
                    let mut snapshot = stored.unwrap_or(crate::market_batch::MarketSnapshot {
                        fetched_unix: now,
                        backoff_until: 0,
                        body: None,
                    });
                    crate::market_batch::apply_rate_limit(&mut snapshot, now);
                    let _ = storage.put("market_snapshot", &snapshot).await;
                    if snapshot.body.is_some() {
                        market_from_snapshot(self, path, Some(&snapshot), true).await
                    } else {
                        market_error(503, "market_unavailable", "The market RPC is rate limited.")
                    }
                }
                Err(MarketFail::Failed(message)) => {
                    if stored
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.body.is_some())
                    {
                        market_from_snapshot(self, path, stored.as_ref(), true).await
                    } else {
                        market_error(
                            503,
                            "market_unavailable",
                            &format!("Could not read the market ({message})."),
                        )
                    }
                }
            }
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

    enum MarketFail {
        RateLimited,
        Failed(String),
    }

    async fn market_from_snapshot(
        feed: &SpectatorFeed,
        path: &str,
        snapshot: Option<&crate::market_batch::MarketSnapshot>,
        stale: bool,
    ) -> Result<Response> {
        let Some(body) = snapshot.and_then(|snapshot| snapshot.body.clone()) else {
            return market_error(503, "market_unavailable", "Could not read the market.");
        };
        let mut body = body;
        if stale {
            body["source"] = serde_json::json!("stale");
        }
        let storage = feed.state.storage();
        if let Ok(stored) = storage.get::<crate::logic::ArenaState>("arena").await {
            let mut arena = stored.unwrap_or_default();
            let sql = storage.sql();
            if attach_player_book(&sql, &mut arena).is_ok() {
                crate::passport::overlay_vault_careers(&mut body, &arena.book.passports);
            }
        }
        let payload = if path.ends_with("/sales") {
            serde_json::json!({
                "sales": body["sales"],
                "source": body["source"],
                "contract": body["contract"],
                "network": body["network"],
            })
        } else {
            body
        };
        with_cors(Response::ok(payload.to_string())?, "application/json")
    }

    async fn load_market_body(env: &Env) -> Result<serde_json::Value, MarketFail> {
        let network = env_string(env, "FE2VI_NETWORK");
        let rpc = env_string(env, "FE2VI_RPC_URL");
        if let Err(message) = indexer::allow_chain_rpc(&network, &rpc) {
            return Err(MarketFail::Failed(message.to_string()));
        }
        let passport = env_string(env, "FE2VI_PASSPORT_ID");
        let market = env_string(env, "FE2VI_PASSPORT_MARKET_ID");
        if passport.is_empty() || market.is_empty() {
            return Err(MarketFail::Failed(
                "Passport or market account is not configured.".into(),
            ));
        }
        let mut boards = vec![(passport.clone(), market.clone())];
        let recorder_passport = env_string(env, "FE2VI_PASSPORT_RECORDER_ID");
        let recorder_market = env_string(env, "FE2VI_RECORDER_MARKET_ID");
        if !recorder_passport.is_empty() && !recorder_market.is_empty() && recorder_market != market
        {
            boards.push((recorder_passport, recorder_market));
        }
        let mut listed = Vec::new();
        let mut passports = Vec::new();
        let mut read_one = false;
        let mut first_error = None;
        for (passport_id, market_id) in &boards {
            let board = match read_market_board(&rpc, passport_id, market_id).await {
                Ok(board) => board,
                Err(MarketFail::RateLimited) => return Err(MarketFail::RateLimited),
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    continue;
                }
            };
            read_one = true;
            for token in &board.tokens {
                let token_id = token
                    .get("token_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let listing = board
                    .listings
                    .get(token_id)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                if let Some(mut row) = crate::passport::listed_passport(token, &listing) {
                    row["marketContract"] = serde_json::json!(market_id);
                    listed.push(row);
                }
                if let Some(mut card) = crate::passport::market_passport(token, &listing) {
                    card["marketContract"] = serde_json::json!(market_id);
                    passports.push(card);
                }
            }
        }
        if !read_one {
            return Err(
                first_error.unwrap_or(MarketFail::Failed("Could not read the market.".into()))
            );
        }
        let mut settlement =
            match market_call(&rpc, &market, "last_settlement", serde_json::json!({})).await {
                Ok(value) if value.is_null() => None,
                Ok(value) => Some(value),
                Err(_) => None,
            };
        if passports.len() == 1 {
            if let Some(value) = settlement.as_mut() {
                if let Some(token_id) = passports[0].get("tokenId").cloned() {
                    value["token_id"] = token_id;
                }
            }
        }
        let mut body = crate::passport::market_from_chain(
            &market,
            &network,
            &listed,
            settlement.as_ref(),
            "chain",
        );
        body["passports"] = serde_json::Value::Array(passports);
        Ok(body)
    }

    struct MarketBoard {
        tokens: Vec<serde_json::Value>,
        listings: std::collections::HashMap<String, serde_json::Value>,
    }

    async fn read_market_board(
        rpc: &str,
        passport: &str,
        market: &str,
    ) -> Result<MarketBoard, MarketFail> {
        let mut tokens = Vec::new();
        let mut from = 0_u64;
        for pages in 1..=crate::market_batch::MAX_PAGES {
            let page = market_call(
                rpc,
                passport,
                "nft_tokens",
                serde_json::json!({
                    "from_index": from.to_string(),
                    "limit": crate::market_batch::PAGE_LIMIT,
                }),
            )
            .await?;
            let Some(rows) = page.as_array().cloned() else {
                return Err(MarketFail::Failed(
                    "Passport token list was not an array.".into(),
                ));
            };
            let count = rows.len() as u64;
            tokens.extend(rows);
            match crate::market_batch::next_token_page(from, count, pages) {
                Some(next) => from = next,
                None => break,
            }
        }
        let mut listings = std::collections::HashMap::new();
        for row in market_listing_batch(rpc, market).await? {
            listings.insert(
                row.token_id.clone(),
                crate::market_batch::listing_json(&row),
            );
        }
        Ok(MarketBoard { tokens, listings })
    }

    async fn market_listing_batch(
        rpc: &str,
        market: &str,
    ) -> Result<Vec<crate::market_batch::SaleRow>, MarketFail> {
        let prefix = base64::engine::general_purpose::STANDARD.encode([b'l']);
        let (status, text) = rpc_exchange(
            rpc,
            "query",
            serde_json::json!({
                "request_type": "view_state",
                "finality": "final",
                "account_id": market,
                "prefix_base64": prefix,
            }),
        )
        .await
        .map_err(|_| MarketFail::Failed("transport".into()))?;
        if crate::market_batch::is_rate_limited(status, &text) {
            return Err(MarketFail::RateLimited);
        }
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|_| MarketFail::Failed("decode".into()))?;
        if value.get("error").is_some_and(|item| !item.is_null()) {
            if crate::market_batch::is_rate_limited(status, &text) {
                return Err(MarketFail::RateLimited);
            }
            return Err(MarketFail::Failed("listing batch".into()));
        }
        let Some(rows) = value
            .pointer("/result/values")
            .and_then(|item| item.as_array())
        else {
            return Err(MarketFail::Failed("listing batch".into()));
        };
        let mut sales = Vec::new();
        for row in rows {
            let Some(key) = row
                .get("key")
                .and_then(serde_json::Value::as_str)
                .and_then(|item| base64::engine::general_purpose::STANDARD.decode(item).ok())
            else {
                continue;
            };
            let Some(raw) = row
                .get("value")
                .and_then(serde_json::Value::as_str)
                .and_then(|item| base64::engine::general_purpose::STANDARD.decode(item).ok())
            else {
                continue;
            };
            if let Some(sale) = crate::market_batch::decode_listing_entry(&key, &raw) {
                sales.push(sale);
            }
        }
        Ok(sales)
    }

    async fn market_call(
        rpc: &str,
        account: &str,
        method: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, MarketFail> {
        let (status, text) = rpc_view_raw(rpc, account, method, args)
            .await
            .map_err(|_| MarketFail::Failed("transport".into()))?;
        if crate::market_batch::is_rate_limited(status, &text) {
            return Err(MarketFail::RateLimited);
        }
        indexer::interpret_rpc_query(status, &text)
            .map_err(|failure| MarketFail::Failed(failure.class.to_string()))
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

    fn vault_receipt_match(path: &str) -> Option<&str> {
        let rest = path.strip_prefix("/v1/internal/vault/matches/")?;
        let id = rest.strip_suffix("/receipt")?;
        if id.is_empty() || id.contains('/') {
            None
        } else {
            Some(id)
        }
    }

    fn finished_match_id(path: &str) -> Option<&str> {
        let rest = path.strip_prefix("/v1/matches/")?;
        if rest.is_empty() || rest.contains('/') {
            None
        } else {
            Some(rest)
        }
    }

    fn contract_result_text(body: &str) -> Result<String, ()> {
        let value: serde_json::Value = serde_json::from_str(body).map_err(|_| ())?;
        if value.get("error").is_some_and(|item| !item.is_null()) {
            return Err(());
        }
        let rows = value
            .pointer("/result/result")
            .and_then(serde_json::Value::as_array)
            .ok_or(())?;
        let bytes: Vec<u8> = rows
            .iter()
            .filter_map(serde_json::Value::as_u64)
            .map(|item| item as u8)
            .collect();
        String::from_utf8(bytes).map_err(|_| ())
    }

    fn pull_json_string(text: &str, field: &str) -> Option<String> {
        let needle = format!("\"{field}\"");
        let index = text.find(&needle)?;
        let rest = text[index + needle.len()..].trim_start();
        let rest = rest.strip_prefix(':')?.trim_start();
        let rest = rest.strip_prefix('"')?;
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }
}
