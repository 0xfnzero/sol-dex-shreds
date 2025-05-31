use std::str::FromStr;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::common::error::{ClientError, ClientResult};
use crate::common::{
    logs_data::{DexInstruction, CreateTokenInfo, TradeInfo}, 
    logs_filters::LogFilter
};

use solana_sdk::pubkey::Pubkey;
use solana_sdk::instruction::CompiledInstruction;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn process_logs<F>(
    signature: &str,
    logs: Vec<String>,
    callback: F,
    payer: Option<Pubkey>,
) -> ClientResult<()>
where
    F: Fn(&str, DexInstruction) + Send + Sync,
{
    let instructions = LogFilter::parse_instruction(&logs, payer)?;
    for instruction in instructions {
        callback(signature, instruction);
    }
    Ok(())
}

// Add parsing function
pub fn parse_create_token_data(data: &str) -> ClientResult<CreateTokenInfo> {
    // First do base64 decoding
    let decoded = BASE64.decode(data)
        .map_err(|e| ClientError::Other(format!("Failed to decode base64: {}", e)))?;
    
    // Skip prefix bytes (if any)
    let mut cursor = if decoded.len() > 8 { 8 } else { 0 };
    
    // Read name length and name
    if cursor + 4 > decoded.len() {
        return Err(ClientError::Other("Data too short for name length".to_string()));
    }
    let name_len = read_u32(&decoded[cursor..]) as usize;
    cursor += 4;
    
    if cursor + name_len > decoded.len() {
        return Err(ClientError::Other(format!("Data too short for name: need {} bytes", name_len)));
    }
    let name = String::from_utf8(decoded[cursor..cursor + name_len].to_vec())
        .map_err(|e| ClientError::Other(format!("Invalid UTF-8 in name: {}", e)))?;
    cursor += name_len;
    
    // Read symbol length and symbol
    if cursor + 4 > decoded.len() {
        return Err(ClientError::Other("Data too short for symbol length".to_string()));
    }
    let symbol_len = read_u32(&decoded[cursor..]) as usize;
    cursor += 4;
    
    if cursor + symbol_len > decoded.len() {
        return Err(ClientError::Other(format!("Data too short for symbol: need {} bytes", symbol_len)));
    }
    let symbol = String::from_utf8(decoded[cursor..cursor + symbol_len].to_vec())
        .map_err(|e| ClientError::Other(format!("Invalid UTF-8 in symbol: {}", e)))?;
    cursor += symbol_len;
    
    // Read URI length and URI
    if cursor + 4 > decoded.len() {
        return Err(ClientError::Other("Data too short for URI length".to_string()));
    }
    let uri_len = read_u32(&decoded[cursor..]) as usize;
    cursor += 4;
    
    if cursor + uri_len > decoded.len() {
        return Err(ClientError::Other(format!("Data too short for URI: need {} bytes", uri_len)));
    }
    let uri = String::from_utf8(decoded[cursor..cursor + uri_len].to_vec())
        .map_err(|e| ClientError::Other(format!("Invalid UTF-8 in uri: {}", e)))?;
    cursor += uri_len;
    
    // Make sure there is enough data to read public keys
    if cursor + 32 * 3 > decoded.len() {
        return Err(ClientError::Other("Data too short for public keys".to_string()));
    }
    
    // Parse Mint Public Key
    let mint = bs58::encode(&decoded[cursor..cursor+32]).into_string();
    cursor += 32;

    // Parse Bonding Curve Public Key
    let bonding_curve = bs58::encode(&decoded[cursor..cursor+32]).into_string();
    cursor += 32;

    // Parse User Public Key
    let user = bs58::encode(&decoded[cursor..cursor+32]).into_string();

    Ok(CreateTokenInfo {
        slot: 0,
        name,
        symbol,
        uri,
        mint: Pubkey::from_str(&mint).unwrap(),
        bonding_curve: Pubkey::from_str(&bonding_curve).unwrap(),
        user: Pubkey::from_str(&user).unwrap(),
    })
}

fn read_u32(data: &[u8]) -> u32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&data[..4]);
    u32::from_le_bytes(bytes)
}

