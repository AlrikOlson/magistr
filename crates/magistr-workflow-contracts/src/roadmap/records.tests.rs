use super::*;

fn chunk(id: &str, status: ChunkStatus, priority: u32) -> Chunk {
    Chunk {
        tier: None,
        id: id.into(),
        title: format!("Chunk {id}"),
        name: crate::roadmap::name::derive(id),
        status,
        priority,
        description: String::new(),
        content: None,
        group: None,
        notes: String::new(),
        acceptance: Vec::new(),
        deps: Vec::new(),
        cross_refs: Vec::new(),
        shared: false,
        reprioritize: None,
        status_proposal: None,
        title_proposal: None,
        obsoleted_reason: None,
        blocked_by: None,
        project_id: None,
        created_at: "t0".into(),
        updated_at: "t0".into(),
    }
}

#[test]
fn status_serializes_snake_case() {
    let j = serde_json::to_string(&ChunkStatus::InProgress).unwrap();
    assert_eq!(j, "\"in_progress\"");
}

#[test]
fn chunk_round_trips_through_json() {
    let c = chunk("phase-26a", ChunkStatus::Pending, 10);
    let s = serde_json::to_string(&c).unwrap();
    let back: Chunk = serde_json::from_str(&s).unwrap();
    assert_eq!(c, back);
}

#[test]
fn roadmap_round_trips_and_defaults_optional_fields() {
    // A minimal chunk object missing the #[serde(default)] fields still loads.
    let json = r#"{
        "project_id": "p",
        "chunks": [
            {"id":"a","title":"A","status":"pending","priority":1,
             "created_at":"t","updated_at":"t"}
        ]
    }"#;
    let r: Roadmap = serde_json::from_str(json).unwrap();
    assert_eq!(r.chunks.len(), 1);
    assert_eq!(r.chunks[0].deps, Vec::<String>::new());
    assert!(!r.chunks[0].shared);
    assert!(r.refreshes.is_empty());
}

#[test]
fn transitions_permit_the_normal_lifecycle() {
    use ChunkStatus::*;
    assert!(Pending.allows(InProgress));
    assert!(InProgress.allows(Done));
    assert!(InProgress.allows(Blocked));
    assert!(Blocked.allows(InProgress));
    assert!(Done.allows(InProgress)); // reopen
    assert!(Obsoleted.allows(Pending)); // revive
    assert!(Pending.allows(Pending)); // no-op
}

#[test]
fn transitions_forbid_nonsense_jumps() {
    use ChunkStatus::*;
    assert!(!Obsoleted.allows(InProgress));
    assert!(!Backlog.allows(Done));
    assert!(!Done.allows(Obsoleted));
    assert!(!Blocked.allows(Done));
}

fn roadmap_with(chunks: Vec<Chunk>) -> Roadmap {
    Roadmap {
        project_id: "p".into(),
        preamble: String::new(),
        chunks,
        notes: Vec::new(),
        refreshes: Vec::new(),
        links: Vec::new(),
        tracker_opt_ins: Vec::new(),
        container_links: Vec::new(),
        focuses: Vec::new(),
    }
}

fn container(external_id: &str, created_by_us: bool, updated_at: &str) -> ContainerLink {
    ContainerLink {
        kind: ContainerKind::Group,
        name: "tracker".into(),
        provider: "linear".into(),
        external_id: external_id.into(),
        created_by_us,
        created_at: "2026-07-26T00:00:00Z".into(),
        updated_at: updated_at.into(),
    }
}

/// Recency picks which copy of a container binding wins, but the mint
/// memory must survive the merge in BOTH directions: whichever copy
/// remembers that we created the container is remembering a fact.
#[test]
fn merge_keeps_the_newer_container_binding_but_the_mint_memory_is_sticky() {
    let mut memory = roadmap_with(vec![]);
    memory.container_links = vec![container("proj-1", false, "2026-07-26T12:00:00Z")];
    let mut disk = roadmap_with(vec![]);
    disk.container_links = vec![container("proj-1-old", true, "2026-07-26T01:00:00Z")];

    let merged = merge_roadmaps(&memory, disk);
    let link = &merged.container_links[0];
    assert_eq!(
        link.external_id, "proj-1",
        "the newer binding wins the merge"
    );
    assert!(
        link.created_by_us,
        "but the older copy's mint memory is not erased by recency"
    );

    // And a disk-only binding survives the union — the anti-clobber rule.
    let mut disk2 = roadmap_with(vec![]);
    let mut other = container("init-1", true, "2026-07-26T02:00:00Z");
    other.kind = ContainerKind::Initiative;
    disk2.container_links = vec![other];
    let merged2 = merge_roadmaps(&merged, disk2);
    assert_eq!(merged2.container_links.len(), 2);
}

