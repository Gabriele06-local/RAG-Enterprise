//! Admin endpoints (admin role required):
//! POST   /api/admin/backup                  → trigger immediate backup
//! GET    /api/admin/backup/list             → list available archives
//! POST   /api/admin/backup/restore          → restore one of them
//! GET    /api/admin/qdrant/stats            → collection stats
//! GET    /api/admin/qdrant/documents        → unique documents in Qdrant
//! DELETE /api/admin/qdrant/document/{id}   → delete all vectors for a document
//! GET    /api/admin/sqlite/documents        → all rows (soft-deleted included)

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::auth::{jwt::Claims, rbac::Role};
use crate::backup::service;
use crate::db;
use crate::state::AppState;

/// A 4xx tells the caller what they got wrong, so its message travels.
/// A 5xx does not: `msg` is an anyhow chain carrying whatever context the
/// failure picked up on the way out — filesystem paths, the Qdrant URL, SQL
/// text, the body of an eullm reply — and handing that to an unauthenticated
/// caller is free reconnaissance. The detail goes to the log, where it is
/// actually useful, and the response says only that something broke.
fn err(status: StatusCode, msg: impl std::fmt::Display) -> Response {
    if status.is_server_error() {
        tracing::error!(status = %status, detail = %msg, "request failed");
        return (status, Json(json!({ "error": "internal server error" }))).into_response();
    }
    (status, Json(json!({ "error": msg.to_string() }))).into_response()
}

fn require_admin(claims: &Claims) -> Option<Response> {
    if claims.role != Role::Admin {
        Some(err(StatusCode::FORBIDDEN, "admin role required"))
    } else {
        None
    }
}

// ── POST /api/admin/backup ────────────────────────────────────────────────────

