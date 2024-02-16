use warp::{Filter, http::Response};
use rand::Rng;
use std::net::SocketAddr;
use std::env;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use aws_sdk_dynamodb::{types::{AttributeDefinition, AttributeValue, ScalarAttributeType, KeySchemaElement, BillingMode, KeyType}, Client};
use aws_config::BehaviorVersion; // required for local testing

enum DynamoDBConfig {
    Local,
    Remote,
}

impl DynamoDBConfig {
    async fn from_env() -> Self {
        match env::var("DYNAMODB_CONFIG") {
            Ok(val) if val == "Remote" => DynamoDBConfig::Remote,
            _ => DynamoDBConfig::Local,
        }
    }

    async fn check_connection(&self, client: &Client) -> Result<(), aws_sdk_dynamodb::Error> {
        match self {
            DynamoDBConfig::Local => {
                println!("Local DynamoDB server doesn't support listing tables.");
                Ok(())
            }
            DynamoDBConfig::Remote => {
                println!("Listing tables from remote DynamoDB server...");
                let req = client.list_tables().limit(10);
                let resp = req.send().await?;
                if let Some(table_names) = resp.table_names {
                    println!("Tables:");
                    for name in table_names {
                        println!("{}", name);
                    }
                } else {
                    println!("No tables found.");
                }
                Ok(())
            }
        }
    }
    
    async fn build_client(&self) -> Client {
        match self {
            DynamoDBConfig::Local => {                
                let config = aws_config::defaults(BehaviorVersion::latest())
                    .test_credentials()
                    .load()
                    .await;

                let dynamodb_local_config = aws_sdk_dynamodb::config::Builder::from(&config)
                    .endpoint_url("http://dynamodb:8000")
                    .build();
                Client::from_conf(dynamodb_local_config)
            }

            DynamoDBConfig::Remote => {
                let shared_config = aws_config::load_from_env().await;
             
                Client::new(&shared_config)
                
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct RollResult {
    dice: u32,
    rolls: Vec<u32>,
    total: u32,
}

fn roll_dice(num_dice: u32) -> RollResult {
    let mut rng = rand::thread_rng();
    let rolls: Vec<u32> = (0..num_dice).map(|_| rng.gen_range(1..=6)).collect();
    let total: u32 = rolls.iter().sum();
    RollResult {
        dice: num_dice,
        rolls,
        total,
    }
}

async fn create_table(client: &Client, table_name: &str) {
    let create_table_response = client
        .create_table()
        .table_name(table_name.to_string())
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("name".to_string())
                .attribute_type(ScalarAttributeType::S)
                .build()
                .expect("Failed to build name AttributeDefinition"),
        )
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("createdAt".to_string())
                .attribute_type(ScalarAttributeType::S)
                .build()
                .expect("Failed to build createdAt AttributeDefinition"),
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("name".to_string())
                .key_type(KeyType::Hash) // Using KeyType::HASH
                .build()
                .expect("Failed to build KeySchemaElement for HASH"),
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("createdAt".to_string())
                .key_type(KeyType::Range) // Using KeyType::RANGE
                .build()
                .expect("Failed to build KeySchemaElement for RANGE"),
        )
        .billing_mode(BillingMode::PayPerRequest)
        .send()
        .await;

    match create_table_response {
        Ok(response) => {
            println!("Table created successfully: {:?}", response.table_description);
            // Additional handling if needed
        }
        Err(err) => {
            eprintln!("Failed to create table: {:?}", err);
            // Handle the error appropriately
        }
    }
}

async fn save_roll_result(name: &str, roll_result: &RollResult) -> Result<(), Box<dyn std::error::Error>> {
    // Get DynamoDB configuration from environment variable
    let dynamodb_config = DynamoDBConfig::from_env().await;
    
    // Build the client based on the configuration
    let client = dynamodb_config.build_client().await;

    // set table name from environment variable
    let table_name = env::var("TABLE_NAME").expect("Error: TABLE_NAME not found");
    
    // check if the table exists, and create it if not
    let req = client.list_tables().limit(10);
    let resp = req.send().await?;

    if resp.table_names.is_none() || !resp.table_names.unwrap().contains(&table_name) {
        // Table doesn't exist, create it
        println!("Table '{}' does not exist, creating...", table_name);

        //  Call create_table function
        create_table(&client, &table_name).await;
        
    }
    
    // Print the roll and table name to the console
    println!("Saving dice roll {:?} to table: {:?}...", roll_result, table_name);

    // create item hashmap for name, createdAt, and roll_result
    let mut item = HashMap::new();
    item.insert("name".to_string(), AttributeValue::S(name.to_string()));
    item.insert("createdAt".to_string(), AttributeValue::S(Utc::now().to_rfc3339()));
    item.insert("roll_result".to_string(), AttributeValue::S(serde_json::to_string(&roll_result)?));

    let request = client
        .put_item()
        .table_name(table_name)
        .set_item(Some(item)); // Note the set_item call.
    
    request.send().await?;
    println!("Roll saved successfully!");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), aws_sdk_dynamodb::Error> {
    let addr: SocketAddr = "0.0.0.0:80".parse().unwrap(); // Bind to all available network interfaces

    // Get DynamoDB configuration from environment variable
    let dynamodb_config = DynamoDBConfig::from_env().await;
    let client = dynamodb_config.build_client().await;

    // Print connection status
    match &dynamodb_config {
        DynamoDBConfig::Local => println!("Connected to Local DynamoDB server."),
        DynamoDBConfig::Remote => {
            dynamodb_config.check_connection(&client).await?;
        }
    }

    // Define a warp route for rolling dice
    let roll_route = warp::path!("roll" / u32)
        .and_then(|num_dice| async move {
            let roll_result = roll_dice(num_dice);
            let name = "RollResult";
        
            save_roll_result(name, &roll_result).await;
        
        // Return the roll result as JSON
        Ok::<_, warp::Rejection>(warp::reply::json(&roll_result))
        });

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
    let routes = roll_route.or(index);

    // Start the warp server
    println!("Starting Warp server and listening on port {}", 80);
    
    warp::serve(routes)
        .run(addr)
        .await;
    
    // Confirm server is listening
    println!("Warp server listening on port {}", 80);

    Ok(())
}