pub fn parse_trade_data(data: &str) -> ClientResult<TradeInfo> {
    let engine = base64::engine::general_purpose::STANDARD;
    let decoded = engine.decode(data).map_err(|e| 
        ClientError::Parse(
            "Failed to decode base64".to_string(),
            e.to_string()
        )
    )?;

    let mut cursor = 8;  // Skip prefix

    // 1. Mint (32 bytes)
    let mint = bs58::encode(&decoded[cursor..cursor + 32]).into_string();
    cursor += 32;

    // 2. Sol Amount (8 bytes)
    let sol_amount = u64::from_le_bytes(decoded[cursor..cursor + 8].try_into().unwrap());
    cursor += 8;

    // 3. Token Amount (8 bytes)
    let token_amount = u64::from_le_bytes(decoded[cursor..cursor + 8].try_into().unwrap());
    cursor += 8;

    // 4. Is Buy (1 byte)
    let is_buy = decoded[cursor] != 0;
    cursor += 1;

    // 5. User (32 bytes)
    let user = bs58::encode(&decoded[cursor..cursor + 32]).into_string();
    cursor += 32;

    // 6. Timestamp (8 bytes)
    let timestamp = i64::from_le_bytes(decoded[cursor..cursor + 8].try_into().unwrap());
    cursor += 8;

    // 7. Virtual Sol Reserves (8 bytes)
    let virtual_sol_reserves = u64::from_le_bytes(decoded[cursor..cursor + 8].try_into().unwrap());
    cursor += 8;

    // 8. Virtual Token Reserves (8 bytes)
    let virtual_token_reserves = u64::from_le_bytes(decoded[cursor..cursor + 8].try_into().unwrap());
    cursor += 8;

    let real_sol_reserves = u64::from_le_bytes(decoded[cursor..cursor + 8].try_into().unwrap());
    cursor += 8;

    let real_token_reserves = u64::from_le_bytes(decoded[cursor..cursor + 8].try_into().unwrap());

    Ok(TradeInfo {
        slot: 0,
        mint: Pubkey::from_str(&mint).unwrap(),
        sol_amount,
        token_amount,
        is_buy,
        user: Pubkey::from_str(&user).unwrap(),
        timestamp,
        virtual_sol_reserves,
        virtual_token_reserves,
        real_sol_reserves,
        real_token_reserves,
    })
}

fn current_timestamp_millis() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    duration.as_millis() as i64
}

pub fn parse_instruction_create_token_data(instruction: &CompiledInstruction, accounts: &[Pubkey]) -> ClientResult<CreateTokenInfo> {
    let data = instruction.data.clone();
    let mut offset = 0;
    offset += 8;
    let len1 = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    let name = String::from_utf8_lossy(&data[offset..offset + len1]);
    offset += len1;
    let len2 = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    let symbol = String::from_utf8_lossy(&data[offset..offset + len2]);
    offset += len2;
    let _flag = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
    offset += 4;
    let hash_start = data.len() - 32;
    let ipfs_bytes = &data[offset..hash_start];
    let uri = String::from_utf8_lossy(ipfs_bytes);
    let mint = accounts[instruction.accounts[0] as usize];
    let user = accounts[instruction.accounts[7] as usize];
    let bonding_curve= accounts[instruction.accounts[2] as usize];
    Ok(CreateTokenInfo {
        slot: 0,
        name: name.to_string(),
        symbol: symbol.to_string(),
        uri: uri.to_string(),
        mint,
        bonding_curve,
        user,
    })
}

pub fn parse_instruction_trade_data(instruction: &CompiledInstruction, accounts: &[Pubkey], is_buy: bool) -> ClientResult<TradeInfo> {
    let data = instruction.data.clone();
    let amount = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let max_sol_cost_or_min_sol_output = u64::from_le_bytes(data[16..24].try_into().unwrap()); 
    let user = accounts[instruction.accounts[6] as usize];
    let mint = accounts[instruction.accounts[2] as usize];
    Ok(TradeInfo {
        slot: 0,
        mint,
        sol_amount: max_sol_cost_or_min_sol_output,
        token_amount: amount,
        is_buy,
        user,
        timestamp: current_timestamp_millis(),
        virtual_sol_reserves: 0,
        virtual_token_reserves: 0,
        real_sol_reserves: 0,
        real_token_reserves: 0,
    })
}