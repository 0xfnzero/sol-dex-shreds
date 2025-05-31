pub mod shredstream;
pub mod shared; 

use std::sync::Arc;

use futures::{channel::mpsc, StreamExt};
use solana_entry::entry::Entry;
use tonic::transport::Channel;

use log::error;
use solana_sdk::transaction::VersionedTransaction;

pub type AnyResult<T> = anyhow::Result<T>;

use solana_sdk::pubkey::Pubkey;
use crate::common::logs_data::DexInstruction;
use crate::common::logs_events::PumpfunEvent;
use crate::common::logs_filters::LogFilter;
use crate::grpc::shredstream::shredstream_proxy_client::ShredstreamProxyClient;
use crate::grpc::shredstream::SubscribeEntriesRequest;

const CHANNEL_SIZE: usize = 1000;

pub struct ShredStreamGrpc {
    shredstream_client: Arc<ShredstreamProxyClient<Channel>>,
}

struct TransactionWithSlot {
    transaction: VersionedTransaction,
    slot: u64,
}

impl ShredStreamGrpc {
    pub async fn new(endpoint: String) -> AnyResult<Self> {
        let shredstream_client = ShredstreamProxyClient::connect(endpoint.clone()).await?;
        Ok(Self { 
            shredstream_client: Arc::new(shredstream_client)
        })
    }

    pub async fn shredstream_subscribe<F>(&self, callback: F, bot_wallet: Option<Pubkey>) -> AnyResult<()> 
    where
        F: Fn(PumpfunEvent) + Send + Sync + 'static,
    {
        let request = tonic::Request::new(SubscribeEntriesRequest {});
        let mut client = (*self.shredstream_client).clone();
        let mut stream = client.subscribe_entries(request).await?.into_inner();
        let (mut tx, mut rx) = mpsc::channel::<TransactionWithSlot>(CHANNEL_SIZE);
        let callback = Box::new(callback);
        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if let Ok(entries) = bincode::deserialize::<Vec<Entry>>(&msg.entries) {
                            for entry in entries {
                                for transaction in entry.transactions {
                                    let _ = tx.try_send(TransactionWithSlot {
                                        transaction: transaction.clone(),
                                        slot: msg.slot,
                                    });
                                }
                            }
                        }
                    }
                    Err(error) => {
                        error!("Stream error: {error:?}");
                        break;
                    }
                }
            }
        });

        while let Some(transaction_with_slot) = rx.next().await {
            if let Err(e) = Self::process_pumpfun_transaction(transaction_with_slot, &*callback, bot_wallet).await {
                error!("Error processing transaction: {:?}", e);
            }
        }
    
        Ok(())
    }

    async fn process_pumpfun_transaction<F>(transaction_with_slot: TransactionWithSlot, callback: &F, bot_wallet: Option<Pubkey>) -> AnyResult<()> 
    where
        F: Fn(PumpfunEvent) + Send + Sync,
    {
        let slot = transaction_with_slot.slot;
        let versioned_tx = transaction_with_slot.transaction;
        let mut dev_address: Option<Pubkey> = None;
        // let hash = versioned_tx.signatures[0].to_string();
        let instructions = LogFilter::parse_compiled_instruction(versioned_tx, bot_wallet).unwrap();
        for instruction in instructions {
            // println!("hash: {}\ninstruction: {:?}\n\n", hash, instruction);
            match instruction {
                DexInstruction::CreateToken(mut token_info) => {
                    token_info.slot = slot;
                    dev_address = Some(token_info.user);
                    callback(PumpfunEvent::NewToken(token_info));
                }
                DexInstruction::UserTrade(mut trade_info) => {
                    trade_info.slot = slot;
                    if Some(trade_info.user) == dev_address {
                        callback(PumpfunEvent::NewDevTrade(trade_info));
                    } else {
                        callback(PumpfunEvent::NewUserTrade(trade_info));
                    }
                }
                DexInstruction::BotTrade(mut trade_info) => {
                    trade_info.slot = slot;
                    callback(PumpfunEvent::NewBotTrade(trade_info));
                }
                _ => {}
            }
        }
        Ok(())
    }
}