fn stamped(id: &str, status: ChunkStatus, updated_at: &str) -> Chunk {
    let mut c = chunk(id, status, 10);
    c.updated_at = updated_at.into();
    c
}

#[test]
fn merge_unions_disk_only_chunks() {
    let memory = roadmap_with(vec![chunk("mine", ChunkStatus::Pending, 1)]);
    let disk = roadmap_with(vec![chunk("theirs", ChunkStatus::Pending, 2)]);
    let merged = merge_roadmaps(&memory, disk);
    let ids: Vec<&str> = merged.chunks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["mine", "theirs"]);
}

/// The exception to disk-only survival: a record with no id is not an
/// identity the union can key on, and folding it back in would undo the
/// load-time repair on the very save that persists it — then re-grow a
/// freshly-minted copy on every later load.
#[test]
fn merge_never_resurrects_an_id_less_disk_chunk() {
    let memory = roadmap_with(vec![chunk("mine", ChunkStatus::Pending, 1)]);
    let mut ghost = chunk("", ChunkStatus::Obsoleted, 0);
    ghost.title = String::new();
    let disk = roadmap_with(vec![ghost, chunk("theirs", ChunkStatus::Pending, 2)]);
    let merged = merge_roadmaps(&memory, disk);
    let ids: Vec<&str> = merged.chunks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["mine", "theirs"]);
}

/// Adding `project_id` changed a persisted struct, so the merge path has to
/// keep working across the boundary: a store written before the field
/// existed and one written after hold the same chunk, and recency still
/// decides. If this regressed, upgrading would drop chunks on the first
/// concurrent save.
#[test]
fn merge_reconciles_a_stamped_and_an_unstamped_copy_by_recency() {
    let mut old_copy = stamped("x", ChunkStatus::Pending, "2026-06-09T10:00:00+00:00");
    old_copy.project_id = None; // written before the field existed
    let mut new_copy = stamped("x", ChunkStatus::InProgress, "2026-06-09T11:00:00+00:00");
    new_copy.project_id = Some("ours".into());

    let merged = merge_roadmaps(&roadmap_with(vec![old_copy]), roadmap_with(vec![new_copy]));
    assert_eq!(merged.chunks.len(), 1, "the two copies are one chunk");
    assert_eq!(merged.chunks[0].status, ChunkStatus::InProgress);
    assert_eq!(
        merged.chunks[0].project_id.as_deref(),
        Some("ours"),
        "the newer copy wins whole, stamp included"
    );

    // …and the same in the other direction: an older stamped copy must not
    // beat a newer unstamped one just because it carries a stamp.
    let mut older_stamped = stamped("y", ChunkStatus::Pending, "2026-06-09T10:00:00+00:00");
    older_stamped.project_id = Some("ours".into());
    let mut newer_unstamped = stamped("y", ChunkStatus::Done, "2026-06-09T12:00:00+00:00");
    newer_unstamped.project_id = None;

    let merged = merge_roadmaps(
        &roadmap_with(vec![older_stamped]),
        roadmap_with(vec![newer_unstamped]),
    );
    assert_eq!(merged.chunks[0].status, ChunkStatus::Done);
}

#[test]
fn merge_conflict_newest_updated_at_wins() {
    let memory = roadmap_with(vec![stamped(
        "x",
        ChunkStatus::Pending,
        "2026-06-09T10:00:00+00:00",
    )]);
    let disk = roadmap_with(vec![stamped(
        "x",
        ChunkStatus::InProgress,
        "2026-06-09T11:00:00+00:00",
    )]);
    let merged = merge_roadmaps(&memory, disk);
    assert_eq!(merged.chunks.len(), 1);
    assert_eq!(
        merged.chunks[0].status,
        ChunkStatus::InProgress,
        "the newer disk copy must win"
    );

    // And the reverse: newer memory copy beats an older disk copy.
    let memory = roadmap_with(vec![stamped(
        "x",
        ChunkStatus::Done,
        "2026-06-09T12:00:00+00:00",
    )]);
    let disk = roadmap_with(vec![stamped(
        "x",
        ChunkStatus::Pending,
        "2026-06-09T11:00:00+00:00",
    )]);
    let merged = merge_roadmaps(&memory, disk);
    assert_eq!(merged.chunks[0].status, ChunkStatus::Done);
}

