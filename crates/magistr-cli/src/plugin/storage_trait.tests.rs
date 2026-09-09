use super::*;

#[test]
fn storage_record_fields() {
    let record = StorageRecord {
        session_id: "sess-123".to_string(),
        project: Some("my-project".to_string()),
        data: serde_json::json!({"prompt": "hello", "turns": 3}),
    };

    assert_eq!(record.session_id, "sess-123");
    assert_eq!(record.project.as_deref(), Some("my-project"));
    assert_eq!(record.data["turns"], 3);
}

#[test]
fn storage_filter_defaults() {
    let filter = StorageFilter::default();
    assert!(filter.project.is_none());
    assert!(filter.limit.is_none());
    assert!(filter.session_ids.is_none());
}

#[test]
fn sqlite_storage_name() {
    let storage = SqliteStorage::new("/tmp/test.db");
    assert_eq!(storage.name(), "sqlite");
}

#[test]
fn sqlite_storage_db_path() {
    let storage = SqliteStorage::new("/tmp/magistr-test.db");
    assert_eq!(
        storage.db_path(),
        std::path::Path::new("/tmp/magistr-test.db")
    );
}

#[tokio::test]
async fn sqlite_storage_store_and_get() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let mut storage = SqliteStorage::new(&db_path);

    let record = StorageRecord {
        session_id: "sess-001".to_string(),
        project: Some("test-project".to_string()),
        data: serde_json::json!({"prompt": "hello world"}),
    };

    storage.store(record.clone()).await.unwrap();

    let got = storage.get("sess-001").await.unwrap();
    assert!(got.is_some());
    let got = got.unwrap();
    assert_eq!(got.session_id, "sess-001");
    assert_eq!(got.project.as_deref(), Some("test-project"));
    assert_eq!(got.data["prompt"], "hello world");
}

#[tokio::test]
async fn sqlite_storage_get_missing() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let mut storage = SqliteStorage::new(&db_path);

    let got = storage.get("nonexistent").await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn sqlite_storage_store_upsert() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let mut storage = SqliteStorage::new(&db_path);

    let record1 = StorageRecord {
        session_id: "sess-002".to_string(),
        project: Some("proj".to_string()),
        data: serde_json::json!({"version": 1}),
    };
    storage.store(record1).await.unwrap();

    let record2 = StorageRecord {
        session_id: "sess-002".to_string(),
        project: Some("proj".to_string()),
        data: serde_json::json!({"version": 2}),
    };
    storage.store(record2).await.unwrap();

    let got = storage.get("sess-002").await.unwrap().unwrap();
    assert_eq!(got.data["version"], 2);
}

#[tokio::test]
async fn sqlite_storage_delete() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let mut storage = SqliteStorage::new(&db_path);

    let record = StorageRecord {
        session_id: "sess-del".to_string(),
        project: None,
        data: serde_json::json!({}),
    };
    storage.store(record).await.unwrap();

    assert!(storage.delete("sess-del").await.unwrap());
    assert!(!storage.delete("sess-del").await.unwrap());
    assert!(storage.get("sess-del").await.unwrap().is_none());
}

#[tokio::test]
async fn sqlite_storage_query_by_project() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let mut storage = SqliteStorage::new(&db_path);

    for i in 0..5 {
        let project = if i < 3 { "alpha" } else { "beta" };
        storage
            .store(StorageRecord {
                session_id: format!("sess-{i}"),
                project: Some(project.to_string()),
                data: serde_json::json!({"i": i}),
            })
            .await
            .unwrap();
    }

    let alpha = storage
        .query(StorageFilter {
            project: Some("alpha".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(alpha.len(), 3);

    let beta = storage
        .query(StorageFilter {
            project: Some("beta".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(beta.len(), 2);
}

#[tokio::test]
async fn sqlite_storage_query_with_limit() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let mut storage = SqliteStorage::new(&db_path);

    for i in 0..10 {
        storage
            .store(StorageRecord {
                session_id: format!("sess-{i}"),
                project: None,
                data: serde_json::json!({}),
            })
            .await
            .unwrap();
    }

    let limited = storage
        .query(StorageFilter {
            limit: Some(3),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(limited.len(), 3);
}

#[test]
fn postgres_storage_masked_url() {
    let pg = PostgresStorage::new("postgresql://user:password@localhost:5432/magistr");
    assert_eq!(pg.masked_url(), "postgresql://***@localhost:5432/magistr");
}

#[test]
fn postgres_storage_name() {
    let pg = PostgresStorage::new("postgresql://localhost/magistr");
    assert_eq!(pg.name(), "postgresql");
}

#[test]
fn s3_storage_name() {
    let s3 = S3Storage::new("my-bucket");
    assert_eq!(s3.name(), "s3");
}

#[test]
fn s3_storage_bucket() {
    let s3 = S3Storage::new("magistr-sessions");
    assert_eq!(s3.bucket(), "magistr-sessions");
}

#[test]
fn s3_storage_object_url_default() {
    let s3 = S3Storage::new("my-bucket");
    let url = s3.object_url("sess-123");
    assert_eq!(
        url,
        "https://s3.amazonaws.com/my-bucket/magistr/sessions/sess-123.json"
    );
}

#[test]
fn s3_storage_object_url_custom_endpoint() {
    let s3 = S3Storage::new("my-bucket").with_endpoint("https://minio.local:9000");
    let url = s3.object_url("sess-456");
    assert_eq!(
        url,
        "https://minio.local:9000/my-bucket/magistr/sessions/sess-456.json"
    );
}

#[test]
fn extract_s3_keys_basic() {
    let xml = r#"<?xml version="1.0"?>
<ListBucketResult>
  <Contents><Key>magistr/sessions/a.json</Key></Contents>
  <Contents><Key>magistr/sessions/b.json</Key></Contents>
</ListBucketResult>"#;

    let keys = extract_s3_keys(xml);
    assert_eq!(
        keys,
        vec!["magistr/sessions/a.json", "magistr/sessions/b.json"]
    );
}

#[test]
fn extract_s3_keys_empty() {
    let xml = r#"<?xml version="1.0"?><ListBucketResult></ListBucketResult>"#;
    let keys = extract_s3_keys(xml);
    assert!(keys.is_empty());
}
