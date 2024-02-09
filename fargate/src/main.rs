use warp::{Filter, Reply};
use rand::Rng;
use std::net::SocketAddr;
use std::env;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use aws_sdk_dynamodb::{Client, Error};
use aws_sdk_dynamodb::types::{AttributeValue};

#[derive(Debug, Deserialize, Serialize)]
struct RollResult {
    dice: u32,
    rolls: Vec<u32>,
    total: u32,
}


#[tokio::main]
async fn main() -> Result<(), aws_sdk_dynamodb::Error> {
    let addr: SocketAddr = "0.0.0.0:80".parse().unwrap(); // Bind to all available network interfaces
    let table = env::var("TABLE_NAME").expect("Error: TABLE_NAME not found");

    // datetime
    let current_datetime: DateTime<Utc> = Utc::now();
    let now: String = current_datetime.to_rfc3339();
    println!("{}", now);

    // set up as a DynamoDB client
    let shared_config = aws_config::load_from_env().await;
    let client = Client::new(&shared_config);

    // test connection
    let req = client.list_tables().limit(10);
    let resp = req.send().await?;
    println!("Current DynamoDB tables: {:?}", resp.table_names);

    // dice roll
    let mut rng = rand::thread_rng();
    let roll: u8 = rng.gen_range(1..=6);

    // Define a warp route for rolling dice
    let roll_dice = warp::path!("roll" / u32)
        .map(|num_dice| {
            let mut rng = rand::thread_rng();
            let rolls: Vec<u32> = (0..num_dice).map(|_| rng.gen_range(1..=6)).collect();
            let total: u32 = rolls.iter().sum();
            let roll_result = RollResult {
                dice: num_dice,
                rolls,
                total,
            };
            warp::reply::json(&roll_result)
        });
    
    // save result to dynamodb
    client
        .put_item()
        .table_name(table)
        .item(
            "name",
            AttributeValue::S(String::from(
                "random".to_string(),
            )),
        )
        .item(
            "createdAt",
            AttributeValue::S(String::from(
                now,
            )),
        )
        .item(
            "roll",
            AttributeValue::N(roll.to_string()),
        )
        .send()
        .await?;

    let index = warp::path::end()
        .map(|| {
            warp::reply::html(
                r#"
                <html>
                    <head>
                        <title>Roll Dice</title>
                    </head>
                    <body>
                        <h1>Roll Dice</h1>
                        <p>Roll some dice by visiting <code>/roll/&lt;number&gt;</code></p>
                    </body>
                </html>
                "#,
            )
        });    

    // Combine the filters into a warp service
    let routes = roll_dice.or(index);

    // Start the warp server
    warp::serve(routes)
        .run(addr)
        .await;
    
    println!("Dice Server listening on port: 80");

    Ok(())
}