#[test]
fn merge_tie_or_garbage_stamp_keeps_memory() {
    // Equal stamps: memory wins (pin/mutation just made must stick).
    let memory = roadmap_with(vec![stamped(
        "x",
        ChunkStatus::Done,
        "2026-06-09T10:00:00+00:00",
    )]);
    let disk = roadmap_with(vec![stamped(
        "x",
        ChunkStatus::Pending,
        "2026-06-09T10:00:00+00:00",
    )]);
    assert_eq!(
        merge_roadmaps(&memory, disk).chunks[0].status,
        ChunkStatus::Done
    );
    // A malformed disk stamp can never win.
    let disk = roadmap_with(vec![stamped("x", ChunkStatus::Pending, "not-a-date")]);
    assert_eq!(
        merge_roadmaps(&memory, disk).chunks[0].status,
        ChunkStatus::Done
    );
}

fn link(chunk_id: &str, provider: &str, external_id: &str, updated_at: &str) -> TrackerLink {
    TrackerLink {
        chunk_id: chunk_id.into(),
        provider: provider.into(),
        external_id: external_id.into(),
        our_last_write_hash: "h".into(),
        last_seen_version: Some("1".into()),
        our_last_relations_hash: None,
        our_last_authored_hash: None,
        created_at: "t0".into(),
        updated_at: updated_at.into(),
    }
}

/// The invariant a duplicate ticket would violate: one chunk, one twin per
/// provider. Two processes that both projected the same chunk must converge
/// on the newer binding, not accumulate two — the second would make every
/// later write ambiguous.
#[test]
fn merge_keeps_one_link_per_chunk_and_provider() {
    let mut memory = roadmap_with(vec![]);
    memory
        .links
        .push(link("c1", "github", "1", "2026-07-01T00:00:00+00:00"));
    let mut disk = roadmap_with(vec![]);
    disk.links
        .push(link("c1", "github", "2", "2026-07-02T00:00:00+00:00"));

    let merged = merge_roadmaps(&memory, disk);
    assert_eq!(merged.links.len(), 1, "same (chunk, provider) is one link");
    assert_eq!(merged.links[0].external_id, "2", "newer binding wins");
}

#[test]
fn merge_unions_links_across_providers_and_chunks() {
    let mut memory = roadmap_with(vec![]);
    memory
        .links
        .push(link("c1", "github", "1", "2026-07-01T00:00:00+00:00"));
    let mut disk = roadmap_with(vec![]);
    // Same chunk, different provider — a legitimate second twin.
    disk.links
        .push(link("c1", "linear", "ENG-1", "2026-07-01T00:00:00+00:00"));
    // Different chunk entirely.
    disk.links
        .push(link("c2", "github", "9", "2026-07-01T00:00:00+00:00"));

    let merged = merge_roadmaps(&memory, disk);
    assert_eq!(merged.links.len(), 3);
}

/// A stale disk copy must not clobber a fresher in-memory binding — the
/// same anti-clobber rule chunks get.
#[test]
fn merge_link_tie_or_older_disk_keeps_memory() {
    let mut memory = roadmap_with(vec![]);
    memory
        .links
        .push(link("c1", "github", "fresh", "2026-07-05T00:00:00+00:00"));
    let mut disk = roadmap_with(vec![]);
    disk.links
        .push(link("c1", "github", "stale", "2026-07-01T00:00:00+00:00"));
    assert_eq!(merge_roadmaps(&memory, disk).links[0].external_id, "fresh");

    // Equal stamps: memory wins, matching the chunk rule.
    let mut disk = roadmap_with(vec![]);
    disk.links
        .push(link("c1", "github", "tie", "2026-07-05T00:00:00+00:00"));
    assert_eq!(merge_roadmaps(&memory, disk).links[0].external_id, "fresh");
}

/// A store written before tracker links existed must still load — the same
/// forward-compatibility the project_id rollout needed.
#[test]
fn roadmap_without_links_still_loads() {
    let json = r#"{"project_id":"p","chunks":[]}"#;
    let r: Roadmap = serde_json::from_str(json).unwrap();
    assert!(r.links.is_empty());
    assert!(r.tracker_opt_ins.is_empty());
}

