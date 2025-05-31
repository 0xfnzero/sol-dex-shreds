use shred_parsed::{PumpfunEvent, ShredStreamGrpc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let grpc = ShredStreamGrpc::new(
        "http://127.0.0.1:10000".to_string(), 
    ).await?;

    let callback = |event: PumpfunEvent| {
        match event {
            PumpfunEvent::NewDevTrade(trade_info) => {
                println!("Received new dev trade event: {:?}", trade_info);
            },
            PumpfunEvent::NewToken(token_info) => {
                println!("Received new token event: {:?}", token_info);
            },
            PumpfunEvent::NewUserTrade(trade_info) => {
                println!("Received new trade event: {:?}", trade_info);
            },
            PumpfunEvent::NewBotTrade(trade_info) => {
                println!("Received new bot trade event: {:?}", trade_info);
            },
            PumpfunEvent::Error(err) => {
                println!("Received error: {}", err);
            }
        }
    };

    grpc.shredstream_subscribe(callback, None).await?;

    Ok(())  
}