use axum::{Json, extract::State, http::StatusCode};
use faucet_valkey::FaucetStats;
use serde::Serialize;
use tracing::error;
use utoipa::ToSchema;

use crate::AppState;

#[derive(Debug, Eq, PartialEq, Serialize, ToSchema)]
pub(super) struct StatsResponse {
    total_sent_nanograms: u64,
    antifraud: AntifraudStatsResponse,
}

#[derive(Debug, Eq, PartialEq, Serialize, ToSchema)]
pub(super) struct AntifraudStatsResponse {
    wallet_balance: u64,
    sent_amount_window: u64,
    subnet_amount_window: u64,
    successful_claim_window: u64,
}

#[derive(Serialize)]
pub(super) struct ErrorResponse {
    error: &'static str,
}

type StatsResult = Result<Json<StatsResponse>, (StatusCode, Json<ErrorResponse>)>;

#[utoipa::path(
    get,
    path = "/stats",
    responses(
        (status = 200, description = "Aggregate amount sent and antifraud trigger counts", body = StatsResponse),
        (status = 500, description = "Failed to load faucet statistics", body = crate::handlers::auth::ErrorResponse)
    ),
    tag = "statistics"
)]
pub(super) async fn get_stats(State(state): State<AppState>) -> StatsResult {
    let stats = state.valkey.get_stats().await.map_err(|err| {
        error!(error = %err, "Failed to get faucet stats");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to get faucet stats",
            }),
        )
    })?;

    Ok(Json(stats.into()))
}

impl From<FaucetStats> for StatsResponse {
    fn from(stats: FaucetStats) -> Self {
        Self {
            total_sent_nanograms: stats.total_sent_nanograms,
            antifraud: AntifraudStatsResponse {
                wallet_balance: stats.antifraud.wallet_balance,
                sent_amount_window: stats.antifraud.sent_amount_window,
                subnet_amount_window: stats.antifraud.subnet_amount_window,
                successful_claim_window: stats.antifraud.successful_claim_window,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use faucet_valkey::{AntifraudStats, FaucetStats};
    use serde_json::json;

    use super::StatsResponse;

    #[test]
    fn serializes_stats_response() {
        let response = StatsResponse::from(FaucetStats {
            total_sent_nanograms: 1_500_000_000,
            antifraud: AntifraudStats {
                wallet_balance: 2,
                sent_amount_window: 3,
                subnet_amount_window: 4,
                successful_claim_window: 5,
            },
        });

        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({
                "total_sent_nanograms": 1_500_000_000_u64,
                "antifraud": {
                    "wallet_balance": 2,
                    "sent_amount_window": 3,
                    "subnet_amount_window": 4,
                    "successful_claim_window": 5,
                },
            })
        );
    }
}