fn opt_in(chunk_id: &str, provider: &str, enabled: bool, updated_at: &str) -> TrackerOptIn {
    TrackerOptIn {
        chunk_id: chunk_id.into(),
        provider: provider.into(),
        enabled,
        updated_at: updated_at.into(),
    }
}

/// Opting out is a recorded `enabled: false`, not a deletion — and this is
/// why. If the opt-out were a removal, the merge would see only the peer's
/// surviving opt-in and silently re-enable projection for a chunk a human
/// just switched off.
#[test]
fn an_opt_out_survives_a_peer_still_holding_the_opt_in() {
    let mut memory = roadmap_with(vec![]);
    memory
        .tracker_opt_ins
        .push(opt_in("c1", "github", false, "2026-07-05T00:00:00+00:00"));
    let mut disk = roadmap_with(vec![]);
    disk.tracker_opt_ins
        .push(opt_in("c1", "github", true, "2026-07-01T00:00:00+00:00"));

    let merged = merge_roadmaps(&memory, disk);
    assert_eq!(merged.tracker_opt_ins.len(), 1);
    assert!(
        !merged.tracker_opt_ins[0].enabled,
        "the newer refusal must win"
    );
}

/// A newer opt-in does win — recency is symmetric, not biased toward off.
#[test]
fn a_newer_opt_in_wins_over_an_older_opt_out() {
    let mut memory = roadmap_with(vec![]);
    memory
        .tracker_opt_ins
        .push(opt_in("c1", "github", false, "2026-07-01T00:00:00+00:00"));
    let mut disk = roadmap_with(vec![]);
    disk.tracker_opt_ins
        .push(opt_in("c1", "github", true, "2026-07-05T00:00:00+00:00"));

    assert!(merge_roadmaps(&memory, disk).tracker_opt_ins[0].enabled);
}

#[test]
fn merge_unions_opt_ins_across_providers_and_chunks() {
    let mut memory = roadmap_with(vec![]);
    memory
        .tracker_opt_ins
        .push(opt_in("c1", "github", true, "2026-07-01T00:00:00+00:00"));
    let mut disk = roadmap_with(vec![]);
    disk.tracker_opt_ins
        .push(opt_in("c1", "linear", true, "2026-07-01T00:00:00+00:00"));
    disk.tracker_opt_ins
        .push(opt_in("c2", "github", true, "2026-07-01T00:00:00+00:00"));

    assert_eq!(merge_roadmaps(&memory, disk).tracker_opt_ins.len(), 3);
}

/// The relations fence is separate from the content fence, so it must merge
/// as part of the link record rather than being dropped by an older peer.
#[test]
fn the_relations_hash_travels_with_the_link() {
    let mut memory = roadmap_with(vec![]);
    memory
        .links
        .push(link("c1", "github", "1", "2026-07-01T00:00:00+00:00"));
    let mut disk = roadmap_with(vec![]);
    let mut newer = link("c1", "github", "1", "2026-07-02T00:00:00+00:00");
    newer.our_last_relations_hash = Some("rel-hash".into());
    disk.links.push(newer);

    let merged = merge_roadmaps(&memory, disk);
    assert_eq!(
        merged.links[0].our_last_relations_hash.as_deref(),
        Some("rel-hash")
    );
}

#[test]
fn merge_unions_refreshes_and_notes_without_duplicates() {
    let mut memory = roadmap_with(vec![]);
    memory.refreshes.push(RefreshNote {
        at: "2026-06-09T10:00:00+00:00".into(),
        summary: "shared".into(),
        think_steps: vec![1],
    });
    memory.notes.push(NoteSection {
        heading: "Vision".into(),
        body: "ours".into(),
    });
    let mut disk = roadmap_with(vec![]);
    disk.refreshes.push(RefreshNote {
        at: "2026-06-09T10:00:00+00:00".into(),
        summary: "shared".into(),
        think_steps: vec![1],
    });
    disk.refreshes.push(RefreshNote {
        at: "2026-06-09T09:00:00+00:00".into(),
        summary: "disk-only".into(),
        think_steps: vec![],
    });
    disk.notes.push(NoteSection {
        heading: "vision".into(), // case-insensitive duplicate
        body: "theirs".into(),
    });
    disk.preamble = "from disk".into();

    let merged = merge_roadmaps(&memory, disk);
    assert_eq!(merged.refreshes.len(), 2, "dedup by (at, summary)");
    assert_eq!(merged.refreshes[0].summary, "disk-only", "sorted by at");
    assert_eq!(merged.notes.len(), 1);
    assert_eq!(merged.notes[0].body, "ours");
    assert_eq!(merged.preamble, "from disk", "empty preamble backfilled");
}

