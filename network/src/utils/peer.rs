use anyhow::{anyhow, Result};
use solana_sdk::pubkey::Pubkey;
use reqwest::Client as HttpClient;
use serde_json::json;
//use base64::decode;

/// Fetches the Pubkey address for a given spool number from the trusted peer.
pub async fn fetch_spool_address(
    http: &HttpClient,
    trusted_peer_url: &str,
    spool_number: u64,
) -> Result<Pubkey> {
    let addr_resp = http
        .post(trusted_peer_url)
        .header("Content-Type", "application/json")
        .body(
            json!({
                "jsonrpc": "2.0", "id": 1,
                "method": "getSpoolAddress",
                "params": { "spool_number": spool_number }
            })
            .to_string(),
        )
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let addr_str = addr_resp["result"]
        .as_str()
        .ok_or_else(|| anyhow!("Invalid getSpoolAddress response: {:?}", addr_resp))?;

    addr_str.parse().map_err(|_| anyhow!("Invalid Pubkey: {}", addr_str))
}


///// Fetches all segments for a spool from the trusted peer.
//pub async fn fetch_spool_segments(
//    http: &HttpClient,
//    trusted_peer_url: &str,
//    spool_address: &Pubkey,
//) -> Result<Vec<(u64, Vec<u8>)>> {
//    let addr_str = spool_address.to_string();
//    let seg_resp = http
//        .post(trusted_peer_url)
//        .header("Content-Type", "application/json")
//        .body(
//            json!({
//                "jsonrpc": "2.0", "id": 4,
//                "method": "getSpool",
//                "params": { "spool_address": addr_str }
//            })
//            .to_string(),
//        )
//        .send()
//        .await?
//        .json::<serde_json::Value>()
//        .await?;
//
//    let segments = seg_resp["result"]
//        .as_array()
//        .ok_or_else(|| anyhow!("Invalid getSpool response: {:?}", seg_resp))?;
//
//    let mut result = Vec::new();
//    for seg in segments {
//        let seg_num = seg["segment_number"]
//            .as_u64()
//            .ok_or_else(|| anyhow!("Invalid segment_number: {:?}", seg))?;
//        let data_b64 = seg["data"]
//            .as_str()
//            .ok_or_else(|| anyhow!("Invalid data field: {:?}", seg))?;
//        let data = decode(data_b64)?;
//
//        result.push((seg_num, data));
//    }
//
//    Ok(result)
//}


