/// Integration tests — build-order gates.
///
/// Step 1: types serde round-trips
/// Step 2: activation values match Python fixtures within 1e-4
/// Step 3: SQLite schema init and CRUD (coming)
///
/// Fixture generation: `PYTHONPATH=../CerebroCortex/src python3 scripts/gen_activation_fixtures.py`
/// using the CerebroCortex venv:
///   `/home/andre/Projects/CerebroCortex/venv/bin/python3 scripts/gen_activation_fixtures.py`

// =============================================================================
// Step 1 — types serde round-trips
// =============================================================================

#[cfg(test)]
mod types_roundtrip {
    use cerebro::types::*;

    #[test]
    fn memory_type_all_variants() {
        let variants = [
            MemoryType::Episodic,
            MemoryType::Semantic,
            MemoryType::Procedural,
            MemoryType::Affective,
            MemoryType::Prospective,
            MemoryType::Schematic,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let back: MemoryType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, v, "failed round-trip for {v:?}");
        }
    }

    #[test]
    fn link_type_all_variants_with_weights() {
        let cases = [
            (LinkType::Causal,      0.9),
            (LinkType::Semantic,    0.8),
            (LinkType::Supports,    0.8),
            (LinkType::PartOf,      0.8),
            (LinkType::Contextual,  0.7),
            (LinkType::DerivedFrom, 0.7),
            (LinkType::Temporal,    0.6),
            (LinkType::Affective,   0.5),
            (LinkType::Contradicts, 0.3),
        ];
        for (lt, expected_w) in cases {
            let json = serde_json::to_string(&lt).unwrap();
            let back: LinkType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, lt);
            let w = back.activation_weight();
            assert!((w - expected_w).abs() < f32::EPSILON,
                "{lt:?}: got {w}, expected {expected_w}");
        }
    }

    #[test]
    fn memory_layer_serde() {
        for v in [MemoryLayer::Sensory, MemoryLayer::Working,
                  MemoryLayer::LongTerm, MemoryLayer::Cortex] {
            let back: MemoryLayer = serde_json::from_str(
                &serde_json::to_string(&v).unwrap()
            ).unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn visibility_scope_global_sql() {
        let scope = VisibilityScope::global();
        let (sql, params) = scope.sql_filter();
        assert_eq!(sql, "1=1");
        assert!(params.is_empty());
    }

    #[test]
    fn visibility_scope_agent_sql() {
        let scope = VisibilityScope::for_agent(AgentId("test-agent".into()));
        let (sql, params) = scope.sql_filter();
        assert!(sql.contains("visibility='shared'"));
        assert!(sql.contains("agent_id=?"));
        assert_eq!(params[0], "test-agent");
    }

    #[test]
    fn memory_node_new_defaults() {
        use cerebro::models::MemoryNode;
        let node = MemoryNode::new("hello world", cerebro::types::MemoryType::Semantic);
        assert_eq!(node.content, "hello world");
        assert_eq!(node.memory_type, cerebro::types::MemoryType::Semantic);
        assert_eq!(node.visibility, cerebro::types::Visibility::Shared);
        assert_eq!(node.access_count, 0);
        assert_eq!(node.access_times.len(), 1); // created_at added as first access
        assert!(!node.id.0.is_empty());
    }
}

// =============================================================================
// Step 2 — activation math vs Python fixtures (tolerance: 1e-4)
// =============================================================================

#[cfg(test)]
mod activation_fixtures {
    use cerebro::activation::{
        base_level_activation, recall_probability, retrievability,
        update_difficulty_on_recall, update_stability_on_lapse, update_stability_on_recall,
    };
    use chrono::{DateTime, Duration, TimeZone, Utc};
    use serde::Deserialize;

    // Fixed reference time matching the fixture generator: 2025-01-01T12:00:00Z
    fn now_fixed() -> DateTime<Utc> {
        Utc.timestamp_opt(1_735_732_800, 0).unwrap()
    }

    // -----------------------------------------------------------------------
    // Fixture loading helpers
    // -----------------------------------------------------------------------

    const FIXTURE_PATH: &str = "tests/fixtures/activation.json";
    const TOL: f32 = 1e-4;

    fn load_fixtures() -> serde_json::Value {
        let path = std::path::Path::new(FIXTURE_PATH);
        if !path.exists() {
            panic!(
                "Fixture file not found: {FIXTURE_PATH}\n\
                 Run: /home/andre/Projects/CerebroCortex/venv/bin/python3 \
                 scripts/gen_activation_fixtures.py"
            );
        }
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    // -----------------------------------------------------------------------
    // ACT-R fixtures
    // -----------------------------------------------------------------------

    #[test]
    fn actr_all_fixture_cases() {
        let fixtures = load_fixtures();
        let now = now_fixed();

        for (i, case) in fixtures["actr"].as_array().unwrap().iter().enumerate() {
            let times_ago: Vec<i64> = serde_json::from_value(
                case["access_times_ago_secs"].clone()
            ).unwrap();
            let decay     = case["decay"].as_f64().unwrap() as f32;
            let expected  = case["actr"].as_f64().unwrap() as f32;

            let times: Vec<DateTime<Utc>> = times_ago
                .iter()
                .map(|&s| now - Duration::seconds(s))
                .collect();

            let got = base_level_activation(&times, now, decay);
            assert!(
                (got - expected).abs() < TOL,
                "ACT-R case {i}: got {got}, expected {expected} (diff {})",
                (got - expected).abs()
            );
        }
    }

    // -----------------------------------------------------------------------
    // FSRS retrievability fixtures
    // -----------------------------------------------------------------------

    #[test]
    fn fsrs_retrievability_all_fixture_cases() {
        let fixtures = load_fixtures();

        for (i, case) in fixtures["fsrs_retrievability"].as_array().unwrap().iter().enumerate() {
            let elapsed   = case["elapsed_days"].as_f64().unwrap() as f32;
            let stability = case["stability"].as_f64().unwrap() as f32;
            let expected  = case["retrievability"].as_f64().unwrap() as f32;

            let got = retrievability(elapsed, stability);
            assert!(
                (got - expected).abs() < TOL,
                "FSRS retrievability case {i}: got {got}, expected {expected}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // FSRS update_stability_on_recall fixtures
    // -----------------------------------------------------------------------

    #[test]
    fn fsrs_update_recall_all_fixture_cases() {
        let fixtures = load_fixtures();

        for (i, case) in fixtures["fsrs_update_recall"].as_array().unwrap().iter().enumerate() {
            let s   = case["stability"].as_f64().unwrap() as f32;
            let d   = case["difficulty"].as_f64().unwrap() as f32;
            let r   = case["retrievability"].as_f64().unwrap() as f32;
            let exp = case["new_stability"].as_f64().unwrap() as f32;

            let got = update_stability_on_recall(s, d, r);
            assert!(
                (got - exp).abs() < TOL,
                "update_stability_on_recall case {i}: s={s} d={d} r={r} → got {got}, expected {exp}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // FSRS update_stability_on_lapse fixtures
    // -----------------------------------------------------------------------

    #[test]
    fn fsrs_update_lapse_all_fixture_cases() {
        let fixtures = load_fixtures();

        for (i, case) in fixtures["fsrs_update_lapse"].as_array().unwrap().iter().enumerate() {
            let s   = case["stability"].as_f64().unwrap() as f32;
            let d   = case["difficulty"].as_f64().unwrap() as f32;
            let exp = case["new_stability"].as_f64().unwrap() as f32;

            let got = update_stability_on_lapse(s, d);
            assert!(
                (got - exp).abs() < TOL,
                "update_stability_on_lapse case {i}: s={s} d={d} → got {got}, expected {exp}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // FSRS update_difficulty_on_recall fixtures
    // -----------------------------------------------------------------------

    #[test]
    fn fsrs_update_difficulty_all_fixture_cases() {
        let fixtures = load_fixtures();

        for (i, case) in fixtures["fsrs_update_difficulty"].as_array().unwrap().iter().enumerate() {
            let d   = case["difficulty"].as_f64().unwrap() as f32;
            let r   = case["retrievability"].as_f64().unwrap() as f32;
            let exp = case["new_difficulty"].as_f64().unwrap() as f32;

            let got = update_difficulty_on_recall(d, r);
            assert!(
                (got - exp).abs() < TOL,
                "update_difficulty case {i}: d={d} r={r} → got {got}, expected {exp}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // recall_probability (sigmoid) fixtures
    // -----------------------------------------------------------------------

    #[test]
    fn recall_probability_all_fixture_cases() {
        let fixtures = load_fixtures();

        for (i, case) in fixtures["recall_probability"].as_array().unwrap().iter().enumerate() {
            let act   = case["activation"].as_f64().unwrap() as f32;
            let tau   = case["threshold"].as_f64().unwrap() as f32;
            let noise = case["noise"].as_f64().unwrap() as f32;
            let exp   = case["probability"].as_f64().unwrap() as f32;

            let got = recall_probability(act, tau, noise);
            assert!(
                (got - exp).abs() < TOL,
                "recall_probability case {i}: act={act} tau={tau} noise={noise} → got {got}, expected {exp}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Link decay fixtures
    // -----------------------------------------------------------------------

    #[test]
    fn link_decay_all_fixture_cases() {
        use cerebro::models::AssociativeLink;
        use cerebro::types::{LinkType, MemoryId};

        let fixtures = load_fixtures();
        let now = Utc::now();

        for (i, case) in fixtures["link_decay"].as_array().unwrap().iter().enumerate() {
            let w        = case["stored_weight"].as_f64().unwrap() as f32;
            let age_days = case["age_days"].as_f64().unwrap() as f32;
            let halflife = case["halflife_days"].as_f64().unwrap() as f32;
            let exp      = case["effective_weight"].as_f64().unwrap() as f32;

            let mut link = AssociativeLink::new(
                MemoryId("a".into()), MemoryId("b".into()), LinkType::Semantic, w,
            );
            // Set last_traversed to age_days ago
            if age_days > 0.0 {
                link.last_traversed = Some(now - Duration::seconds((age_days * 86400.0) as i64));
            } else {
                link.last_traversed = Some(now);
            }

            let got = link.effective_weight(now, halflife);
            assert!(
                (got - exp).abs() < TOL,
                "link_decay case {i}: w={w} age={age_days}d H={halflife}d → got {got}, expected {exp}"
            );
        }
    }
}

// =============================================================================
// Step 3 — SQLite storage (basic)
// =============================================================================

#[cfg(test)]
mod storage_basic {
    use cerebro::{
        config::Config,
        models::{AssociativeLink, MemoryNode},
        storage::{ListFilter, StorageCoordinator},
        types::{AgentId, LinkType, MemoryType, Visibility, VisibilityScope},
    };
    use tempfile::TempDir;

    async fn make_store() -> (StorageCoordinator, TempDir) {
        let dir = TempDir::new().unwrap();
        let config = Config {
            db_path:       dir.path().join("test.db"),
            anthropic_key: None,
            embed_model:   "BAAI/bge-small-en-v1.5".into(),
        };
        let store = StorageCoordinator::new(&config).await.unwrap();
        (store, dir)
    }

    #[tokio::test]
    async fn schema_creates_without_error() {
        let (_store, _dir) = make_store().await;
    }

    #[tokio::test]
    async fn schema_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let config = Config {
            db_path:       dir.path().join("test.db"),
            anthropic_key: None,
            embed_model:   "BAAI/bge-small-en-v1.5".into(),
        };
        StorageCoordinator::new(&config).await.unwrap();
        StorageCoordinator::new(&config).await.unwrap();
    }

    #[tokio::test]
    async fn insert_and_get_memory_global_scope() {
        let (store, _dir) = make_store().await;
        let node = MemoryNode::new("hello world", MemoryType::Semantic);
        let id   = node.id.clone();
        store.sqlite.insert_memory(&node).await.unwrap();

        let got = store.sqlite.get_memory(&id, &VisibilityScope::global()).await.unwrap();
        let got = got.expect("should find the inserted memory");
        assert_eq!(got.id, id);
        assert_eq!(got.content, "hello world");
        assert_eq!(got.memory_type, MemoryType::Semantic);
        assert_eq!(got.visibility, Visibility::Shared);
    }

    #[tokio::test]
    async fn get_memory_returns_none_for_missing_id() {
        let (store, _dir) = make_store().await;
        use cerebro::types::MemoryId;
        let result = store.sqlite.get_memory(
            &MemoryId("does-not-exist".into()),
            &VisibilityScope::global(),
        ).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn scope_filters_private_memories() {
        let (store, _dir) = make_store().await;

        // Private memory owned by agent-a
        let mut node = MemoryNode::new("agent-a secret", MemoryType::Semantic);
        node.visibility = Visibility::Private;
        node.agent_id   = Some(AgentId("agent-a".into()));
        let id = node.id.clone();
        store.sqlite.insert_memory(&node).await.unwrap();

        // agent-a can see it
        let scope_a = VisibilityScope::for_agent(AgentId("agent-a".into()));
        assert!(
            store.sqlite.get_memory(&id, &scope_a).await.unwrap().is_some(),
            "agent-a should see its own private memory"
        );

        // agent-b cannot see it
        let scope_b = VisibilityScope::for_agent(AgentId("agent-b".into()));
        assert!(
            store.sqlite.get_memory(&id, &scope_b).await.unwrap().is_none(),
            "agent-b must not see agent-a's private memory"
        );

        // global scope sees everything
        assert!(
            store.sqlite.get_memory(&id, &VisibilityScope::global()).await.unwrap().is_some(),
            "global scope sees private memories"
        );
    }

    #[tokio::test]
    async fn soft_delete_hides_memory() {
        let (store, _dir) = make_store().await;
        let node = MemoryNode::new("to be deleted", MemoryType::Episodic);
        let id   = node.id.clone();
        store.sqlite.insert_memory(&node).await.unwrap();

        let deleted = store.sqlite.delete_memory(&id).await.unwrap();
        assert!(deleted, "first delete returns true");

        // Should be invisible now
        let got = store.sqlite.get_memory(&id, &VisibilityScope::global()).await.unwrap();
        assert!(got.is_none(), "deleted memory must not appear in get_memory");

        // Second delete returns false (already deleted)
        let deleted2 = store.sqlite.delete_memory(&id).await.unwrap();
        assert!(!deleted2, "double-delete returns false");
    }

    #[tokio::test]
    async fn update_memory_persists_changes() {
        let (store, _dir) = make_store().await;
        let mut node = MemoryNode::new("original", MemoryType::Semantic);
        let id = node.id.clone();
        store.sqlite.insert_memory(&node).await.unwrap();

        node.content = "updated content".into();
        node.salience = 0.9;
        store.sqlite.update_memory(&node).await.unwrap();

        let got = store.sqlite.get_memory(&id, &VisibilityScope::global()).await.unwrap().unwrap();
        assert_eq!(got.content, "updated content");
        assert!((got.salience - 0.9).abs() < 1e-5, "salience should be 0.9, got {}", got.salience);
    }

    #[tokio::test]
    async fn insert_link_and_list_links_from() {
        let (store, _dir) = make_store().await;

        let a = MemoryNode::new("node a", MemoryType::Semantic);
        let b = MemoryNode::new("node b", MemoryType::Semantic);
        let a_id = a.id.clone();
        let b_id = b.id.clone();
        store.sqlite.insert_memory(&a).await.unwrap();
        store.sqlite.insert_memory(&b).await.unwrap();

        let link = AssociativeLink::new(a_id.clone(), b_id.clone(), LinkType::Causal, 0.8);
        store.sqlite.insert_link(&link).await.unwrap();

        let links = store.sqlite.list_links_from(&a_id).await.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].source_id, a_id);
        assert_eq!(links[0].target_id, b_id);
        assert!((links[0].weight - 0.8).abs() < 1e-5, "weight should be 0.8");

        // No links from b
        let links_b = store.sqlite.list_links_from(&b_id).await.unwrap();
        assert!(links_b.is_empty());
    }

    #[tokio::test]
    async fn list_memories_scoped_type_filter() {
        let (store, _dir) = make_store().await;

        store.sqlite.insert_memory(&MemoryNode::new("ep1", MemoryType::Episodic)).await.unwrap();
        store.sqlite.insert_memory(&MemoryNode::new("ep2", MemoryType::Episodic)).await.unwrap();
        store.sqlite.insert_memory(&MemoryNode::new("sem1", MemoryType::Semantic)).await.unwrap();

        let filter = ListFilter { memory_type: Some(MemoryType::Episodic), limit: 50, offset: 0, include_deleted: false };
        let results = store.sqlite.list_memories_scoped(&VisibilityScope::global(), &filter).await.unwrap();
        assert_eq!(results.len(), 2, "should return 2 episodic memories, got {}", results.len());
        for r in &results {
            assert_eq!(r.memory_type, MemoryType::Episodic);
        }
    }
}