// ---- blocked_by: the readiness vocabulary that is not a dependency ----

/// Two chunk records lifted VERBATIM out of this project's real store,
/// written long before `blocked_by` existed.
///
/// Foreign input on purpose. A fixture the current code just serialized
/// would prove only that serde is self-consistent; these were written by an
/// older binary and carry the shape that actually sits on disk — no
/// `blocked_by`, no `tier`, no `content` — so they exercise every
/// `#[serde(default)]` the struct relies on.
const REAL_STORED_CHUNKS: &str = r#"[
{"id":"ratelimit-null-kv","title":"checkRateLimit crashes on a KV that returns null for a missing key","name":"Ratelimit null KV","status":"backlog","priority":80,"description":"Found while writing the registry tests, by a fake KV that returned null.","notes":"","group":"Cloud sync","acceptance":["Worker suite green"],"deps":[],"cross_refs":["ext:linear/THI-30"],"shared":false,"project_id":"magistr-676f38","created_at":"2026-07-26T02:38:28.940751+00:00","updated_at":"2026-07-26T09:59:53.522332+00:00"},
{"id":"roadmap-content-param-untyped","title":"roadmap_add_chunk's structured body cannot be written from an MCP client","name":"Roadmap content param","status":"backlog","priority":120,"group":"The plan and its store","deps":[],"cross_refs":[],"shared":false,"project_id":"magistr-676f38","created_at":"2026-07-31T18:46:50.516197+00:00","updated_at":"2026-07-31T18:46:50.519797+00:00"}
]"#;

/// The vocabulary is closed, and the error says what the closed set IS.
///
/// Derived rather than hardcoded: every legal spelling comes from
/// `BlockerKind::ALL`, so adding a variant without teaching `from_wire`
/// about it fails here instead of silently becoming unparseable.
#[test]
fn blocker_kind_accepts_exactly_its_own_vocabulary() {
    for kind in BlockerKind::ALL {
        assert_eq!(
            BlockerKind::from_wire(kind.as_wire()),
            Ok(kind),
            "{} is a legal kind and must parse back to itself",
            kind.as_wire()
        );
        // The wire spelling and the serde spelling must not drift apart:
        // `as_wire` is what error messages and any future projection use,
        // and serde is what reaches disk.
        let via_serde = serde_json::to_string(&kind).unwrap();
        assert_eq!(
            via_serde,
            format!("\"{}\"", kind.as_wire()),
            "as_wire must match the serde representation"
        );
    }
}

/// An unrecognised word is REFUSED, never coerced to `External`.
///
/// The failure this guards against is specific: a lenient fallback would
/// file every typo and every future kind as "external", which reads as an
/// answered question rather than a rejected one.
#[test]
fn blocker_kind_refuses_a_word_outside_the_vocabulary() {
    for bad in [
        "blocked",
        "waiting",
        "",
        "PremiseRefuted",
        "premise-refuted",
    ] {
        let err = BlockerKind::from_wire(bad)
            .expect_err("a word outside the vocabulary must be rejected");
        for kind in BlockerKind::ALL {
            assert!(
                err.contains(kind.as_wire()),
                "the error for '{bad}' must name every legal value; missing {}: {err}",
                kind.as_wire()
            );
        }
    }
    assert_ne!(
        BlockerKind::from_wire("nonsense"),
        Ok(BlockerKind::External),
        "an unknown kind must never be coerced into the catch-all"
    );
}

/// A chunk with no blocker must not grow a key. This is what lets the field
/// land on a store of hundreds of records without rewriting any of them.
#[test]
fn a_chunk_without_a_blocker_serializes_no_blocked_by_key() {
    let c = chunk("plain", ChunkStatus::Pending, 10);
    assert!(c.blocked_by.is_none());
    let json = serde_json::to_string(&c).unwrap();
    assert!(
        !json.contains("blocked_by"),
        "skip_serializing_if must keep the key out entirely: {json}"
    );
}

