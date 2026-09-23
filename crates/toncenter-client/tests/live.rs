use toncenter_client::{
    Client,
    toncenter::{v2, v3},
};

#[tokio::test]
async fn public_endpoints() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .mainnet()
        .api_key(std::env::var("TONCENTER_API_KEY").ok())
        .build()?;
    let info = client
        .v2()
        .get_masterchain_info(&v2::requests::EmptyRequest {})
        .await?;
    assert!(info.last.seqno > 0);
    let indexed = client
        .v3()
        .get_masterchain_info(&v3::requests::MasterchainInfoQuery {})
        .await?;
    assert!(indexed.last.seqno > 0);
    client
        .v3()
        .get_messages(&v3::requests::MessagesQuery {
            limit: Some(20),
            ..Default::default()
        })
        .await?;
    Ok(())
}
