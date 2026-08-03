use anyhow::Context;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

#[derive(Clone)]
pub(crate) struct BlacklistStore {
    pool: SqlitePool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BlacklistMatch {
    pub(crate) subject: String,
    pub(crate) reason: String,
    pub(crate) expires_at: Option<i64>,
}

impl BlacklistStore {
    pub(crate) async fn setup(pool: SqlitePool) -> anyhow::Result<Self> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS antifraud_blacklist (
                subject TEXT PRIMARY KEY NOT NULL CHECK (length(trim(subject)) > 0),
                reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
                created_at INTEGER NOT NULL DEFAULT (unixepoch()),
                expires_at INTEGER
            )
            "#,
        )
        .execute(&pool)
        .await
        .context("Failed to create antifraud blacklist table")?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS antifraud_blacklist_expires_at_idx
            ON antifraud_blacklist (expires_at)
            "#,
        )
        .execute(&pool)
        .await
        .context("Failed to create antifraud blacklist expiry index")?;

        Ok(Self { pool })
    }

    pub(crate) async fn check(&self, subjects: &[&str]) -> anyhow::Result<Option<BlacklistMatch>> {
        if subjects.is_empty() {
            return Ok(None);
        }

        let mut query = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT subject, reason, expires_at
            FROM antifraud_blacklist
            WHERE subject IN (
            "#,
        );
        let mut separated = query.separated(", ");
        for subject in subjects {
            separated.push_bind(*subject);
        }
        separated.push_unseparated(
            r#"
            )
            AND (expires_at IS NULL OR expires_at > unixepoch())
            ORDER BY created_at ASC
            LIMIT 1
            "#,
        );

        let row = query
            .build()
            .fetch_optional(&self.pool)
            .await
            .context("Failed to check antifraud blacklist")?;

        let Some(row) = row else {
            return Ok(None);
        };

        Ok(Some(BlacklistMatch {
            subject: row.try_get("subject")?,
            reason: row.try_get("reason")?,
            expires_at: row.try_get("expires_at")?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::BlacklistStore;

    async fn store() -> BlacklistStore {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        BlacklistStore::setup(pool).await.unwrap()
    }

    #[tokio::test]
    async fn finds_active_entry() {
        let store = store().await;
        sqlx::query(
            r#"
            INSERT INTO antifraud_blacklist (subject, reason)
            VALUES ('device-uid:test', 'automated abuse')
            "#,
        )
        .execute(&store.pool)
        .await
        .unwrap();

        let entry = store
            .check(&["wallet:other", "device-uid:test"])
            .await
            .unwrap()
            .unwrap();

        assert_eq!(entry.subject, "device-uid:test");
        assert_eq!(entry.reason, "automated abuse");
        assert_eq!(entry.expires_at, None);
    }

    #[tokio::test]
    async fn ignores_expired_entry() {
        let store = store().await;
        sqlx::query(
            r#"
            INSERT INTO antifraud_blacklist (subject, reason, expires_at)
            VALUES ('client-ip:192.0.2.1', 'expired', unixepoch() - 1)
            "#,
        )
        .execute(&store.pool)
        .await
        .unwrap();

        assert_eq!(store.check(&["client-ip:192.0.2.1"]).await.unwrap(), None);
    }
}