/// Keys a re-save adds to a record that predates them, for reasons that
/// have nothing to do with `blocked_by`.
///
/// These three are `#[serde(default)]` WITHOUT `skip_serializing_if`, so a
/// chunk stored before they existed gains `""` / `[]` values on its next
/// write. That was true before this chunk and is unchanged by it — it is
/// named here so the round-trip tests below can be exact about what they
/// tolerate instead of quietly comparing nothing. `blocked_by` is
/// deliberately NOT among them, and the tests assert that.
const BACKFILLED_ON_RESAVE: [&str; 3] = ["description", "notes", "acceptance"];

/// Current optional-field behavior, checked against recorded JSON.
/// This fixture is not an obligation to retain older persisted formats.
#[test]
fn stored_chunks_without_blocker_round_trip_without_inventing_one() {
    let original: Vec<serde_json::Value> = serde_json::from_str(REAL_STORED_CHUNKS).unwrap();
    let chunks: Vec<Chunk> = serde_json::from_str(REAL_STORED_CHUNKS)
        .expect("current optional blocked_by field may be absent");
    assert_eq!(chunks.len(), 2);

    for c in &chunks {
        assert!(
            c.blocked_by.is_none(),
            "a record with no blocked_by key must load as None, not as some default blocker"
        );
    }

    let after = serde_json::to_value(&chunks).unwrap();
    for (before, after) in original.iter().zip(after.as_array().unwrap()) {
        let before = before.as_object().unwrap();
        let after = after.as_object().unwrap();

        // Nothing the record already said may change or disappear.
        for (k, v) in before {
            assert_eq!(after.get(k), Some(v), "re-saving altered or dropped '{k}'");
        }
        // Anything gained must be a known pre-existing backfill — and the
        // whole point: never `blocked_by`.
        for k in after.keys() {
            if before.contains_key(k) {
                continue;
            }
            assert!(
                BACKFILLED_ON_RESAVE.contains(&k.as_str()),
                "re-saving grew an unexpected key '{k}'"
            );
            assert_ne!(k, "blocked_by", "a chunk with no blocker must not grow one");
        }
    }
}

/// Writing a blocker touches ONE record. Every other chunk in the same
/// store must come back exactly as it went in — otherwise attaching a
/// blocker anywhere would rewrite records everywhere, which is how a
/// "harmless" additive migration quietly churns a whole store.
#[test]
fn writing_a_blocker_leaves_every_other_stored_chunk_untouched() {
    // Serialize FIRST, so the baseline already carries the backfills above
    // and the only difference this test can see is the one it is about.
    let baseline: Vec<serde_json::Value> = {
        let chunks: Vec<Chunk> = serde_json::from_str(REAL_STORED_CHUNKS).unwrap();
        serde_json::to_value(&chunks)
            .unwrap()
            .as_array()
            .unwrap()
            .clone()
    };
    let mut chunks: Vec<Chunk> = serde_json::from_str(REAL_STORED_CHUNKS).unwrap();

    // Derive the victim rather than naming one: whichever record sorts
    // first by id gets the blocker, so the test does not depend on the
    // fixture's authoring order.
    let victim = chunks
        .iter()
        .map(|c| c.id.clone())
        .min()
        .expect("fixture is non-empty");
    let idx = chunks.iter().position(|c| c.id == victim).unwrap();
    chunks[idx].blocked_by = Some(BlockedBy {
        kind: BlockerKind::PremiseRefuted,
        reason: "measurement disproved the central assumption".into(),
        evidence: Some("think:42".into()),
        blocked_at: "2026-08-07T00:00:00+00:00".into(),
    });

    let after = serde_json::to_value(&chunks).unwrap();
    let after = after.as_array().unwrap();
    assert_eq!(after.len(), baseline.len());

    let mut blocked_seen = 0;
    for (i, c) in chunks.iter().enumerate() {
        if c.id == victim {
            blocked_seen += 1;
            assert!(
                after[i].get("blocked_by").is_some(),
                "the blocked chunk must carry the field"
            );
        } else {
            assert_eq!(
                after[i], baseline[i],
                "chunk '{}' was not touched and must serialize identically",
                c.id
            );
        }
    }
    assert_eq!(blocked_seen, 1, "exactly one record should have changed");
}
