use anyhow::Context;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

static STATIC_BLACKLIST_SUBJECTS: &[&str] =
    &["wallet:0:ff3b559e33e89c318f092281b383248443c8afbe84a9812a20954b1ab9ae98"];
const STATIC_BLACKLIST_REASON: &str = "address is unavailable";

#[derive(Clone)]
pub(crate) struct BlacklistStore {
    pool: SqlitePool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BlacklistMatch {
    pub(crate) source: BlacklistSource,
    pub(crate) subject: String,
    pub(crate) reason: String,
    pub(crate) expires_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BlacklistSource {
    Static,
    Database,
}

impl BlacklistSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Database => "database",
        }
    }
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
        if let Some(entry) = static_blacklist_match(subjects, STATIC_BLACKLIST_SUBJECTS) {
            return Ok(Some(entry));
        }

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
            source: BlacklistSource::Database,
            subject: row.try_get("subject")?,
            reason: row.try_get("reason")?,
            expires_at: row.try_get("expires_at")?,
        }))
    }

    pub(crate) async fn active_entries(&self) -> anyhow::Result<Vec<BlacklistMatch>> {
        let rows = sqlx::query(
            r#"
            SELECT subject, reason, expires_at
            FROM antifraud_blacklist
            WHERE expires_at IS NULL OR expires_at > unixepoch()
            ORDER BY created_at ASC, subject ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list active antifraud blacklist entries")?;

        let mut entries = STATIC_BLACKLIST_SUBJECTS
            .iter()
            .map(|subject| BlacklistMatch {
                source: BlacklistSource::Static,
                subject: (*subject).to_string(),
                reason: STATIC_BLACKLIST_REASON.to_string(),
                expires_at: None,
            })
            .collect::<Vec<_>>();

        for row in rows {
            entries.push(BlacklistMatch {
                source: BlacklistSource::Database,
                subject: row.try_get("subject")?,
                reason: row.try_get("reason")?,
                expires_at: row.try_get("expires_at")?,
            });
        }

        Ok(entries)
    }
}

fn static_blacklist_match(subjects: &[&str], blacklist: &[&str]) -> Option<BlacklistMatch> {
    let subject = subjects
        .iter()
        .find(|subject| blacklist.contains(subject))?;
    Some(BlacklistMatch {
        source: BlacklistSource::Static,
        subject: (*subject).to_string(),
        reason: STATIC_BLACKLIST_REASON.to_string(),
        expires_at: None,
    })
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::{
        BlacklistMatch, BlacklistSource, BlacklistStore, STATIC_BLACKLIST_REASON,
        STATIC_BLACKLIST_SUBJECTS, static_blacklist_match,
    };

    async fn store() -> BlacklistStore {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        BlacklistStore::setup(pool).await.unwrap()
    }

    #[test]
    fn finds_static_entry() {
        let blacklisted_wallet = STATIC_BLACKLIST_SUBJECTS[0];
        assert_eq!(
            static_blacklist_match(
                &["wallet:allowed", blacklisted_wallet],
                STATIC_BLACKLIST_SUBJECTS,
            ),
            Some(BlacklistMatch {
                source: BlacklistSource::Static,
                subject: blacklisted_wallet.to_string(),
                reason: STATIC_BLACKLIST_REASON.to_string(),
                expires_at: None,
            })
        );
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
        assert_eq!(entry.source, BlacklistSource::Database);
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

    #[tokio::test]
    async fn lists_static_and_active_database_entries() {
        let store = store().await;
        sqlx::query(
            r#"
            INSERT INTO antifraud_blacklist (subject, reason, expires_at)
            VALUES
                ('device-uid:active', 'automated abuse', NULL),
                ('client-ip:192.0.2.1', 'temporary abuse', unixepoch() + 60),
                ('client-ip:192.0.2.2', 'expired abuse', unixepoch() - 1)
            "#,
        )
        .execute(&store.pool)
        .await
        .unwrap();

        let entries = store.active_entries().await.unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0],
            BlacklistMatch {
                source: BlacklistSource::Static,
                subject: STATIC_BLACKLIST_SUBJECTS[0].to_string(),
                reason: STATIC_BLACKLIST_REASON.to_string(),
                expires_at: None,
            }
        );
        assert_eq!(entries[1].source, BlacklistSource::Database);
        assert_eq!(entries[1].subject, "client-ip:192.0.2.1");
        assert_eq!(entries[1].reason, "temporary abuse");
        assert!(entries[1].expires_at.is_some());
        assert_eq!(
            entries[2],
            BlacklistMatch {
                source: BlacklistSource::Database,
                subject: "device-uid:active".to_string(),
                reason: "automated abuse".to_string(),
                expires_at: None,
            }
        );
    }
}