pub async fn trigger_backup(State(state): State<AppState>, claims: Claims) -> Response {
    if let Some(r) = require_admin(&claims) {
        return r;
    }

    // Extract the filesystem path from the sqlite:// URL
    let db_path = state
        .settings
        .database
        .url
        .trim_start_matches("sqlite://")
        .to_owned();

    match service::create_backup(
        &state.db,
        &db_path,
        &state.settings.qdrant.url,
        &state.settings.qdrant.collection,
        &state.settings.backup.dir,
        state.settings.backup.retain_last,
    )
    .await
    {
        Ok(path) => Json(json!({
            "ok": true,
            "archive": path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        }))
        .into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

// ── GET /api/admin/backup/list ────────────────────────────────────────────────

pub async fn list_backups(State(state): State<AppState>, claims: Claims) -> Response {
    if let Some(r) = require_admin(&claims) {
        return r;
    }
    let archives = service::list_backups(&state.settings.backup.dir).await;
    Json(json!({ "backups": archives })).into_response()
}

// ── POST /api/admin/backup/restore ────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct RestoreRequest {
    /// File name of an archive in the backup directory, as returned by
    /// GET /api/admin/backup/list.
    archive: String,
}

/// Restore a backup over the running installation.
///
/// Destructive by nature: the contents of the Qdrant collection and of every
/// table also present in the archive are replaced, not merged. It is admin-only
/// for that reason.
pub async fn restore_backup(
    State(state): State<AppState>,
    claims: Claims,
    Json(req): Json<RestoreRequest>,
) -> Response {
    if let Some(r) = require_admin(&claims) {
        return r;
    }

    tracing::warn!(
        archive = %req.archive,
        user = %claims.username,
        "restore requested: the collection and the database are about to be replaced"
    );

    match service::restore_backup(
        &state.db,
        &state.settings.qdrant.url,
        &state.settings.qdrant.collection,
        &state.settings.backup.dir,
        &req.archive,
    )
    .await
    {
        Ok(report) => Json(json!({ "ok": true, "restored": report })).into_response(),
        // A bad archive - the name, or the archive itself - is the caller's
        // mistake, not a server fault. Every other failure happened while
        // restoring and is ours.
        Err(e) if is_bad_request(&e) => err(StatusCode::BAD_REQUEST, e),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

/// Whether the restore failed because of the archive or the name it was given,
/// rather than because of us.
///
/// The archive's own failures are the caller's to fix - wrong name, damaged or
/// truncated tarball, a digest that does not match, a member missing, a
/// manifest naming something outside the archive, a snapshot belonging to a
/// collection this installation is not configured for. Reporting those as 5xx
/// was not merely a wrong status: `err` deliberately withholds a 5xx's message
/// from the caller, so an admin who picked the wrong archive, or restored one
/// taken from another collection, was told "internal server error" and nothing
/// about which archive or why.
///
/// Matched against the whole chain, not `e.to_string()`: that is only the
/// outermost context, so any `.context()` added above one of these in the
/// service would silently demote a 4xx to a 5xx with the message hidden. The
/// test below holds every refusal the service can produce here, so a new one
/// has to be classified deliberately rather than by omission.
fn is_bad_request(e: &anyhow::Error) -> bool {
    let msg = format!("{e:#}");
    const CALLER_FAULT: &[&str] = &[
        // resolve_archive
        "invalid archive name",
        "not a backup archive",
        "archive not found",
        // unpack_tar_gz
        "archive entry escapes the destination",
        "archive entry is not a regular file or directory",
        "the archive's gzip stream",
        // verify_unpacked
        "parsing backup.json",
        "this archive is in backup format",
        "not a plain file name",
        "the archive promises",
        "does not match the archive's own manifest",
        // restore_backup
        "the archive contains no",
        "holds the Qdrant collection",
    ];
    CALLER_FAULT.iter().any(|reason| msg.contains(reason))
}

// ── GET /api/admin/qdrant/stats ───────────────────────────────────────────────

pub async fn qdrant_stats(State(state): State<AppState>, claims: Claims) -> Response {
    if let Some(r) = require_admin(&claims) { return r; }
    let url = format!("{}/collections/{}", state.settings.qdrant.url, state.settings.qdrant.collection);
    match reqwest::get(&url).await {
        Ok(r) => match r.json::<serde_json::Value>().await {
            Ok(body) => Json(body).into_response(),
            Err(e) => err(StatusCode::BAD_GATEWAY, e),
        },
        Err(e) => err(StatusCode::BAD_GATEWAY, e),
    }
}

// ── GET /api/admin/qdrant/documents ──────────────────────────────────────────

pub async fn qdrant_documents(State(state): State<AppState>, claims: Claims) -> Response {
    if let Some(r) = require_admin(&claims) { return r; }
    let client = reqwest::Client::new();
    match scroll_all_documents(&client, &state.settings.qdrant.url, &state.settings.qdrant.collection).await {
        Ok(list) => Json(json!({ "documents": list })).into_response(),
        Err(e) => err(StatusCode::BAD_GATEWAY, e),
    }
}

/// How many points one scroll request asks for.
const SCROLL_PAGE: usize = 1000;

/// Ceiling on the pages one call will fetch, so a misbehaving or hostile
/// endpoint that always reports another page cannot spin here forever.
const SCROLL_MAX_PAGES: usize = 1000;

/// Every document in the collection, with the number of chunks each has.
///
/// The scroll is followed to the end. Asking once with a limit and taking what
/// comes back is the shape this used to have, and it silently truncates: the
/// limit is in **points**, and a document is a few dozen of those, so a
/// collection of any size stopped at the first page and the admin's sync check
/// then reported every document past it as "in SQLite but not in Qdrant" -
/// documents that are entirely present and entirely fine, listed for deletion.
async fn scroll_all_documents(
    client: &reqwest::Client,
    qdrant_url: &str,
    collection: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let url = format!("{qdrant_url}/collections/{collection}/points/scroll");
    let mut docs: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    // A point id. This project's are UUID strings, and the scroll offset is
    // whatever the previous page's next_page_offset was, so it is passed back
    // exactly as Qdrant gave it rather than read as a number.
    let mut offset: Option<serde_json::Value> = None;

    for _ in 0..SCROLL_MAX_PAGES {
        let mut body = json!({
            "limit": SCROLL_PAGE,
            "with_payload": true,
            "with_vector": false,
        });
        if let Some(at) = &offset {
            body["offset"] = at.clone();
        }
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("scrolling the vectors of {collection}"))?;
        let status = resp.status();
        if !status.is_success() {
            let detail = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant refused the scroll of {collection} ({status}): {detail}");
        }
        let raw: serde_json::Value = resp
            .json()
            .await
            .with_context(|| format!("reading the scroll of {collection}"))?;

        // A page short of the limit is the last one. Qdrant also hands back
        // `next_page_offset`, and that is the authoritative answer, so it is
        // preferred and the length is only the fallback for an older build that
        // omits it.
        let result = raw.get("result");
        let points = result
            .and_then(|r| r.get("points"))
            .and_then(|p| p.as_array())
            .map(Vec::len)
            .unwrap_or(0);
        let next = result
            .and_then(|r| r.get("next_page_offset"))
            .filter(|v| !v.is_null())
            .cloned();
        let last = next.is_none() && points < SCROLL_PAGE;

        // Group by document_id from the payloads.
        if let Some(points) = result
            .and_then(|r| r.get("points"))
            .and_then(|p| p.as_array())
        {
            for point in points {
                let Some(payload) = point.get("payload") else {
                    continue;
                };
                let doc_id = payload.get("document_id").and_then(|v| v.as_str()).unwrap_or("");
                if doc_id.is_empty() { continue; }
                let entry = docs.entry(doc_id.to_owned()).or_insert_with(|| json!({
                    "document_id": doc_id,
                    "filename": payload.get("filename").and_then(|v| v.as_str()).unwrap_or(""),
                    "upload_date": payload.get("upload_date").and_then(|v| v.as_str()).unwrap_or(""),
                    "chunk_count": 0_i64,
                }));
                if let Some(n) = entry.get_mut("chunk_count").and_then(|v| v.as_i64()) {
                    *entry.get_mut("chunk_count").unwrap() = json!(n + 1);
                }
            }
        }

        match next {
            Some(at) if !last => offset = Some(at),
            _ => break,
        }
    }

    let mut list: Vec<serde_json::Value> = docs.into_values().collect();
    list.sort_by(|a, b| {
        let da = a.get("upload_date").and_then(|v| v.as_str()).unwrap_or("");
        let db = b.get("upload_date").and_then(|v| v.as_str()).unwrap_or("");
        db.cmp(da)
    });
    Ok(list)
}

// ── DELETE /api/admin/qdrant/document/{id} ────────────────────────────────────

pub async fn qdrant_delete_document(
    State(state): State<AppState>,
    claims: Claims,
    Path(document_id): Path<String>,
) -> Response {
    if let Some(r) = require_admin(&claims) { return r; }
    // INVARIANT: use the SINGLE entry point shared with the user-facing
    // delete — Qdrant AND SQLite together, Qdrant first. This handler used to
    // delete from Qdrant ONLY, over REST, leaving the SQLite row "active" with
    // no vectors: an orphan, and exactly the second entry point the invariant
    // exists to forbid.
    match crate::api::documents::purge_document(&state, &document_id).await {
        Ok(true) => Json(json!({ "deleted": true })).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "document not found"),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

// ── GET /api/admin/sqlite/documents ──────────────────────────────────────────

pub async fn sqlite_documents(State(state): State<AppState>, claims: Claims) -> Response {
    if let Some(r) = require_admin(&claims) { return r; }
    match db::documents::list_all(&state.db).await {
        Ok(docs) => Json(json!({ "documents": docs })).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[cfg(test)]
mod tests {
    use super::is_bad_request;
    use super::scroll_all_documents;

    /// The admin panel's sync check asks Qdrant which documents exist and
    /// reports any that SQLite has and Qdrant does not. That answer came from
    /// a single scroll with a fixed limit, so on any collection past it the
    /// check listed perfectly healthy documents as missing - and the panel
    /// offers to delete them.
    ///
    /// The pages are followed here: the fake answers with one page, then a
    /// second, then stops.
    #[tokio::test]
    async fn the_scroll_is_followed_until_qdrant_says_there_is_no_more() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = [0u8; 4096];
                    let read = stream.read(&mut buf).await.unwrap_or(0);
                    // Which page is being asked for is decided by the request
                    // body carrying an offset.
                    let asked_for_second = read > 0
                        && String::from_utf8_lossy(&buf[..read]).contains("\"offset\"");
                    let page: serde_json::Value = if asked_for_second {
                        serde_json::json!({
                            "result": {
                                "points": [
                                    { "payload": { "document_id": "doc-b", "filename": "b.pdf",
                                                   "upload_date": "2026-01-02T00:00:00Z" } },
                                    { "payload": { "document_id": "doc-c", "filename": "c.pdf",
                                                   "upload_date": "2026-01-03T00:00:00Z" } },
                                ],
                                "next_page_offset": null,
                            }
                        })
                    } else {
                        serde_json::json!({
                            "result": {
                                "points": [
                                    { "payload": { "document_id": "doc-a", "filename": "a.pdf",
                                                   "upload_date": "2026-01-01T00:00:00Z" } },
                                    { "payload": { "document_id": "doc-b", "filename": "b.pdf",
                                                   "upload_date": "2026-01-02T00:00:00Z" } },
                                ],
                                "next_page_offset": 2,
                            }
                        })
                    };
                    let body = page.to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });

        let list = scroll_all_documents(
            &reqwest::Client::new(),
            &format!("http://{addr}"),
            "rag_documents",
        )
        .await
        .expect("both pages");
        let ids: Vec<&str> = list
            .iter()
            .map(|d| d["document_id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            ["doc-c", "doc-b", "doc-a"],
            "newest first, and doc-c is only on the second page"
        );
        let chunks = |id: &str| {
            list.iter()
                .find(|d| d["document_id"] == id)
                .unwrap()["chunk_count"]
                .as_i64()
                .unwrap()
        };
        assert_eq!(chunks("doc-b"), 2, "a document spanning two pages is counted once per chunk");
    }

    fn bad(msg: &str) -> anyhow::Error {
        anyhow::anyhow!("{msg}")
    }

    /// Every refusal backup::service can produce about the archive or the name
    /// it was given, held here so a drift misclassifying a caller error as a
    /// 500 shows up as a failing test rather than as an admin told
    /// "internal server error".
    #[test]
    fn caller_errors_are_recognised() {
        for msg in [
            // resolve_archive
            "invalid archive name: \"../x.tar.gz\"",
            "not a backup archive: \"rag_users.db\"",
            "archive not found: absent.tar.gz",
            // unpack_tar_gz
            "archive entry escapes the destination: x",
            "archive entry is not a regular file or directory: link",
            "the archive's gzip stream is truncated or its checksum does not match",
            // verify_unpacked
            "parsing backup.json: expected value at line 1 column 1",
            "this archive is in backup format 9 and was written by a newer engine (x)",
            "the archive's manifest names \"/etc/passwd\", which is not a plain file name",
            "the archive promises rag_users.db but does not contain it",
            "rag_users.db does not match the archive's own manifest - the backup is damaged",
            // restore_backup
            "the archive contains no rag_users.db: nothing was restored",
            "this archive holds the Qdrant collection \"old\", but this installation is \
             configured for \"new\"",
        ] {
            assert!(is_bad_request(&bad(msg)), "msg={msg:?}");
        }
    }

    /// And ours, which must keep their 5xx - and with it the withheld message.
    #[test]
    fn server_faults_are_not_recognised() {
        for msg in [
            "the Qdrant snapshot could not be verified, backup aborted",
            "the Qdrant snapshot is corrupt: expected sha256 aa, got bb",
            "internal server error",
            "",
        ] {
            assert!(!is_bad_request(&bad(msg)), "msg={msg:?}");
        }
    }

    /// The classification reads the whole chain, so a `.context()` added above
    /// one of these in the service cannot quietly demote a 4xx into a 5xx with
    /// the message withheld.
    #[test]
    fn a_wrapped_caller_error_is_still_a_caller_error() {
        let wrapped = bad("invalid archive name: \"x\"").context("restoring backup");
        assert!(is_bad_request(&wrapped), "{wrapped:#}");
    }
}
